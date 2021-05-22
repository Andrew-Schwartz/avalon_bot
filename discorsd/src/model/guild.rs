use std::collections::HashSet;

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::cache::IdMap;
use crate::model::{ImageFormat, StillImage};
use crate::model::channel::Channel;
use crate::model::emoji::CustomEmoji;
use crate::model::ids::*;
use crate::model::permissions::{Permissions, Role};
use crate::model::user::User;
use crate::model::voice::VoiceState;
use crate::shard::dispatch::PresenceUpdate;

pub use super::ids::GuildId;

/// Guilds in Discord represent an isolated collection of users and channels, and are often referred to as "servers" in the UI.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Guild {
    /// guild id
    pub id: GuildId,
    /// guild name (2-100 characters, excluding trailing and leading whitespace)
    pub name: Option<String>,
    /// icon hash
    pub icon: Option<String>,
    /// icon hash, returned when in the template object
    pub splash: Option<String>,
    /// discovery splash hash; only present for guilds with the "DISCOVERABLE" feature
    pub discovery_splash: Option<String>,
    // TODO auto fetch these, and handle not overwriting them in the update
    /// true if the user is the owner of the guild
    /// todo link when impl'd
    /// only sent when using the `GET Current User Guilds` endpoint and are relative to the requested user
    #[serde(default)]
    pub owner: bool,
    /// id of owner
    pub owner_id: UserId,
    /// total permissions for the user in the guild (excludes overrides)
    /// todo link when impl'd
    /// only sent when using the `GET Current User Guilds` endpoint and are relative to the requested user
    pub permissions: Option<Permissions>,
    /// voice region id for the guild
    pub region: String,
    /// id of afk channel
    pub afk_channel_id: Option<ChannelId>,
    /// afk timeout in seconds
    pub afk_timeout: u32,
    /// true if the server widget is enabled
    pub widget_enabled: Option<bool>,
    /// the channel id that the widget will generate an invite to, or `None` if set to no invite
    pub widget_channel_id: Option<ChannelId>,
    /// verification level required for the guild
    pub verification_level: VerificationLevel,
    /// default message notifications level
    pub default_message_notifications: NotificationLevel,
    /// explicit content filter level
    pub explicit_content_filter: ExplicitFilterLevel,
    /// roles in the guild
    pub roles: IdMap<Role>,
    /// custom guild emojis
    pub emojis: IdMap<CustomEmoji>,
    /// enabled guild features
    pub features: HashSet<GuildFeature>,
    /// required MFA level for the guild
    pub mfa_level: MfaLevel,
    /// application id of the guild creator if it is bot-created
    pub application_id: Option<ApplicationId>,
    /// the id of the channel where guild notices such as welcome messages and boost events are posted
    pub system_channel_id: Option<ChannelId>,
    /// system channel flags
    pub system_channel_flags: SystemChannelFlags,
    /// the id of the channel where Community guilds can display rules and/or guidelines
    pub rules_channel_id: Option<ChannelId>,
    /// when this guild was joined at
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub joined_at: Option<DateTime<Local>>,
    /// true if this is considered a large guild
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub large: Option<bool>,
    /// true if this guild is unavailable due to an outage
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub unavailable: Option<bool>,
    /// total number of members in this guild
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub member_count: Option<u32>,
    /// states of members currently in voice channels; lacks the guild_id key
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub voice_states: IdMap<VoiceState>,
    /// users in the guild
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub members: IdMap<GuildMember>,
    /// channels in the guild
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub channels: IdMap<Channel>,
    /// presences of the members in the guild, will only include non-offline members if the size is
    /// greater than `large threshold` only sent within the `GUILD_CREATE` event
    ///
    /// only sent within the [GuildCreate](crate::shard::dispatch::GuildCreate) event
    pub presences: IdMap<PresenceUpdate>,
    /// the maximum number of presences for the guild (the default value, currently 25000, is in effect when `null` is returned)
    pub max_presences: Option<u32>,
    /// the maximum number of members for the guild
    pub max_members: Option<u32>,
    /// the vanity url code for the guild
    pub vanity_url_code: Option<String>,
    /// the description for the guild, if the guild is discoverable
    pub description: Option<String>,
    /// banner hash
    pub banner: Option<String>,
    /// premium tier (Server Boost level)
    pub premium_tier: PremiumTier,
    /// the number of boosts this guild currently has
    pub premium_subscription_count: Option<u32>,
    /// the preferred locale of a Community guild; used in server discovery and notices from Discord; defaults to "en-US"
    pub preferred_locale: Option<String>,
    /// the id of the channel where admins and moderators of Community guilds receive notices from Discord
    pub public_updates_id_channel: Option<ChannelId>,
    /// the maximum amount of users in a video channel
    pub max_video_channel_users: Option<u32>,
    /// approximate number of members in this guild, returned from the `GET /guild/<id>` endpoint when `with_counts` is `true`
    pub approximate_member_count: Option<u32>,
    /// approximate number of non-offline members in this guild, returned from the `GET /guild/<id>` endpoint when `with_counts` is `true`
    pub approximate_presence_count: Option<u32>,
}
id_impl!(Guild => id: GuildId);

impl Guild {
    /// The url where this guild's icon can be retrieved from Discord, if it has an icon. The
    /// desired format must be specified by `I`. If `I` is an animated format (currently only
    /// [Gif](crate::model::Gif), the [icon](Guild::icon) must start with `a_` or `None` will be
    /// returned.
    ///
    /// The returned image size can be changed by appending a querystring of `?size=desired_size` to
    /// the URL. Image size can be any power of two between 16 and 4096.
    pub fn icon_url<I: ImageFormat>(&self) -> Option<String> {
        if I::ANIMATED {
            self.icon.as_ref().and_then(|icon| if icon.starts_with("a_") {
                Some(cdn!("icons/{}/{}.{}", self.id, icon, I::EXTENSION))
            } else {
                None
            })
        } else {
            self.icon.as_ref()
                .map(|icon| cdn!("icons/{}/{}.{}", self.id, icon, I::EXTENSION))
        }
    }

    /// The url where this guild's splash can be retrieved from Discord, if it has one
    /// ([`GuildFeature::Discoverable`](GuildFeature::Discoverable) must be one of the guild's
    /// [`features`](Guild::features)). The desired format must be specified by `I`, and can only be
    /// a [`StillImage`](crate::model::StillImage) format.
    ///
    /// The returned image size can be changed by appending a querystring of `?size=desired_size` to
    /// the URL. Image size can be any power of two between 16 and 4096.
    pub fn splash_url<I: StillImage>(&self) -> Option<String> {
        self.splash.as_ref()
            .map(|splash| cdn!("splashes/{}/{}.{}", self.id, splash, I::EXTENSION))
    }

    /// The url where this guild's discovery splash can be retrieved from Discord, if it has one.
    /// The desired format must be specified by `I`, and can only be a
    /// [`StillImage`](crate::model::StillImage) format.
    ///
    /// The returned image size can be changed by appending a querystring of `?size=desired_size` to
    /// the URL. Image size can be any power of two between 16 and 4096.
    pub fn discovery_splash_url<I: StillImage>(&self) -> Option<String> {
        self.discovery_splash.as_ref()
            .map(|splash| cdn!("discovery-splashes/{}/{}.{}", self.id, splash, I::EXTENSION))
    }

    /// The url where this guild's banner can be retrieved from Discord, if it has one. The
    /// desired format must be specified by `I`, and can only be a
    /// [`StillImage`](crate::model::StillImage) format.
    ///
    /// The returned image size can be changed by appending a querystring of `?size=desired_size` to
    /// the URL. Image size can be any power of two between 16 and 4096.
    pub fn banner_url<I: StillImage>(&self) -> Option<String> {
        self.banner.as_ref()
            .map(|banner| cdn!("banners/{}/{}.{}", self.id, banner, I::EXTENSION))
    }
}

serde_repr! {
    pub enum NotificationLevel: u8 {
        AllMessage = 0,
        OnlyMentions = 1,
    }
}

serde_repr! {
    pub enum ExplicitFilterLevel: u8 {
        Disabled = 0,
        MembersWithoutRoles = 1,
        AllMembers = 2,
    }
}

serde_repr! {
    pub enum MfaLevel: u8 {
        None = 0,
        Elevated = 1,
    }
}

serde_repr! {
    pub enum VerificationLevel: u8 {
        /// unrestricted
        None = 0,
        /// must have verified email on account
        Low = 1,
        /// must be registered on Discord for longer than 5 minutes
        Medium = 2,
        /// must be a member of the server for longer than 10 minutes
        High = 3,
        /// must have a verified phone number
        VeryHigh = 4,
    }
}

serde_repr! {
    pub enum PremiumTier: u8 {
        None = 0,
        Tier1 = 1,
        Tier2 = 2,
        Tier3 = 3,
    }
}

bitflags! {
    pub struct SystemChannelFlags: u8 {
        /// Suppress member join notifications
        const SUPPRESS_JOIN_NOTIFICATIONS = 1 << 0;
        /// Suppress server boost notifications
        const SUPPRESS_PREMIUM_SUBSCRIPTIONS = 1 << 1;
    }
}
#[allow(clippy::use_self)]
serde_bitflag!(SystemChannelFlags: u8);

#[derive(Deserialize, Serialize, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum GuildFeature {
    /// guild has access to set an invite splash background
    InviteSplash,
    /// guild has access to set 384kbps bitrate in voice (previously VIP voice servers)
    VipRegions,
    /// guild has access to set a vanity URL
    VanityUrl,
    /// guild is verified
    Verified,
    /// guild is partnered
    Partnered,
    /// guild can enable welcome screen and discovery, and receives community updates
    Community,
    /// guild has access to use commerce features (i.e. create store channels)
    Commerce,
    /// guild has access to create news channels
    News,
    /// guild is lurkable and able to be discovered in the directory
    Discoverable,
    /// guild is able to be featured in the directory
    Featurable,
    /// guild has access to set an animated guild icon
    AnimatedIcon,
    /// guild has access to set a guild banner image
    Banner,
    /// guild has enabled the welcome screen
    WelcomeScreenEnabled,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PartialGuild {
    id: GuildId,
    name: String,
    icon: Option<String>,
    owner: bool,
    permissions: Permissions,
    features: HashSet<GuildFeature>,
}

/// A partial guild object. Represents an Offline Guild, or a Guild whose information has not been
/// provided through [`GuildCreate`](crate::shard::dispatch::GuildCreate)
/// events during the Gateway connect.
///
/// If the `unavailable` field is not set, the user was removed from the guild.
#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Copy, Clone)]
pub struct UnavailableGuild {
    pub id: GuildId,
    pub unavailable: bool,
}

impl Id for UnavailableGuild {
    type Id = GuildId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

// todo?
//  https://discord.com/developers/docs/resources/guild#guild-preview-object
pub struct GuildPreview {
    pub id: GuildId,
}
id_impl!(GuildPreview => GuildId);

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Copy, Clone)]
pub struct GuildWidget {
    /// whether the widget is enabled
    pub enabled: bool,
    /// the widget channel id
    pub channel_id: Option<ChannelId>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GuildMember {
    /// the user this guild member represents
    ///
    /// Partial object, containing `id`, `username`, `avatar`, `discriminator`, and `public_flags`
    pub user: User,
    /// this users guild nickname
    pub nick: Option<String>,
    /// array of role object ids
    pub roles: HashSet<RoleId>,
    /// when the user joined the guild
    pub joined_at: DateTime<Utc>,
    /// when the user started boosting the guild
    pub premium_since: Option<DateTime<Utc>>,
    /// whether the user is deafened in voice channels
    pub deaf: bool,
    /// whether the user is muted in voice channels
    pub mute: bool,
    /// whether the user has passed the guild's Membership Screening requirements
    #[serde(default)]
    pub pending: bool,
}

id_eq!(GuildMember);
impl Id for GuildMember {
    type Id = UserId;

    fn id(&self) -> Self::Id {
        self.user.id
    }
}

impl GuildMember {
    pub fn nick_or_name(&self) -> &str {
        self.nick.as_deref()
            .unwrap_or_else(|| self.user.username.as_str())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GuildMemberUserless {
    /// this users guild nickname
    pub nick: Option<String>,
    /// array of role object ids
    pub roles: Vec<RoleId>,
    /// when the user joined the guild
    pub joined_at: DateTime<Utc>,
    /// when the user started boosting the guild
    pub premium_since: Option<DateTime<Utc>>,
    /// whether the user is deafened in voice channels
    pub deaf: bool,
    /// whether the user is muted in voice channels
    pub mute: bool,
    /// whether the user has passed the guild's Membership Screening requirements
    #[serde(default)]
    pub pending: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Integration {
    /// integration id
    pub id: IntegrationId,
    /// integration name
    pub name: String,
    /// integration type (twitch, youtube, or discord)
    #[serde(rename = "type")]
    pub integration_type: String,
    /// is this integration enabled
    pub enabled: bool,
    /// is this integration syncing
    ///
    /// not provided for discord bot integrations
    pub syncing: Option<bool>,
    /// id that this integration uses for "subscribers"
    ///
    /// not provided for discord bot integrations
    pub role_id: Option<RoleId>,
    /// whether emoticons should be synced for this integration (twitch only currently)
    ///
    /// not provided for discord bot integrations
    pub enable_emoticons: Option<bool>,
    /// the behavior of expiring subscribers
    ///
    /// not provided for discord bot integrations
    pub expire_behavior: Option<ExpireBehavior>,
    /// the grace period (in days) before expiring subscribers
    ///
    /// not provided for discord bot integrations
    pub expire_grace_period: Option<u32>,
    /// user for this integration
    ///
    /// not provided for discord bot integrations
    pub user: Option<User>,
    /// integration account information
    pub account: IntegrationAccount,
    /// when this integration was last synced
    ///
    /// not provided for discord bot integrations
    pub synced_at: Option<DateTime<Utc>>,
    /// how many subscribers this integration has
    ///
    /// not provided for discord bot integrations
    pub subscriber_count: Option<u32>,
    /// has this integration been revoked
    ///
    /// not provided for discord bot integrations
    pub revoked: Option<bool>,
    /// The bot/OAuth2 application for discord integrations
    ///
    /// not provided for discord bot integrations
    pub application: Option<IntegrationApplication>,
}
id_impl!(Integration => IntegrationId);

serde_repr! {
    pub enum ExpireBehavior: u8 {
        RemoveRole = 0,
        Kick = 1,
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IntegrationAccount {
    /// id of the account
    pub id: String,
    /// name of the account
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IntegrationApplication {
    /// the id of the app
    pub id: IntegrationId,
    /// the name of the app
    pub name: String,
    /// the icon hash of the app
    pub icon: Option<String>,
    /// the description of the app
    pub description: String,
    /// the description of the app
    pub summary: String,
    /// the bot associated with this application
    pub bot: Option<User>,
}
id_impl!(IntegrationApplication => IntegrationId);

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Ban {
    /// the reason for the ban
    pub reason: Option<String>,
    /// the banned user
    ///
    /// Partial object, containing `id`, `username`, `avatar`, `discriminator`, and `public_flags`
    /// (I assume its same as in GuildMember)
    pub user: User,
}