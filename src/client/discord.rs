use anyhow::{Context as _, Result};
use serenity::{all::GatewayIntents, client::Client};

pub struct DiscordClient;

impl DiscordClient {
    pub async fn run(self, token: &str) -> Result<()> {
        let mut client = Client::builder(token, GatewayIntents::default())
            .await
            .expect("Failed to create Discord client");

        // TODO: Add event handler

        client
            .start()
            .await
            .context("Failed to start Discord client")
    }
}
