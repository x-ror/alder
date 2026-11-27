//! Reading an app's routes by running `bin/rails runner`, off the request path, one run per app at a time.

use super::routes::{self, Route};
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Rails boots slowly on big apps, but a hung runner must not hold a load forever.
const RUNNER_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// A forked process (spring) can keep the pipes open after the runner exits.
const OUTPUT_GRACE: Duration = Duration::from_secs(1);
const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;
/// A burst of saves to the routes files loads once.
pub const CHANGE_DEBOUNCE: Duration = Duration::from_millis(500);

/// How long `stop` waits for the runners it killed to be gone.
const STOP_WAIT: Duration = Duration::from_secs(3);

/// Set by `stop`: runners in flight are killed and no new one starts.
static STOPPING: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

/// Counts a load in `ACTIVE` until it has cleaned up.
struct Active;

impl Active {
    fn begin() -> Self {
        ACTIVE.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Kills the runners in flight and waits until they and their temp scripts are gone, so the server doesn't exit leaving a
/// process with its working directory in the app (Windows can't delete such a directory).
pub async fn stop() {
    STOPPING.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + STOP_WAIT;
    while ACTIVE.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

const MAX_NAME_ATTEMPTS: usize = 8;

const SCRIPT: &str = include_str!("routes_dump.rb");
/// The line before the JSON, which separates it from whatever the app's initializers print.
const MARKER: &str = "__ALDER_ROUTES__";

/// The JSON array on the line after the last marker line; `None` without a marker or when it isn't an array.
fn routes_json(stdout: &str) -> Option<serde_json::Value> {
    let lines: Vec<&str> = stdout.lines().collect();
    let at = lines.iter().rposition(|line| *line == MARKER)?;
    serde_json::from_str(lines.get(at + 1)?).ok().filter(serde_json::Value::is_array)
}

fn first_line<'a>(stderr: &'a str, stdout: &'a str) -> &'a str {
    let text = if stderr.is_empty() { stdout } else { stderr };
    text.lines().next().unwrap_or_default()
}

fn drain(mut pipe: impl Read + Send + 'static) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.by_ref().take(MAX_OUTPUT_BYTES).read_to_end(&mut bytes);
        let _ = io::copy(&mut pipe, &mut io::sink());
        let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    receiver
}

/// `<ruby> bin/rails runner routes_dump.rb` in `root`; the reason when it can't run, fails or times out.
pub fn load(root: &Path, ruby: &str) -> Result<Vec<Route>, String> {
    let _active = Active::begin();
    if STOPPING.load(Ordering::SeqCst) {
        return Err("the server is shutting down".into());
    }
    let script = write_script().map_err(|e| format!("writing the runner script: {e}"))?;
    let result = run_runner(root, ruby, &script);
    let _ = std::fs::remove_file(&script);
    result
}

/// The script in a new temp file, which Ruby runs: a file, not the code as an argument, because Windows `.bat` shims can't
/// take the script's quotes. The name is unpredictable and the file is created exclusively (and private on unix), so
/// another user can't plant or swap it.
fn write_script() -> io::Result<PathBuf> {
    static RUNS: AtomicU64 = AtomicU64::new(0);
    for _ in 0..MAX_NAME_ATTEMPTS {
        let random = RandomState::new().build_hasher().finish();
        let name = format!("alder-routes-{}-{}-{random:x}.rb", std::process::id(), RUNS.fetch_add(1, Ordering::Relaxed));
        let script = std::env::temp_dir().join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        match options.open(&script) {
            Ok(mut file) => {
                file.write_all(SCRIPT.as_bytes())?;
                return Ok(script);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "no free file name"))
}

fn run_runner(root: &Path, ruby: &str, script: &Path) -> Result<Vec<Route>, String> {
    let mut child = Command::new(ruby)
        .arg(Path::new("bin").join("rails"))
        .arg("runner")
        .arg(script)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("starting `{ruby}`: {e}"))?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else { return Err("no output pipes".into()) };
    let (stdout, stderr) = (drain(stdout), drain(stderr));
    let deadline = Instant::now() + RUNNER_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if STOPPING.load(Ordering::SeqCst) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the server is shutting down".into());
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("bin/rails runner timed out after {}s", RUNNER_TIMEOUT.as_secs()));
            }
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(e) => return Err(format!("waiting for bin/rails runner: {e}")),
        }
    };
    let stdout = stdout.recv_timeout(OUTPUT_GRACE).unwrap_or_default();
    let stderr = stderr.recv_timeout(OUTPUT_GRACE).unwrap_or_default();
    let reason = first_line(&stderr, &stdout);
    if !status.success() {
        return Err(if reason.is_empty() { format!("bin/rails runner {status}") } else { reason.to_string() });
    }
    let json = routes_json(&stdout).ok_or_else(|| format!("bin/rails runner printed no routes: {reason}"))?;
    Ok(routes::parse(root, &json))
}

#[derive(Default)]
struct Slot {
    running: bool,
    rerun: bool,
    /// Bumped by each request, so a debounced one that a later request superseded does nothing.
    latest: u64,
}

/// Which apps have a load running or pending.
#[derive(Default)]
pub struct Loads(Mutex<HashMap<PathBuf, Slot>>);

impl Loads {
    fn slots(&self) -> MutexGuard<'_, HashMap<PathBuf, Slot>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a load of `root` was ever requested.
    pub fn known(&self, root: &Path) -> bool {
        self.slots().contains_key(root)
    }

    /// Records a request; the returned ticket is the latest until the next request.
    pub fn ask(&self, root: &Path) -> u64 {
        let mut slots = self.slots();
        let slot = slots.entry(root.to_path_buf()).or_default();
        slot.latest += 1;
        slot.latest
    }

    /// Whether the caller runs now; with a run in flight, that run goes once more when it ends.
    fn start(&self, root: &Path, ticket: u64) -> bool {
        let mut slots = self.slots();
        let Some(slot) = slots.get_mut(root).filter(|slot| slot.latest == ticket) else { return false };
        if slot.running {
            slot.rerun = true;
            return false;
        }
        slot.running = true;
        true
    }

    /// Whether another run is due after one ended.
    fn again(&self, root: &Path) -> bool {
        let mut slots = self.slots();
        let Some(slot) = slots.get_mut(root) else { return false };
        slot.running = std::mem::take(&mut slot.rerun);
        slot.running
    }
}

/// Runs `run` for `root` once `delay` has passed with no newer `ask`, never two at once; a request during a run runs it again
/// after. `handle` takes each outcome.
pub async fn load_after<Run, Handle>(loads: &Loads, root: &Path, ticket: u64, delay: Duration, run: Run, mut handle: Handle)
where
    Run: Fn(&Path) -> Result<Vec<Route>, String> + Clone + Send + 'static,
    Handle: FnMut(Result<Vec<Route>, String>),
{
    tokio::time::sleep(delay).await;
    if !loads.start(root, ticket) {
        return;
    }
    loop {
        let (path, run) = (root.to_path_buf(), run.clone());
        let outcome = tokio::task::spawn_blocking(move || run(&path)).await.unwrap_or_else(|e| Err(format!("load task failed: {e}")));
        handle(outcome);
        if !loads.again(root) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    type Outcome = Result<Vec<Route>, String>;

    #[test]
    fn routes_follow_the_last_marker() {
        let out = "Booting\r\n\r\n__ALDER_ROUTES__\r\n[{\"name\":\"users\"}]\r\n";
        assert_eq!(routes_json(out), Some(serde_json::json!([{ "name": "users" }])));
        let twice = "__ALDER_ROUTES__\n[1]\nnoise\n__ALDER_ROUTES__\n[2]\n";
        assert_eq!(routes_json(twice), Some(serde_json::json!([2])));
        assert_eq!(routes_json("Puma starting\n"), None);
        assert_eq!(routes_json("__ALDER_ROUTES__\n{oops"), None);
        assert_eq!(routes_json("__ALDER_ROUTES__\n{\"a\":1}"), None);
        assert_eq!(routes_json("__ALDER_ROUTES__\n"), None);
    }

    #[test]
    fn failures_name_the_first_line() {
        assert_eq!(first_line("boom\nat x", "out"), "boom");
        assert_eq!(first_line("", "only out\nmore"), "only out");
    }

    fn counting_run(runs: &Arc<AtomicUsize>, takes: Duration) -> impl Fn(&Path) -> Outcome + Clone + Send + 'static {
        let runs = Arc::clone(runs);
        move |_| {
            runs.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(takes);
            Ok(vec![])
        }
    }

    async fn request(loads: &Loads, root: &Path, delay: Duration, run: impl Fn(&Path) -> Outcome + Clone + Send + 'static) {
        let ticket = loads.ask(root);
        load_after(loads, root, ticket, delay, run, |_| {}).await;
    }

    #[tokio::test]
    async fn a_burst_of_changes_loads_once() {
        let (loads, runs, root) = (Loads::default(), Arc::new(AtomicUsize::new(0)), PathBuf::from("/app"));
        let run = counting_run(&runs, Duration::ZERO);
        let delay = Duration::from_millis(50);
        tokio::join!(
            request(&loads, &root, delay, run.clone()),
            request(&loads, &root, delay, run.clone()),
            request(&loads, &root, delay, run.clone())
        );
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_change_during_a_run_reruns_once() {
        let (loads, runs, root) = (Loads::default(), Arc::new(AtomicUsize::new(0)), PathBuf::from("/app"));
        let run = counting_run(&runs, Duration::from_millis(150));
        let first = request(&loads, &root, Duration::ZERO, run.clone());
        let during = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            for _ in 0..3 {
                request(&loads, &root, Duration::ZERO, run.clone()).await;
            }
        };
        tokio::join!(first, during);
        assert_eq!(runs.load(Ordering::SeqCst), 2);
        request(&loads, &root, Duration::ZERO, run).await;
        assert_eq!(runs.load(Ordering::SeqCst), 3, "an idle app loads again");
    }

    #[tokio::test]
    async fn apps_load_independently() {
        let (loads, runs) = (Loads::default(), Arc::new(AtomicUsize::new(0)));
        let run = counting_run(&runs, Duration::from_millis(50));
        let (a, b) = (PathBuf::from("/a"), PathBuf::from("/b"));
        tokio::join!(request(&loads, &a, Duration::ZERO, run.clone()), request(&loads, &b, Duration::ZERO, run));
        assert_eq!(runs.load(Ordering::SeqCst), 2);
        assert!(loads.known(&a) && !loads.known(Path::new("/c")));
    }

    #[test]
    fn the_script_is_a_new_private_file_each_time() {
        let (first, second) = (write_script().unwrap(), write_script().unwrap());
        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), SCRIPT);
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&first).unwrap().permissions()) & 0o777, 0o600);
        let _ = (std::fs::remove_file(first), std::fs::remove_file(second));
    }

    #[test]
    fn a_missing_ruby_is_reported() {
        let err = load(&std::env::temp_dir(), "alder-no-such-ruby").unwrap_err();
        assert!(err.starts_with("starting `alder-no-such-ruby`"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_runner_prints_routes_or_fails() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("alder-routes-load-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let rails = dir.join("bin").join("rails");
        let write = |body: &str| {
            std::fs::write(&rails, body).unwrap();
            std::fs::set_permissions(&rails, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        write("echo booting\necho __ALDER_ROUTES__\necho '[{\"name\":\"users\",\"verb\":\"GET\",\"path\":\"/users(.:format)\"}]'\n");
        let routes = load(&dir, "sh").unwrap();
        assert_eq!(routes.len(), 1);
        assert_eq!((routes[0].name.as_str(), routes[0].path.as_str()), ("users", "/users"));
        write("echo 'boot failed' >&2\nexit 1\n");
        assert_eq!(load(&dir, "sh").unwrap_err(), "boot failed");
        write("echo nothing\n");
        assert!(load(&dir, "sh").unwrap_err().contains("printed no routes"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
