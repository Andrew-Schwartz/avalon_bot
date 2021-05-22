use serde::{Deserialize, Serialize};

use crate::model::guild::Integration;
use crate::model::ids::*;
pub use crate::model::ids::UserId;
use crate::model::ImageFormat;

/// Users in Discord are generally considered the base entity. Users can spawn across the entire
/// platform, be members of guilds, participate in text and voice chat, and much more. Users are
/// separated by a distinction of "bot" vs "normal." Although they are similar, bot users are
/// automated users that are "owned" by another user. Unlike normal users, bot users do not have a
/// limitation on the number of Guilds they can be a part of.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct User {
    /// the user's id
    ///
    /// Required OAuth2 scope: identify
    pub id: UserId,
    /// the user's username, not unique across the platform
    ///
    /// Required OAuth2 scope: identify
    pub username: String,
    /// the user's 4-digit discord-tag
    ///
    /// Required OAuth2 scope: identify
    pub discriminator: String,
    /// the user's avatar hash
    ///
    /// Required OAuth2 scope: identify
    pub avatar: Option<String>,
    /// whether the user belongs to an OAuth2 application
    ///
    /// Required OAuth2 scope: identify
    pub bot: Option<bool>,
    /// whether the user is an Official Discord System user (part of the urgent message system)
    ///
    /// Required OAuth2 scope: identify
    pub system: Option<bool>,
    /// whether the user has two factor enabled on their account
    ///
    /// Required OAuth2 scope: identify
    pub mfa_enabled: Option<bool>,
    /// the user's chosen language option
    ///
    /// Required OAuth2 scope: identify
    pub locale: Option<String>,
    /// whether the email on this account has been verified
    ///
    /// Required OAuth2 scope: email
    pub verified: Option<bool>,
    /// the user's email
    ///
    /// Required OAuth2 scope: email
    pub email: Option<String>,
    /// the flags on a user's account
    ///
    /// Required OAuth2 scope: identify
    pub flags: Option<UserFlags>,
    /// the type of Nitro subscription on a user's account
    ///
    /// Required OAuth2 scope: identify
    pub premium_type: Option<PremiumType>,
    /// the public flags on a user's account
    ///
    /// Required OAuth2 scope: identify
    pub public_flags: Option<UserFlags>,
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Id for User {
    type Id = UserId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl User {
    /// The url where this user's default avatar can be retrieved from Discord. The image will
    /// always be a png and is one of the five default avatars.
    #[allow(clippy::missing_panics_doc)]
    pub fn default_avatar_url(&self) -> String {
        let disc: u16 = self.discriminator.parse()
            .expect("a user's discriminator should be a 4 digit number");
        cdn!("embed/avatars/{}.png", disc % 5)
    }

    /// The url where this user's avatar can be retrieved from Discord, if they have one. The
    /// desired format must be specified by `I`. If `I` is an animated format (currently only
    /// [Gif](crate::model::Gif), the [avatar](User::avatar) must start with `a_` or `None` will be
    /// returned.
    ///
    /// The returned image size can be changed by appending a querystring of `?size=desired_size` to
    /// the URL. Image size can be any power of two between 16 and 4096.
    pub fn avatar_url<I: ImageFormat>(&self) -> Option<String> {
        if I::ANIMATED {
            self.avatar.as_ref().and_then(|avatar| if avatar.starts_with("a_") {
                Some(cdn!("avatars/{}/{}.{}", self.id, avatar, I::EXTENSION))
            } else {
                None
            })
        } else {
            self.avatar.as_ref()
                .map(|avatar| cdn!("avatars/{}/{}.{}", self.id, avatar, I::EXTENSION))
        }
    }
}

pub trait UserMarkupExt: Id<Id=UserId> {
    fn ping(&self) -> String {
        format!("<@{}>", self.id())
    }

    fn ping_nick(&self) -> String {
        format!("<@!{}>", self.id())
    }
}

impl<I: Id<Id=UserId>> UserMarkupExt for I {}

bitflags! {
    pub struct UserFlags: u32 {
        const NONE = 0;
        const DISCORD_EMPLOYEE = 1 << 0;
        const PARTNERED_SERVER_OWNER = 1 << 1;
        const HYPESQUAD_EVENTS = 1 << 2;
        const BUG_HUNTER_LEVEL_1 = 1 << 3;
        const HOUSE_BRAVERY = 1 << 6;
        const HOUSE_BRILIANCE = 1 << 7;
        const HOUSE_BALANCE = 1 << 8;
        const EARLY_SUPPORTER = 1 << 9;
        const TEAM_USER = 1 << 10;
        const SYSTEM = 1 << 12;
        const BUG_HUNTER_LEVEL_2 = 1 << 14;
        const VERIFIED_BOT = 1 << 16;
        const EARLY_VERIFIED_BOT_DEVELOPER = 1 << 17;
    }
}
// #[allow(clippy::use_self)]
serde_bitflag!(UserFlags: u32);

serde_repr! {
    /// Premium types denote the level of premium a user has.
    pub enum PremiumType: u8 {
        None = 0,
        NitroClassic = 1,
        Nitro = 2,
    }
}

/// The connection object that the user has attached.
#[derive(Deserialize, Serialize, Debug)]
pub struct Connection {
    /// id of the connection account
    pub id: String,
    /// the username of the connection account
    pub name: String,
    /// the service of the connection (twitch, youtube)
    #[serde(rename = "type")]
    pub connection_type: String,
    /// whether the connection is revoked
    pub revoked: Option<bool>,
    /// an array of partial server integrations
    pub integrations: Option<Vec<Integration>>,
    /// whether the connection is verified
    pub verified: bool,
    /// whether friend sync is enabled for this connection
    pub friend_sync: bool,
    /// whether activities related to this connection will be shown in presence updates
    pub show_activity: bool,
    /// visibility of this connection
    pub visibility: ConnectionVisibility,
}

serde_repr! {
    pub enum ConnectionVisibility: u8 {
        /// invisible to everyone except the user themselves
        None = 0,
        /// visible to everyone
        Everyone = 1,
    }
}