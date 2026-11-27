use std::fs;

use zed_extension_api::{
    self as zed, Architecture, DownloadedFileType, LanguageServerId, LanguageServerInstallationStatus, Os, Result, Worktree,
};

const SERVER: &str = "alder-ls";
const REPOSITORY: &str = "x-ror/alder";
const DOWNLOAD_DIR_PREFIX: &str = "alder-ls-";
const SETUP_HINT: &str = "put `alder-ls` on your PATH or set `lsp.alder-ls.binary.path` in your Zed settings";

#[derive(Debug, PartialEq)]
struct ReleaseAsset {
    name: &'static str,
    file_type: DownloadedFileType,
    binary: &'static str,
}

/// The release archive CI publishes for this platform, if any.
fn release_asset(os: Os, arch: Architecture) -> Option<ReleaseAsset> {
    let (name, file_type, binary) = match (os, arch) {
        (Os::Linux, Architecture::X8664) => ("alder-ls-linux-x64.tar.gz", DownloadedFileType::GzipTar, SERVER),
        (Os::Linux, Architecture::Aarch64) => ("alder-ls-linux-arm64.tar.gz", DownloadedFileType::GzipTar, SERVER),
        (Os::Mac, Architecture::X8664) => ("alder-ls-darwin-x64.tar.gz", DownloadedFileType::GzipTar, SERVER),
        (Os::Mac, Architecture::Aarch64) => ("alder-ls-darwin-arm64.tar.gz", DownloadedFileType::GzipTar, SERVER),
        (Os::Windows, Architecture::X8664) => ("alder-ls-win32-x64.zip", DownloadedFileType::Zip, "alder-ls.exe"),
        _ => return None,
    };
    Some(ReleaseAsset { name, file_type, binary })
}

fn is_file(path: &str) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// Each release is unpacked into `alder-ls-<tag>/` inside the extension's work directory.
fn downloaded_dirs() -> impl Iterator<Item = String> {
    fs::read_dir(".")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(DOWNLOAD_DIR_PREFIX))
}

fn find_downloaded(binary: &str) -> Option<String> {
    downloaded_dirs().map(|dir| format!("{dir}/{binary}")).find(|path| is_file(path))
}

fn install_latest(language_server_id: &LanguageServerId, asset: &ReleaseAsset) -> Result<String> {
    zed::set_language_server_installation_status(language_server_id, &LanguageServerInstallationStatus::CheckingForUpdate);
    let release = zed::latest_github_release(REPOSITORY, zed::GithubReleaseOptions { require_assets: true, pre_release: false })
        .map_err(|err| format!("could not look up the latest {REPOSITORY} release ({err}); {SETUP_HINT}"))?;

    let version_dir = format!("{DOWNLOAD_DIR_PREFIX}{}", release.version);
    let binary_path = format!("{version_dir}/{}", asset.binary);
    if is_file(&binary_path) {
        return Ok(binary_path);
    }

    let download = release
        .assets
        .iter()
        .find(|candidate| candidate.name == asset.name)
        .ok_or_else(|| format!("release {} of {REPOSITORY} has no {} download; {SETUP_HINT}", release.version, asset.name))?;

    zed::set_language_server_installation_status(language_server_id, &LanguageServerInstallationStatus::Downloading);
    let unpacked = zed::download_file(&download.download_url, &version_dir, asset.file_type)
        .and_then(|()| zed::make_file_executable(&binary_path))
        .and_then(|()| if is_file(&binary_path) { Ok(()) } else { Err(format!("{} does not contain {}", asset.name, asset.binary)) });
    if let Err(err) = unpacked {
        fs::remove_dir_all(&version_dir).ok();
        return Err(format!("failed to install {SERVER} {}: {err}", release.version));
    }

    for old_dir in downloaded_dirs().filter(|dir| *dir != version_dir) {
        fs::remove_dir_all(old_dir).ok();
    }
    Ok(binary_path)
}

struct AlderExtension {
    cached_binary: Option<String>,
}

impl AlderExtension {
    fn server_binary(&mut self, language_server_id: &LanguageServerId, worktree: &Worktree) -> Result<String> {
        if let Some(path) = worktree.which(SERVER) {
            return Ok(path);
        }
        if let Some(path) = self.cached_binary.as_ref().filter(|path| is_file(path)) {
            return Ok(path.clone());
        }

        let (os, arch) = zed::current_platform();
        let asset = release_asset(os, arch).ok_or_else(|| format!("no prebuilt {SERVER} for {os:?} {arch:?}; {SETUP_HINT}"))?;
        // Offline, or a release without our asset: keep using the copy downloaded earlier.
        let path = install_latest(language_server_id, &asset).or_else(|err| find_downloaded(asset.binary).ok_or(err))?;
        self.cached_binary = Some(path.clone());
        Ok(path)
    }
}

impl zed::Extension for AlderExtension {
    fn new() -> Self {
        Self { cached_binary: None }
    }

    fn language_server_command(&mut self, language_server_id: &LanguageServerId, worktree: &Worktree) -> Result<zed::Command> {
        Ok(zed::Command { command: self.server_binary(language_server_id, worktree)?, args: Vec::new(), env: Vec::new() })
    }
}

zed::register_extension!(AlderExtension);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_asset_per_platform() {
        let cases = [
            (Os::Linux, Architecture::X8664, "alder-ls-linux-x64.tar.gz", "alder-ls"),
            (Os::Linux, Architecture::Aarch64, "alder-ls-linux-arm64.tar.gz", "alder-ls"),
            (Os::Mac, Architecture::X8664, "alder-ls-darwin-x64.tar.gz", "alder-ls"),
            (Os::Mac, Architecture::Aarch64, "alder-ls-darwin-arm64.tar.gz", "alder-ls"),
            (Os::Windows, Architecture::X8664, "alder-ls-win32-x64.zip", "alder-ls.exe"),
        ];
        for (os, arch, name, binary) in cases {
            let asset = release_asset(os, arch).unwrap_or_else(|| panic!("no asset for {os:?} {arch:?}"));
            assert_eq!((asset.name, asset.binary), (name, binary));
            let archive = if os == Os::Windows { DownloadedFileType::Zip } else { DownloadedFileType::GzipTar };
            assert_eq!(asset.file_type, archive);
        }
    }

    #[test]
    fn release_asset_unsupported_platforms() {
        assert_eq!(release_asset(Os::Windows, Architecture::Aarch64), None);
        assert_eq!(release_asset(Os::Linux, Architecture::X86), None);
        assert_eq!(release_asset(Os::Windows, Architecture::X86), None);
    }
}
