use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::http::model::{Gif, Id, ImageFormat, Png};
use crate::http::model::ids::{EmojiId, RoleId};
use crate::http::model::user::User;
use crate::serde_utils::BoolExt;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Emoji {
    Custom(CustomEmoji),
    // todo:
    //  Unicode(#[serde(rename = "name")] String),
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
            Emoji::Custom(custom) => Some(custom.url()),
            Emoji::Unicode { .. } => None,
        }
    }

    pub fn as_reaction(&self) -> Cow<'_, str> {
        match self {
            Emoji::Custom(CustomEmoji { id, name, animated, .. }) =>
                format!("<{}:{}:{}>", if *animated { "a" } else { "" }, name, id).into(),
            Emoji::Unicode { name } => name.into(),
        }
    }

    pub fn as_custom(&self) -> Option<&CustomEmoji> {
        match self {
            Emoji::Custom(c) => Some(c),
            Emoji::Unicode { .. } => None,
        }
    }

    pub fn as_unicode(&self) -> Option<&str> {
        match self {
            Emoji::Custom(_) => None,
            Emoji::Unicode { name } => Some(name),
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