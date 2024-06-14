use std::env::{set_var, var};

pub fn load_discord_token() -> String {
    var("DISCORD_TOKEN").expect("Expected a token in the environment variable DISCORD_TOKEN")
}

pub fn init_logger() {
    if var("RUST_LOG").is_err() {
        set_var("RUST_LOG", "info");
    }
    env_logger::init()
}
