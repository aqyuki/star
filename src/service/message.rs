use anyhow::Result;
use log::{debug, error, info, warn};
use regex::Regex;
use serenity::{
    all::{
        CreateAllowedMentions, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateMessage,
        Timestamp,
    },
    async_trait,
    client::{Context, EventHandler},
    model::{
        channel::{GuildChannel, Message},
        id::{ChannelId, GuildId},
    },
};
use typed_builder::TypedBuilder;

static LINK_PATTERN: &str = r"https://(?:ptb\.|canary\.)?discord(app)?\.com/channels/(?P<guild_id>\d+)/(?P<channel_id>\d+)/(?P<message_id>\d+)";

pub struct MessageLinkExpandService {
    rgx: Regex,
}

impl MessageLinkExpandService {
    pub fn new() -> Self {
        Self {
            rgx: Regex::new(LINK_PATTERN).expect("Failed to compile regex"),
        }
    }

    fn extract_message_link(&self, msg: &str) -> Option<String> {
        let caps = self.rgx.captures(msg);
        caps.map(|caps| caps[0].to_string())
    }

    fn extract_guild_id(&self, link: &str) -> Option<String> {
        let caps = self.rgx.captures(link);
        caps.map(|caps| caps["guild_id"].to_string())
    }
    fn extract_channel_id(&self, link: &str) -> Option<String> {
        let caps = self.rgx.captures(link);
        caps.map(|caps| caps["channel_id"].to_string())
    }
    fn extract_message_id(&self, link: &str) -> Option<String> {
        let caps = self.rgx.captures(link);
        caps.map(|caps| caps["message_id"].to_string())
    }
}

#[async_trait]
impl EventHandler for MessageLinkExpandService {
    async fn message(&self, ctx: Context, message: Message) {
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

        let guild_id = match self.extract_guild_id(&link) {
            Some(guild_id) => guild_id,
            None => {
                warn!("skip message expand because failed to extract guild_id");
                return;
            }
        };
        debug!("Extracted guild_id: {}", guild_id);

        let msg_guild_id = match message.guild_id {
            Some(guild_id) => guild_id.to_string(),
            None => {
                warn!("skip message expand because failed to extract message guild_id");
                return;
            }
        };
        debug!("Extracted message guild_id: {}", msg_guild_id);

        // if not the same guild, return
        if guild_id != msg_guild_id {
            info!("skip message expand because the guild_id({}) is not the same as the message guild_id({})", guild_id, msg_guild_id);
            return;
        }

        let channel_id = match self.extract_channel_id(&link) {
            Some(channel_id) => channel_id.parse::<u64>().unwrap(),
            None => {
                warn!("skip message expand because failed to extract channel_id");
                return;
            }
        };
        debug!("Extracted channel_id: {}", channel_id);

        let parsed_guild_id = guild_id.parse::<u64>().unwrap();
        debug!("Parsed guild_id: {}", parsed_guild_id);

        let citation_channel =
            match fetch_guild_channel_info(&ctx, parsed_guild_id, channel_id).await {
                Ok(ch) => ch,
                Err(why) => {
                    error!("Failed to fetch channel info: {:?}", why);
                    return;
                }
            };
        debug!("Fetched channel info: {:?}", citation_channel);

        // if citation_channel is nsfw, return
        if citation_channel.nsfw {
            info!("skip message expand because the channel is nsfw");
            return;
        }

        let message_id = match self.extract_message_id(&link) {
            Some(message_id) => message_id.parse::<u64>().unwrap(),
            None => {
                warn!("skip message expand because failed to extract message_id");
                return;
            }
        };
        debug!("Extracted message_id: {}", message_id);

        let target_message = match fetch_message(&ctx, channel_id, message_id).await {
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
    raw_guild_id: u64,
    raw_channel_id: u64,
) -> Result<GuildChannel> {
    let guild_id = GuildId::new(raw_guild_id);
    let channel_id = ChannelId::new(raw_channel_id);
    if let Some(ch) = guild_id.channels(&ctx.http).await?.get(&channel_id) {
        Ok(ch.clone())
    } else {
        Err(anyhow::anyhow!("Failed to fetch channel"))
    }
}

async fn fetch_message(ctx: &Context, raw_channel_id: u64, raw_message_id: u64) -> Result<Message> {
    let channel_id = ChannelId::new(raw_channel_id);
    let message_id = raw_message_id;
    let channel = channel_id.to_channel(&ctx.http).await?.guild().unwrap();
    let message = channel.message(&ctx.http, message_id).await?;
    Ok(message)
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
    fn test_extract_guild_id() {
        let service = super::MessageLinkExpandService::new();
        let link = "https://discord.com/channels/123/456/789";
        let result = service.extract_guild_id(link);
        assert_eq!(result, Some("123".to_string()));
    }

    #[test]
    fn test_extract_channel_id() {
        let service = super::MessageLinkExpandService::new();
        let link = "https://discord.com/channels/123/456/789";
        let result = service.extract_channel_id(link);
        assert_eq!(result, Some("456".to_string()));
    }

    #[test]
    fn test_extract_message_id() {
        let service = super::MessageLinkExpandService::new();
        let link = "https://discord.com/channels/123/456/789";
        let result = service.extract_message_id(link);
        assert_eq!(result, Some("789".to_string()));
    }
}
