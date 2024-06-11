use anyhow::Result;
use log::{debug, error, info, warn};
use regex::Regex;
use serenity::{
    all::{
        CreateAllowedMentions, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateMessage,
        MessageId, Timestamp,
    },
    async_trait,
    client::{Context, EventHandler},
    model::{
        channel::{GuildChannel, Message},
        id::{ChannelId, GuildId},
    },
};
use std::time::Instant;
use typed_builder::TypedBuilder;

static LINK_PATTERN: &str = r"https://(?:ptb\.|canary\.)?discord(app)?\.com/channels/(?P<guild_id>\d+)/(?P<channel_id>\d+)/(?P<message_id>\d+)";

pub struct MessageLinkExpandService {
    rgx: Regex,
    cache: moka::future::Cache<ChannelId, GuildChannel>,
}

impl MessageLinkExpandService {
    pub fn new() -> Self {
        Self {
            rgx: Regex::new(LINK_PATTERN).expect("Failed to compile regex"),
            cache: moka::future::CacheBuilder::new(100)
                .time_to_idle(std::time::Duration::from_secs(60 * 60))
                .build(),
        }
    }

    fn extract_message_link(&self, msg: &str) -> Option<String> {
        let caps = self.rgx.captures(msg);
        caps.map(|caps| caps[0].to_string())
    }

    fn extract_discord_ids(&self, link: &str) -> Option<DiscordID> {
        let caps = self.rgx.captures(link);
        let guild = caps.as_ref().map(|caps| caps["guild_id"].to_string());
        let channel = caps.as_ref().map(|caps| caps["channel_id"].to_string());
        let message = caps.as_ref().map(|caps| caps["message_id"].to_string());

        match (guild, channel, message) {
            (Some(guild), Some(channel), Some(message)) => Some(
                DiscordID::builder()
                    .guild_id(GuildId::new(guild.parse::<u64>().unwrap_or_default()))
                    .channel_id(ChannelId::new(channel.parse::<u64>().unwrap_or_default()))
                    .message_id(MessageId::new(message.parse::<u64>().unwrap_or_default()))
                    .build(),
            ),
            _ => None,
        }
    }
}

#[async_trait]
impl EventHandler for MessageLinkExpandService {
    async fn message(&self, ctx: Context, message: Message) {
        let _timer = Timer::new("Message Link Expand Service");

        if message.author.bot {
            info!("skip message expand because this message is from bot");
            return;
        }

        let result = self.extract_message_link(message.content.as_str());
        let link = match result {
            Some(link) => link,
            None => {
                info!("skip message expand because this message does not contain link");
                return;
            }
        };
        debug!("Extracted link: {}", link);

        let discord_id = match self.extract_discord_ids(&link) {
            Some(discord_id) => discord_id,
            None => {
                warn!("skip message expand because failed to extract discord_id");
                return;
            }
        };
        debug!("Extracted discord_id: {:?}", discord_id);

        let msg_guild_id: u64 = match message.guild_id {
            Some(guild_id) => guild_id.into(),
            None => {
                warn!("skip message expand because failed to extract message guild_id");
                return;
            }
        };
        debug!("Extracted message guild_id: {}", msg_guild_id);

        // if not the same guild, return
        if discord_id.guild_id != msg_guild_id {
            info!("skip message expand because the guild_id({}) is not the same as the message guild_id({})", discord_id.guild_id, msg_guild_id);
            return;
        }

        let citation_channel = match self.cache.get(&discord_id.channel_id).await {
            Some(ch) => ch,
            None => {
                let ch = match fetch_guild_channel_info(
                    &ctx,
                    discord_id.guild_id,
                    discord_id.channel_id,
                )
                .await
                {
                    Ok(ch) => ch,
                    Err(why) => {
                        error!("Failed to fetch channel info: {:?}", why);
                        return;
                    }
                };
                self.cache.insert(discord_id.channel_id, ch.clone()).await;
                ch
            }
        };

        debug!("Fetched channel info: {:?}", citation_channel);

        // if citation_channel is nsfw, return
        if citation_channel.nsfw {
            info!("skip message expand because the channel is nsfw");
            return;
        }

        let target_message =
            match fetch_message(&ctx, discord_id.channel_id, discord_id.message_id).await {
                Ok(msg) => msg,
                Err(why) => {
                    error!("Failed to fetch message: {:?}", why);
                    return;
                }
            };

        // build reply message
        let author = CitationMessageAuthor::builder()
            .name(target_message.author.name.clone())
            .icon_url(target_message.author.avatar_url())
            .build();
        debug!("Author: {:?}", author);

        let attachment_image_url: Option<String> = if !target_message.attachments.is_empty() {
            target_message
                .attachments
                .first()
                .map(|attachment| attachment.clone().url)
        } else {
            None
        };
        debug!("Attachment image url: {:?}", attachment_image_url);

        let sticker_url: Option<String> = if !target_message.sticker_items.is_empty() {
            target_message
                .sticker_items
                .first()
                .map(|sticker| sticker.clone().image_url().unwrap())
        } else {
            None
        };
        debug!("Sticker image url: {:?}", sticker_url);

        let citation_message = CitationMessage::builder()
            .content(target_message.content)
            .author(author)
            .channel_name(target_message.channel_id.name(&ctx).await.unwrap())
            .create_at(target_message.timestamp)
            .attachment_image_url(attachment_image_url)
            .sticker_image_url(sticker_url)
            .build();
        debug!("Citation message: {:?}", citation_message);

        let embed = CreateEmbed::default()
            .description(citation_message.content)
            .color(0x7fffff)
            .timestamp(citation_message.create_at)
            .footer(CreateEmbedFooter::new(citation_message.channel_name))
            .author(
                CreateEmbedAuthor::new(citation_message.author.name)
                    .icon_url(citation_message.author.icon_url.unwrap_or_default()),
            )
            .image(citation_message.attachment_image_url.unwrap_or_default())
            .thumbnail(citation_message.sticker_image_url.unwrap_or_default());
        debug!("Embed: {:?}", embed);

        let mention = CreateAllowedMentions::default().replied_user(true);
        let reply_message = CreateMessage::default()
            .embed(embed)
            .reference_message(&message.clone())
            .allowed_mentions(mention);
        debug!("Reply message: {:?}", reply_message);

        let result = message
            .channel_id
            .send_message(&ctx.http, reply_message)
            .await;
        match result {
            Ok(msg) => info!("Sent citation message: {:?}", msg),
            Err(why) => error!("Failed to send citation message: {:?}", why),
        };
    }
}

async fn fetch_guild_channel_info(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<GuildChannel> {
    if let Some(ch) = guild_id.channels(&ctx.http).await?.get(&channel_id) {
        Ok(ch.clone())
    } else {
        Err(anyhow::anyhow!("Failed to fetch channel"))
    }
}

async fn fetch_message(
    ctx: &Context,
    channel_id: ChannelId,
    message_id: MessageId,
) -> Result<Message> {
    let channel = channel_id.to_channel(&ctx.http).await?.guild().unwrap();
    let message = channel.message(&ctx.http, message_id).await?;
    Ok(message)
}

#[derive(Debug, TypedBuilder, PartialEq)]
struct DiscordID {
    pub guild_id: GuildId,
    pub channel_id: ChannelId,
    pub message_id: MessageId,
}

#[derive(Debug, TypedBuilder)]
struct CitationMessageAuthor {
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, TypedBuilder)]
struct CitationMessage {
    pub content: String,
    pub author: CitationMessageAuthor,
    pub channel_name: String,
    pub create_at: Timestamp,
    pub attachment_image_url: Option<String>,
    pub sticker_image_url: Option<String>,
}

struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    fn new(name: &str) -> Timer {
        Timer {
            start: Instant::now(),
            name: name.to_string(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        info!(
            "{} took {}s {}ms",
            self.name,
            elapsed.as_secs(),
            elapsed.subsec_millis()
        );
    }
}

#[cfg(test)]
mod tests {
    use serenity::all::{ChannelId, GuildId, MessageId};

    #[test]
    fn test_extract_message_link_only_link() {
        let service = super::MessageLinkExpandService::new();
        let msg = "https://discord.com/channels/123/456/789";
        let result = service.extract_message_link(msg);
        assert_eq!(
            result,
            Some("https://discord.com/channels/123/456/789".to_string())
        );
    }

    #[test]
    fn test_extract_message_link_only_message() {
        let service = super::MessageLinkExpandService::new();
        let msg = "Hello, world!";
        let result = service.extract_message_link(msg);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_message_link_return_first() {
        let service = super::MessageLinkExpandService::new();
        let msg = "Hello, world! https://discord.com/channels/123/456/789 https://discord.com/channels/101112/131415/161718";
        let result = service.extract_message_link(msg);
        assert_eq!(
            result,
            Some("https://discord.com/channels/123/456/789".to_string())
        );
    }

    #[test]
    fn test_extract_message_link_discordapp() {
        let service = super::MessageLinkExpandService::new();
        let msg = "https://discordapp.com/channels/123/456/789";
        let result = service.extract_message_link(msg);
        assert_eq!(
            result,
            Some("https://discordapp.com/channels/123/456/789".to_string())
        );
    }

    #[test]
    fn test_extract_discord_id() {
        let service = super::MessageLinkExpandService::new();
        let link = "https://discord.com/channels/123/456/789";
        let result = service.extract_discord_ids(link);
        assert_eq!(
            result,
            Some(super::DiscordID {
                guild_id: GuildId::new(123),
                channel_id: ChannelId::new(456),
                message_id: MessageId::new(789)
            })
        );
    }
}
