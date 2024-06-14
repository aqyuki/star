use serenity::{
    all::EventHandler,
    async_trait,
    client::Context,
    model::{channel::Message, prelude::Ready},
};

use crate::feature::{expand_link::MessageLinkExpandService, health_check::HealthCheckService};

pub struct EvHandler {
    health_check: HealthCheckService,
    expand_message_link: MessageLinkExpandService,
}

impl EvHandler {
    pub fn new() -> Self {
        Self {
            health_check: HealthCheckService::new(),
            expand_message_link: MessageLinkExpandService::new(),
        }
    }
}

#[async_trait]
impl EventHandler for EvHandler {
    async fn ready(&self, _ctx: Context, r: Ready) {
        self.health_check.on(r);
    }
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        self.expand_message_link.on(ctx, msg).await;
    }
}
