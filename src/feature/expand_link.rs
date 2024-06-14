use std::time::Duration;

use anyhow::Result;
use log::{error, info};
use moka::future::{Cache, CacheBuilder};
use regex::Regex;
use serenity::{
    all::{
        Context, CreateAllowedMentions, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
        CreateMessage, GuildId, Message, MessageId, Timestamp,
    },
    model::{channel::GuildChannel, id::ChannelId},
};
use typed_builder::TypedBuilder;

use super::model::Timer;

static LINK_PATTERN: &str = r"https://(?:ptb\.|canary\.)?discord(app)?\.com/channels/(?P<guild_id>\d+)/(?P<channel_id>\d+)/(?P<message_id>\d+)";
const CACHE_AGE: u64 = 60 * 60;

#[derive(Debug)]
pub struct MessageLinkExpandService {
    rgx: Regex,
    guild_channel_cache: Cache<ChannelId, GuildChannel>,
    channel_name_cache: Cache<ChannelId, String>,
}

impl MessageLinkExpandService {
    pub fn new() -> Self {
        Self {
            rgx: Regex::new(LINK_PATTERN).expect("Failed to compile regex"),
            guild_channel_cache: CacheBuilder::new(100)
                .time_to_idle(Duration::from_secs(CACHE_AGE))
                .build(),
            channel_name_cache: CacheBuilder::new(100)
                .time_to_idle(Duration::from_secs(CACHE_AGE))
                .build(),
        }
    }

    fn extract_discord_ids(&self, link: &str) -> Option<DiscordID> {
        self.rgx.captures(link).and_then(|caps| {
            let guild = caps["guild_id"].parse::<u64>().ok()?;
            let channel = caps["channel_id"].parse::<u64>().ok()?;
            let message = caps["message_id"].parse::<u64>().ok()?;

            Some(
                DiscordID::builder()
                    .guild(GuildId::new(guild))
                    .channel(ChannelId::new(channel))
                    .message(MessageId::new(message))
                    .build(),
            )
        })
    }

    async fn get_guild_channel(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Option<GuildChannel> {
        match self.guild_channel_cache.get(&channel_id).await {
            Some(ch) => Some(ch),
            None => {
                let ch = match fetch_guild_channel_info(ctx, guild_id, channel_id).await {
                    Ok(ch) => ch,
                    Err(_) => return None,
                };
                self.guild_channel_cache
                    .insert(channel_id, ch.clone())
                    .await;
                Some(ch)
            }
        }
    }

    async fn get_guild_name(&self, ctx: &Context, id: ChannelId) -> Option<String> {
        match self.channel_name_cache.get(&id).await {
            Some(name) => Some(name),
            None => {
                let name = match id.name(&ctx).await.ok() {
                    Some(name) => name,
                    None => return None,
                };
                self.channel_name_cache.insert(id, name.clone()).await;
                Some(name)
            }
        }
    }
}

impl MessageLinkExpandService {
    pub async fn on(&self, ctx: Context, message: Message) {
        let _t = Timer::new("MessageLinkExpandService::on");

        let ids = match self.extract_discord_ids(&message.content) {
            Some(discord_id) => discord_id,
            None => return,
        };

        let msg_guild_id: u64 = match message.guild_id {
            Some(guild_id) => guild_id.into(),
            None => return,
        };

        // if not the same guild, return
        if ids.guild != msg_guild_id {
            return;
        }

        let src_ch = match self.get_guild_channel(&ctx, ids.guild, ids.channel).await {
            Some(ch) => ch,
            None => return,
        };

        // if citation_channel is nsfw, return
        if src_ch.nsfw {
            return;
        }

        let src_msg = match fetch_message(&ctx, ids.channel, ids.message).await {
            Ok(msg) => msg,
            Err(_) => return,
        };

        // build reply message
        let author = CitationMessageAuthor::builder()
            .name(src_msg.author.name.clone())
            .icon_url(src_msg.author.avatar_url())
            .build();

        let attachment_image_url: Option<String> = if !src_msg.attachments.is_empty() {
            src_msg
                .attachments
                .first()
                .map(|attachment| attachment.clone().url)
        } else {
            None
        };

        let sticker_url: Option<String> = if !src_msg.sticker_items.is_empty() {
            src_msg
                .sticker_items
                .first()
                .map(|sticker| sticker.clone().image_url().unwrap())
        } else {
            None
        };

        let channel_name = match self.get_guild_name(&ctx, ids.channel).await {
            Some(name) => name,
            None => return,
        };

        let citation_message = CitationMessage::builder()
            .content(src_msg.content)
            .author(author)
            .channel_name(channel_name)
            .create_at(src_msg.timestamp)
            .attachment_image_url(attachment_image_url)
            .sticker_image_url(sticker_url)
            .build();

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

        let reply_message = CreateMessage::default()
            .embed(embed)
            .reference_message(&message.clone())
            .allowed_mentions(CreateAllowedMentions::default().replied_user(true));

        match message
            .channel_id
            .send_message(&ctx.http, reply_message)
            .await
        {
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
    match guild_id.channels(&ctx.http).await?.get(&channel_id) {
        Some(channels) => Ok(channels.clone()),
        _ => Err(anyhow::anyhow!("Failed to fetch channel")),
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
    pub guild: GuildId,
    pub channel: ChannelId,
    pub message: MessageId,
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

#[cfg(test)]
mod tests {
    use serenity::all::{ChannelId, GuildId, MessageId};

    use crate::feature::expand_link::DiscordID;

    #[test]
    fn test_extract_discord_id() {
        let service = super::MessageLinkExpandService::new();
        let link = "https://discord.com/channels/123/456/789";
        let result = service.extract_discord_ids(link);
        assert_eq!(
            result,
            Some(DiscordID {
                guild: GuildId::new(123),
                channel: ChannelId::new(456),
                message: MessageId::new(789)
            })
        );
    }
}
