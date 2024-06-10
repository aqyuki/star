use log::info;
use serenity::{async_trait, client::EventHandler};

pub struct ReadyService;

impl ReadyService {
    pub fn new() -> Self {
        ReadyService
    }
}

#[async_trait]
impl EventHandler for ReadyService {
    async fn ready(&self, _: serenity::client::Context, ready: serenity::model::gateway::Ready) {
        info!("{} is connected", ready.user.name)
    }
}
