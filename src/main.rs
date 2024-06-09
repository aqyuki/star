use anyhow::{Ok, Result};
use log::debug;

mod client;
mod config;
mod service;

fn main() -> Result<()> {
    config::init_logger();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}

async fn async_main() -> Result<()> {
    let token = config::load_discord_token();
    debug!("Discord Token : {}", token);
    let client = client::discord::DiscordClient::new();
    client.run(&token).await?;
    Ok(())
}
