#[tokio::main]
async fn main() {
    if std::env::args().nth(1).is_some_and(|arg| arg == "--version" || arg == "-V") {
        println!("ruby-templates-ls {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    ruby_templates::serve().await;
}
