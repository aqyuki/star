pub fn load_discord_token() -> String {
    std::env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment variable DISCORD_TOKEN")
}

pub fn init_logger() {
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init()
}
