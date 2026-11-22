fn main() {
    if std::env::args().nth(1).is_some_and(|arg| arg == "--version" || arg == "-V") {
        println!("ruby-templates-ls {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    // Requests and checks run on the runtime's threads, which tokio would give 2 MiB of stack.
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(ruby_templates::THREAD_STACK).build();
    runtime.expect("the async runtime starts").block_on(ruby_templates::serve());
}
