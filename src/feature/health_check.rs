use log::info;
use serenity::all::Ready;

pub struct HealthCheckService;

impl HealthCheckService {
    pub fn new() -> Self {
        Self
    }
    pub fn on(&self, r: Ready) {
        info!("Discord bot is ready!");

        // display the bot's information
        info!("user name : {}", r.user.name);
        info!("user id : {}", r.user.id);
        info!("bot version : {}", env!("CARGO_PKG_VERSION"));

        // display the bot's guilds
        info!("connected {} guilds", r.guilds.len());
    }
}
