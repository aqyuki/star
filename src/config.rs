pub fn load_discord_token() -> String {
    std::env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment variable DISCORD_TOKEN")
}
