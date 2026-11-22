fn main() {
    if std::env::args().nth(1).is_some_and(|arg| arg == "--version" || arg == "-V") {
        println!("alder-ls {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    // Some requests run on the thread that drives the server, whose stack on Windows is 1 MiB, and the rest on the
    // runtime's threads, which tokio would give 2 MiB.
    let serve = || {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(alder::THREAD_STACK).build();
        runtime.expect("the async runtime starts").block_on(alder::serve());
    };
    let server = std::thread::Builder::new().stack_size(alder::THREAD_STACK).spawn(serve).expect("the server thread starts");
    if let Err(panic) = server.join() {
        std::panic::resume_unwind(panic);
    }
}
