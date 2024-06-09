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

#[cfg(test)]
mod tests {
    #[test]
    fn test_load_discord_token_should_success() {
        std::env::set_var("DISCORD_TOKEN", "test_token");
        let token = super::load_discord_token();
        assert_eq!("test_token", token);
        std::env::remove_var("DISCORD_TOKEN");
    }

    #[test]
    #[should_panic]
    fn test_load_discord_token_should_panic() {
        std::env::remove_var("DISCORD_TOKEN");
        super::load_discord_token();
    }
}
