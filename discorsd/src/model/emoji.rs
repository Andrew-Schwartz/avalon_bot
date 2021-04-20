use std::borrow::Cow;

use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::http::{ClientResult, DiscordClient};
use crate::model::{Gif, ImageFormat, Png};
use crate::model::ids::*;
pub use crate::model::ids::{EmojiId, RoleId};
use crate::model::user::User;
use crate::serde_utils::BoolExt;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Emoji {
    Custom(CustomEmoji),
    Unicode { name: String },
}

impl Emoji {
    /// The url where this image can be retrieved from Discord, if this is a [Custom](Self::Custom)
    /// emoji. Will either be a `.png` or a `.gif`, depending on whether this emoji is
    /// [animated](Emoji::animated).
    ///
    /// The returned image size can be changed by appending a querystring of `?size=desired_size` to
    /// the URL. Image size can be any power of two between 16 and 4096.
    pub fn url(&self) -> Option<String> {
        match self {
            Self::Custom(custom) => Some(custom.url()),
            Self::Unicode { .. } => None,
        }
    }

    pub fn as_reaction(&self) -> Cow<'_, str> {
        match self {
            Self::Custom(CustomEmoji { id, name, animated, .. }) =>
                format!("<{}:{}:{}>", if *animated { "a" } else { "" }, name, id).into(),
            Self::Unicode { name } => name.into(),
        }
    }

    pub const fn as_custom(&self) -> Option<&CustomEmoji> {
        match self {
            Self::Custom(c) => Some(c),
            Self::Unicode { .. } => None,
        }
    }

    pub fn as_unicode(&self) -> Option<&str> {
        match self {
            Self::Custom(_) => None,
            Self::Unicode { name } => Some(name),
        }
    }
}

/// Represents an emoji as shown in the Discord client.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CustomEmoji {
    /// emoji id
    pub id: EmojiId,
    /// emoji name
    ///
    /// (can be null only in reaction emoji objects)
    pub name: String,
    /// roles this emoji is whitelisted to
    #[serde(default = "Vec::new")]
    pub roles: Vec<RoleId>,
    /// user that created this emoji
    pub user: Option<User>,
    /// whether this emoji must be wrapped in colons
    #[serde(skip_serializing_if = "bool::is_false")]
    #[serde(default)]
    pub require_colons: bool,
    /// whether this emoji is managed
    #[serde(skip_serializing_if = "bool::is_false")]
    #[serde(default)]
    pub managed: bool,
    /// whether this emoji is animated
    #[serde(skip_serializing_if = "bool::is_false")]
    #[serde(default)]
    pub animated: bool,
    /// whether this emoji can be used, may be false due to loss of Server Boosts
    #[serde(skip_serializing_if = "bool::is_false")]
    #[serde(default)]
    pub available: bool,
}

impl Id for CustomEmoji {
    type Id = EmojiId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl PartialEq for CustomEmoji {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl CustomEmoji {
    pub fn new(id: EmojiId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            roles: vec![],
            user: None,
            require_colons: false,
            managed: false,
            animated: false,
            available: false,
        }
    }

    /// The url where this image can be retrieved from Discord. Will either be a `.png` or a `.gif`,
    /// depending on whether this emoji is [animated](Emoji::animated)
    ///
    /// The returned image size can be changed by appending a querystring of `?size=desired_size` to
    /// the URL. Image size can be any power of two between 16 and 4096.
    pub fn url(&self) -> String {
        let ext = if self.animated {
            Gif::EXTENSION
        } else {
            Png::EXTENSION
        };
        cdn!("emojis/{}.{}", self.id, ext)
    }
}

impl From<CustomEmoji> for Emoji {
    fn from(custom: CustomEmoji) -> Self {
        Self::Custom(custom)
    }
}

impl From<String> for Emoji {
    fn from(name: String) -> Self {
        Self::Unicode { name }
    }
}

impl From<char> for Emoji {
    fn from(name: char) -> Self {
        Self::Unicode { name: name.to_string() }
    }
}

impl DiscordClient {
    /// For each emoji in [emojis](emojis) in order, get the users who reacted with that emoji to
    /// this message.
    pub async fn get_all_reactions<Emojis, E>(
        &self,
        channel: ChannelId,
        message: MessageId,
        emojis: Emojis,
    ) -> ClientResult<Vec<Vec<User>>>
        where
            Emojis: IntoIterator<Item=E> + Send,
            <Emojis as IntoIterator>::IntoIter: Send,
            E: Into<Emoji>,
    {
        futures::stream::iter(emojis)
            .then(|emoji| self.get_reactions(channel, message, emoji.into()))
            .try_collect()
            .await
    }
}