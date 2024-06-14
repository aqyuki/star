use anyhow::{Context as _, Result};
use serenity::{all::GatewayIntents, client::Client};

use super::handler;

pub struct DiscordClient;

impl DiscordClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(self, token: &str) -> Result<()> {
        let mut client = Client::builder(token, GatewayIntents::all())
            .event_handler(handler::EvHandler::new())
            .await
            .expect("Failed to create Discord client");

        client
            .start()
            .await
            .context("Failed to start Discord client")
    }
}
