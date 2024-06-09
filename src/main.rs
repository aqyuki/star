use anyhow::{Ok, Result};

mod client;
mod config;
mod service;

fn main() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}

async fn async_main() -> Result<()> {
    let token = config::load_discord_token();
    let client = client::discord::DiscordClient::new();
    client.run(&token).await?;
    Ok(())
}
