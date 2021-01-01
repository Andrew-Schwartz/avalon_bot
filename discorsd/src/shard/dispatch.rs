use std::collections::hash_map::Entry;
use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use log::info;
use serde::{Deserialize, Serialize};

use crate::cache::{Cache, IdMap};
use crate::cache::update::Update;
use crate::http::model::*;
use crate::http::model::channel::Channel;
use crate::http::model::emoji::Emoji;
use crate::http::model::guild::{Guild, GuildMember, UnavailableGuild};
use crate::http::model::ids::{ChannelId, GuildId, MessageId, RoleId, UserId, WebhookId};
use crate::http::model::permissions::Role;
use crate::http::model::user::User;
use crate::http::model::voice::VoiceState;
use crate::shard::model::{Activity, StatusType};

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "t", content = "d")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum DispatchPayload {
    // Connection
    Ready(Ready),
    Resumed(Resumed),

    // Channels
    ChannelCreate(ChannelCreate),
    ChannelUpdate(ChannelUpdate),
    ChannelDelete(ChannelDelete),
    ChannelPinsUpdate(ChannelPinsUpdate),

    // Guilds
    GuildCreate(GuildCreate),
    GuildUpdate(GuildUpdate),
    GuildDelete(GuildDelete),
    GuildBanAdd(BanAdd),
    GuildBanRemove(BanRemove),
    GuildEmojisUpdate(EmojiUpdate),
    GuildIntegrationsUpdate(IntegrationsUpdate),
    IntegrationUpdate(IntegrationUpdate),
    GuildMemberAdd(GuildMemberAdd),
    GuildMemberRemove(GuildMemberRemove),
    GuildMemberUpdate(GuildMemberUpdate),
    GuildMembersChunk(GuildMembersChunk),
    GuildRoleCreate(GuildRoleCreate),
    GuildRoleUpdate(GuildRoleUpdate),
    GuildRoleRemove(GuildRoleDelete),

    // Invites
    InviteCreate(InviteCreate),
    InviteDelete(InviteDelete),

    // Messages
    MessageCreate(MessageCreate),
    // clippy says to box this because its a big boi... meh?
    MessageUpdate(MessageUpdate),
    MessageDelete(MessageDelete),
    MessageDeleteBulk(MessageDeleteBulk),
    MessageReactionAdd(ReactionAdd),
    MessageReactionRemove(ReactionRemove),
    MessageReactionRemoveAll(ReactionRemoveAll),
    MessageReactionRemoveEmoji(ReactionRemoveEmoji),

    // Presence
    PresenceUpdate(PresenceUpdate),
    TypingStart(TypingStart),
    UserUpdate(UserUpdate),

    // Voice
    VoiceStateUpdate(VoiceStateUpdate),
    VoiceServerUpdate(VoiceServerUpdate),

    // Webhooks
    WebhooksUpdate(WebhookUpdate),

    // Slash Commands!
    InteractionCreate(InteractionCreate),
    ApplicationCommandCreate(ApplicationCommandCreate),
    ApplicationCommandUpdate(ApplicationCommandUpdate),
    ApplicationCommandDelete(ApplicationCommandDelete),
}

#[async_trait]
impl Update for DispatchPayload {
    async fn update(self, cache: &Cache) {
        use DispatchPayload::*;
        match self {
            Ready(ready) => ready.update(cache).await,
            Resumed(resumed) => resumed.update(cache).await,
            ChannelCreate(create) => create.update(cache).await,
            ChannelUpdate(update) => update.update(cache).await,
            ChannelDelete(delete) => delete.update(cache).await,
            ChannelPinsUpdate(pins_update) => pins_update.update(cache).await,
            GuildCreate(create) => create.update(cache).await,
            GuildUpdate(update) => update.update(cache).await,
            GuildDelete(delete) => delete.update(cache).await,
            GuildBanAdd(ban_add) => ban_add.update(cache).await,
            GuildBanRemove(ban_remove) => ban_remove.update(cache).await,
            GuildEmojisUpdate(emojis_update) => emojis_update.update(cache).await,
            GuildIntegrationsUpdate(integrations) => integrations.update(cache).await,
            IntegrationUpdate(update) => update.update(cache).await,
            GuildMemberAdd(member_add) => member_add.update(cache).await,
            GuildMemberRemove(member_remove) => member_remove.update(cache).await,
            GuildMemberUpdate(member_update) => member_update.update(cache).await,
            GuildMembersChunk(members_chunk) => members_chunk.update(cache).await,
            GuildRoleCreate(role_create) => role_create.update(cache).await,
            GuildRoleUpdate(role_update) => role_update.update(cache).await,
            GuildRoleRemove(role_remove) => role_remove.update(cache).await,
            InviteCreate(invite_create) => invite_create.update(cache).await,
            InviteDelete(invite_delete) => invite_delete.update(cache).await,
            MessageCreate(message_create) => message_create.update(cache).await,
            MessageUpdate(message_update) => message_update.update(cache).await,
            MessageDelete(message_delete) => message_delete.update(cache).await,
            MessageDeleteBulk(message_delete_bulk) => message_delete_bulk.update(cache).await,
            MessageReactionAdd(message_reaction_add) => message_reaction_add.update(cache).await,
            MessageReactionRemove(message_reaction_remove) => message_reaction_remove.update(cache).await,
            MessageReactionRemoveAll(message_reaction_remove_all) => message_reaction_remove_all.update(cache).await,
            MessageReactionRemoveEmoji(message_reaction_remove_emoji) => message_reaction_remove_emoji.update(cache).await,
            PresenceUpdate(presence_update) => presence_update.update(cache).await,
            TypingStart(typing_start) => typing_start.update(cache).await,
            UserUpdate(user_update) => user_update.update(cache).await,
            VoiceStateUpdate(voice_state_update) => voice_state_update.update(cache).await,
            VoiceServerUpdate(voice_server_update) => voice_server_update.update(cache).await,
            WebhooksUpdate(webhooks_update) => webhooks_update.update(cache).await,
            InteractionCreate(interactions) => interactions.update(cache).await,
            ApplicationCommandCreate(create) => create.update(cache).await,
            ApplicationCommandUpdate(update) => update.update(cache).await,
            ApplicationCommandDelete(delete) => delete.update(cache).await,
        };
    }
}

// Connection events

/// The ready event is dispatched when a client has completed the initial handshake with the gateway
/// (for new sessions). The ready event can be the largest and most complex event the gateway will
/// send, as it contains all the state required for a client to begin interacting with the rest of
/// the platform.
///
/// `guilds` are the guilds of which your bot is a member. They start out as unavailable when you
/// connect to the gateway. As they become available, your bot will be notified via
/// [DispatchPayload::GuildCreate] events. `private_channels` will be an empty array. As bots
/// receive private messages, they will be notified via [DispatchPayload::ChannelCreate] events.
#[derive(Deserialize, Debug, Clone)]
#[non_exhaustive]
pub struct Ready {
    /// gateway version
    pub v: u8,
    /// information about the user including email
    pub user: User,
    /// the guilds the user is in
    pub guilds: Vec<UnavailableGuild>,
    /// used for resuming connections
    pub session_id: String,
    /// the shard information associated with this session, if sent when identifying
    pub shard: Option<(u64, u64)>,
}

#[async_trait]
impl Update for Ready {
    async fn update(self, cache: &Cache) {
        *cache.user.write().await = Some(self.user);
        cache.unavailable_guilds.write().await.extend(self.guilds)
    }
}

/// The resumed event is dispatched when a client has sent a resume payload to the gateway
/// (for resuming existing sessions).
#[derive(Deserialize, Debug, Clone)]
pub struct Resumed {
    _trace: Vec<serde_json::Value>
}

#[async_trait]
impl Update for Resumed {
    async fn update(self, _cache: &Cache) {
        // don't think we need to update anything here
    }
}

// Channel Events

#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct ChannelCreate {
    channel: Channel,
}

#[async_trait]
impl Update for ChannelCreate {
    async fn update(self, cache: &Cache) {
        info!("create = {:?}", &self);
        let channel = self.channel;
        if let Some(guild) = channel.guild_id() {
            cache.guilds.write().await
                .entry(guild)
                .and_modify(|guild| guild.channels.insert(channel.clone()));
        }
        cache.channel_types.write().await.insert(channel.id(), channel.channel_type());
        match channel {
            Channel::Text(text) => {
                cache.channels.write().await.insert(text);
            }
            Channel::Dm(dm) => {
                let (by_user, by_id) = &mut *cache.dms.write().await;
                by_user.insert(dm.recipient.id, dm.id);
                by_id.insert(dm);
            }
            Channel::Voice(_) => {
                // voice not implemented yet (ever)
            }
            Channel::GroupDm(_) => unreachable!("Bots cannot be in GroupDm channels"),
            Channel::Category(category) => {
                cache.categories.write().await.insert(category);
            }
            Channel::News(news) => {
                cache.news.write().await.insert(news);
            }
            Channel::Store(store) => {
                cache.stores.write().await.insert(store)
            }
        };
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct ChannelUpdate {
    channel: Channel
}

#[async_trait]
impl Update for ChannelUpdate {
    async fn update(self, cache: &Cache) {
        let channel = self.channel;
        if let Some(guild) = channel.guild_id() {
            cache.guilds.write().await
                .entry(guild)
                .and_modify(|guild| {
                    guild.channels.entry(channel.id())
                        .and_modify(|old| *old = channel.clone());
                });
        }
        match channel {
            Channel::Text(channel) => {
                if let Some(text) = cache.channels.write().await.get_mut(&channel) {
                    *text = channel;
                }
            }
            Channel::Dm(channel) => {
                let (_, by_channel) = &mut *cache.dms.write().await;
                if let Some(dm) = by_channel.get_mut(&channel) {
                    *dm = channel;
                }
            }
            Channel::Voice(_) => {
                // voice not implemented yet (ever)
            }
            Channel::GroupDm(_) => unreachable!("Bots cannot be in GroupDm channels"),
            Channel::Category(channel) => {
                if let Some(category) = cache.categories.write().await.get_mut(&channel) {
                    *category = channel;
                }
            }
            Channel::News(channel) => {
                if let Some(news) = cache.news.write().await.get_mut(&channel) {
                    *news = channel;
                }
            }
            Channel::Store(channel) => {
                if let Some(store) = cache.stores.write().await.get_mut(&channel) {
                    *store = channel;
                }
            }
        };
    }
}

/// Sent when a channel relevant to the current user is deleted.
#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct ChannelDelete {
    channel: Channel
}

#[async_trait]
impl Update for ChannelDelete {
    async fn update(self, cache: &Cache) {
        cache.channel_types.write().await.remove(&self.channel.id());
        match self.channel {
            Channel::Text(text) => cache.channels.write().await.remove(text),
            Channel::Dm(dm) => {
                let (by_user, by_channel) = &mut *cache.dms.write().await;
                by_user.remove(&dm.recipient.id);
                by_channel.remove(dm);
            }
            Channel::Category(cat) => cache.categories.write().await.remove(cat),
            Channel::News(news) => cache.news.write().await.remove(news),
            Channel::Store(store) => cache.stores.write().await.remove(store),
            Channel::Voice(_) | Channel::GroupDm(_) => {}
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct ChannelPinsUpdate {
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the time at which the most recent pinned message was pinned
    pub last_pin_timestamp: Option<DateTime<Utc>>,
}

#[async_trait]
impl Update for ChannelPinsUpdate {
    async fn update(self, cache: &Cache) {
        use ChannelType::*;
        let Self { guild_id, channel_id, last_pin_timestamp } = self;
        match cache.channel_types.read().await.get(&channel_id) {
            Some(GuildText) => {
                cache.channels.write().await.entry(&channel_id)
                    .and_modify(|channel| {
                        channel.last_pin_timestamp = last_pin_timestamp;
                    });
            }
            Some(Dm) => {
                cache.dms.write().await.1.entry(&channel_id)
                    .and_modify(|channel| {
                        channel.last_pin_timestamp = last_pin_timestamp;
                    });
            }
            Some(GuildNews) => {
                cache.news.write().await.entry(&channel_id)
                    .and_modify(|channel| {
                        channel.last_pin_timestamp = last_pin_timestamp;
                    });
            }
            Some(GuildVoice) | Some(GuildStore) | Some(GuildCategory) => {}
            Some(GroupDm) | None => {}
        }
        if let Some(guild_id) = guild_id {
            cache.guilds.write().await.entry(guild_id)
                .and_modify(|guild| {
                    guild.channels.entry(&channel_id)
                        .and_modify(|channel| match channel {
                            Channel::Text(channel) => channel.last_pin_timestamp = last_pin_timestamp,
                            Channel::News(channel) => channel.last_pin_timestamp = last_pin_timestamp,
                            // no last timestamp
                            Channel::Voice(_) | Channel::Store(_) | Channel::Category(_) => {}
                            // not in a guild
                            Channel::Dm(_) | Channel::GroupDm(_) => {}
                        });
                });
        }
    }
}

// Guild Events

/// This event can be sent in three different scenarios:
/// 1. When a user is initially connecting, to lazily load and backfill information for all
/// unavailable guilds sent in the [Ready](Ready) event. Guilds that are unavailable due to an outage will
/// send a [GuildDelete](GuildDelete) event.
/// 2. When a [Guild]('Guild') becomes available again to the client.
/// 3. When the current user joins a new Guild.
/// The inner payload is a [Guild]('Guild'), with all the extra fields specified.
///
/// ['Guild']: Guild
#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct GuildCreate {
    pub(crate) guild: Guild,
}

#[async_trait]
impl Update for GuildCreate {
    async fn update(self, cache: &Cache) {
        let (mut t, mut c, mut n, mut s) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        {
            let mut guard = cache.channel_types.write().await;
            self.guild.channels.iter()
                .for_each(|channel| {
                    guard.insert(channel.id(), channel.channel_type());
                    match channel {
                        Channel::Text(text) => t.push(text.clone()),
                        Channel::Category(category) => c.push(category.clone()),
                        Channel::News(news) => n.push(news.clone()),
                        Channel::Store(store) => s.push(store.clone()),
                        Channel::Voice(_) => {
                            // not (yet/ever) implemented
                        }
                        Channel::Dm(_) | Channel::GroupDm(_) => {
                            // not part of a guild
                        }
                    }
                });
        }
        cache.channels.write().await.extend(t);
        cache.categories.write().await.extend(c);
        cache.news.write().await.extend(n);
        cache.stores.write().await.extend(s);

        let mut members = cache.members.write().await;
        for member in self.guild.members.iter().cloned() {
            let user_id = member.user.id;
            members.entry(user_id)
                .or_default()
                .insert(self.guild.id, member);
        }
        cache.unavailable_guilds.write().await.remove(&self.guild);

        cache.guilds.write().await.insert(self.guild);
    }
}

/// Sent when a [Guild](Guild) is updated.
#[derive(Deserialize, Debug, Clone)]
pub struct GuildUpdate {
    id: GuildId,
    name: Option<String>,
    icon: Option<String>,
    splash: Option<String>,
    discovery_splash: Option<String>,
    owner: Option<bool>,
    owner_id: UserId,
    // todo deserialize into u64
    permissions: Option<String>,
    region: String,
    afk_channel_id: Option<ChannelId>,
    afk_timeout: u32,
    widget_enabled: Option<bool>,
    widget_channel_id: Option<ChannelId>,
    verification_level: VerificationLevel,
    default_message_notifications: NotificationLevel,
    explicit_content_filter: ExplicitFilterLevel,
    roles: IdMap<Role>,
    emojis: IdMap<CustomEmoji>,
    features: HashSet<GuildFeature>,
    mfa_level: MfaLevel,
    application_id: Option<ApplicationId>,
    system_channel_id: Option<ChannelId>,
    system_channel_flags: SystemChannelFlags,
    rules_channel_id: Option<ChannelId>,
    max_presences: Option<u32>,
    max_members: Option<u32>,
    vanity_url_code: Option<String>,
    description: Option<String>,
    banner: Option<String>,
    premium_tier: PremiumTier,
    premium_subscription_count: Option<u32>,
    preferred_locale: Option<String>,
    public_updates_id_channel: Option<ChannelId>,
    max_video_channel_users: Option<u32>,
    approximate_member_count: Option<u32>,
    approximate_presence_count: Option<u32>,
}

#[async_trait]
impl Update for GuildUpdate {
    async fn update(self, cache: &Cache) {
        cache.guilds.write().await.entry(self.id).and_modify(|guild| {
            guild.id = self.id;
            guild.name = self.name;
            guild.icon = self.icon;
            guild.splash = self.splash;
            guild.discovery_splash = self.discovery_splash;
            guild.owner = self.owner.unwrap_or(guild.owner);
            guild.owner_id = self.owner_id;
            guild.permissions = self.permissions;
            guild.region = self.region;
            guild.afk_channel_id = self.afk_channel_id;
            guild.afk_timeout = self.afk_timeout;
            guild.widget_enabled = self.widget_enabled;
            guild.widget_channel_id = self.widget_channel_id;
            guild.verification_level = self.verification_level;
            guild.default_message_notifications = self.default_message_notifications;
            guild.explicit_content_filter = self.explicit_content_filter;
            guild.roles = self.roles;
            guild.emojis = self.emojis;
            guild.features = self.features;
            guild.mfa_level = self.mfa_level;
            guild.application_id = self.application_id;
            guild.system_channel_id = self.system_channel_id;
            guild.system_channel_flags = self.system_channel_flags;
            guild.rules_channel_id = self.rules_channel_id;
            guild.max_presences = self.max_presences;
            guild.max_members = self.max_members;
            guild.vanity_url_code = self.vanity_url_code;
            guild.description = self.description;
            guild.banner = self.banner;
            guild.premium_tier = self.premium_tier;
            guild.premium_subscription_count = self.premium_subscription_count;
            guild.preferred_locale = self.preferred_locale;
            guild.public_updates_id_channel = self.public_updates_id_channel;
            guild.max_video_channel_users = self.max_video_channel_users;
            guild.approximate_member_count = self.approximate_member_count;
            guild.approximate_presence_count = self.approximate_presence_count;
        });
    }
}

/// Sent when a guild becomes or was already unavailable due to an outage, or when the user leaves
/// or is removed from a guild. If the `unavailable` field is not set, the user was removed from the
/// guild.
#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct GuildDelete {
    guild: UnavailableGuild,
}

#[async_trait]
impl Update for GuildDelete {
    async fn update(self, cache: &Cache) {
        if let Some(guild) = cache.guilds.read().await.get(&self.guild) {
            {
                let mut guard = cache.channel_types.write().await;
                for channel in &guild.channels {
                    guard.remove(&channel.id());
                }
            }
            let mut guard = cache.members.write().await;
            guild.members.iter()
                .for_each(|m| {
                    guard.entry(m.user.id).and_modify(|map| {
                        map.remove(&guild.id);
                    });
                });
        }
        cache.guilds.write().await.remove(&self.guild);
        if self.guild.unavailable {
            cache.unavailable_guilds.write().await.insert(self.guild);
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct BanAdd {
    /// id of the guild
    pub guild_id: GuildId,
    /// the banned user
    pub user: User,
}

#[async_trait]
impl Update for BanAdd {
    async fn update(self, _cache: &Cache) {
        // todo: cache bans?
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct BanRemove {
    /// id of the guild
    pub guild_id: GuildId,
    /// the unbanned user
    pub user: User,
}

#[async_trait]
impl Update for BanRemove {
    async fn update(self, _cache: &Cache) {
        // todo: cache bans?
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct EmojiUpdate {
    /// id of the guild
    pub guild_id: GuildId,
    /// array of emojis
    pub emojis: IdMap<CustomEmoji>,
}

#[async_trait]
impl Update for EmojiUpdate {
    async fn update(self, cache: &Cache) {
        println!("self = {:#?}", self);
        cache.guilds.write().await.entry(self.guild_id)
            .and_modify(|guild| self.emojis.into_iter()
                .for_each(|emoji| guild.emojis.insert(emoji))
            );
    }
}

// why does this and GUILD_INTEGRATIONS_UPDATE exist? who knows
#[derive(Deserialize, Debug, Clone)]
pub struct IntegrationUpdate {
    #[serde(rename = "user")]
    pub owner: User,
    /// always "discord"?
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    // todo check
    // i think this is what this is? actually no hmmm...
    pub id: ApplicationId,
    pub enabled: bool,
    pub application: ApplicationInfo,
    pub account: Account,
    pub guild_id: GuildId,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationInfo {
    pub summary: String,
    pub name: String,
    pub id: ApplicationId,
    pub icon: Option<String>,
    pub description: String,
    pub bot: User,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Account {
    pub name: String,
    pub id: ApplicationId,
}

#[async_trait]
impl Update for IntegrationUpdate {
    async fn update(self, _cache: &Cache) {}
}

/// Sent when a guild integration is updated.
#[derive(Deserialize, Debug, Clone)]
pub struct IntegrationsUpdate {
    /// id of the guild whose integrations were updated
    pub guild_id: GuildId,
}

#[async_trait]
impl Update for IntegrationsUpdate {
    async fn update(self, _cache: &Cache) {
        // nothing has to happen here
    }
}

/// Sent when a new user joins a guild.
///
/// [GUILD_MEMBERS](crate::shard::intents::Intents::GUILD_MEMBERS) is required to receive this.
#[derive(Deserialize, Debug, Clone)]
pub struct GuildMemberAdd {
    /// id of the guild
    pub guild_id: GuildId,
    #[serde(flatten)]
    pub member: GuildMember,
}

#[async_trait]
impl Update for GuildMemberAdd {
    async fn update(self, cache: &Cache) {
        cache.members.write().await.entry(self.member.user.id)
            .and_modify(|map| {
                map.insert(self.guild_id, self.member.clone());
            });
        cache.guilds.write().await.entry(self.guild_id)
            .and_modify(|guild| guild.members.insert(self.member.clone()));
    }
}

/// Sent when a user is removed from a guild (leave/kick/ban).
///
/// [GUILD_MEMBERS](crate::shard::intents::Intents::GUILD_MEMBERS) is required to receive this.
#[derive(Deserialize, Debug, Clone)]
pub struct GuildMemberRemove {
    /// the id of the guild
    pub guild_id: GuildId,
    /// the user who was removed
    pub user: User,
}

#[async_trait]
impl Update for GuildMemberRemove {
    async fn update(self, cache: &Cache) {
        cache.members.write().await.entry(self.user.id)
            .and_modify(|map| {
                map.remove(&self.guild_id);
            });
        cache.guilds.write().await.entry(self.guild_id)
            .and_modify(|guild| guild.members.remove(self.user));
    }
}

/// Sent when a guild member is updated. This will also fire when the user of a guild member changes.
///
/// [GUILD_MEMBERS](crate::shard::intents::Intents::GUILD_MEMBERS) is required to receive this.
#[derive(Deserialize, Debug, Clone)]
pub struct GuildMemberUpdate {
    /// the id of the guild
    pub guild_id: GuildId,
    /// user role ids
    pub roles: HashSet<RoleId>,
    /// the user
    pub user: User,
    /// nickname of the user in the guild
    pub nick: Option<String>,
    /// when the user joined the guild
    pub joined_at: DateTime<Utc>,
    /// when the user starting boosting the guild
    pub premium_since: Option<DateTime<Utc>>,
}

#[async_trait]
impl Update for GuildMemberUpdate {
    async fn update(self, cache: &Cache) {
        println!("self = {:?}", self);
        let mut guard = cache.members.write().await;
        let option = guard.get_mut(&self.user.id)
            .and_then(|map| map.get_mut(&self.guild_id));
        if let Some(member) = option {
            let new = self.clone();
            member.user = new.user;
            member.nick = new.nick;
            member.roles = new.roles;
            member.joined_at = new.joined_at;
            member.premium_since = new.premium_since;
        }

        if let Some(guild) = cache.guilds.write().await.get_mut(self.guild_id) {
            if let Some(member) = guild.members.get_mut(&self.user) {
                member.user = self.user;
                member.nick = self.nick;
                member.roles = self.roles;
                member.joined_at = self.joined_at;
                member.premium_since = self.premium_since;
            }
        }
    }
}

/// Sent in response to Guild Request Members. You can use `chunk_index` and `chunk_count` to
/// calculate how many chunks are left for your request.
#[derive(Deserialize, Debug, Clone)]
pub struct GuildMembersChunk {
    /// the id of the guild
    pub guild_id: GuildId,
    /// set of guild members
    pub members: IdMap<GuildMember>,
    /// the chunk index in the expected chunks for this response (0 <= chunk_index < chunk_count)
    pub chunk_index: u32,
    /// the total number of expected chunks for this response
    pub chunk_count: u32,
    // todo I think this is user id? could also be the guild id (or both ig)
    /// if passing an invalid id to REQUEST_GUILD_MEMBERS, it will be returned here
    #[serde(default)]
    pub not_found: Vec<UserId>,
    /// if passing true to REQUEST_GUILD_MEMBERS, presences of the returned members will be here
    #[serde(default)]
    pub presences: IdMap<PresenceUpdate>,
    /// the nonce used in the Guild Members Request
    pub nonce: Option<String>,
}

#[async_trait]
impl Update for GuildMembersChunk {
    async fn update(self, cache: &Cache) {
        let mut guard = cache.members.write().await;
        for member in &self.members {
            let cached = guard.get_mut(&member.user.id)
                .and_then(|map| map.get_mut(&self.guild_id));
            if let Some(cached) = cached {
                *cached = member.clone();
            }
        }
        if let Some(guild) = cache.guilds.write().await.get_mut(self.guild_id) {
            guild.members.extend(self.members);
            guild.presences.extend(self.presences);
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct GuildRoleCreate {
    /// the id of the guild
    pub(crate) guild_id: GuildId,
    /// the role created
    pub(crate) role: Role,
}

#[async_trait]
impl Update for GuildRoleCreate {
    async fn update(self, cache: &Cache) {
        if let Some(guild) = cache.guilds.write().await.get_mut(self.guild_id) {
            guild.roles.insert(self.role);
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct GuildRoleUpdate {
    /// the id of the guild
    pub(crate) guild_id: GuildId,
    /// the role created
    pub(crate) role: Role,
}

#[async_trait]
impl Update for GuildRoleUpdate {
    async fn update(self, cache: &Cache) {
        if let Some(guild) = cache.guilds.write().await.get_mut(self.guild_id) {
            guild.roles.insert(self.role);
        }
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct GuildRoleDelete {
    /// the id of the guild
    pub(crate) guild_id: GuildId,
    /// the role created
    pub(crate) role_id: RoleId,
}

#[async_trait]
impl Update for GuildRoleDelete {
    async fn update(self, cache: &Cache) {
        if let Some(guild) = cache.guilds.write().await.get_mut(self.guild_id) {
            guild.roles.remove(self.role_id);
        }
    }
}

// Invite Events

#[derive(Deserialize, Debug, Clone)]
pub struct InviteCreate {
    /// the channel the invite is for
    pub(crate) channel_id: ChannelId,
    /// the unique invite code
    pub(crate) code: String,
    /// the time at which the invite was created
    pub(crate) created_at: DateTime<Utc>,
    /// the guild of the invite
    pub(crate) guild_id: Option<GuildId>,
    /// the user that created the invite
    pub(crate) inviter: Option<User>,
    /// how long the invite is valid for (in seconds)
    // todo deserialize as Duration
    pub(crate) max_age: i32,
    /// the maximum number of times the invite can be used
    pub(crate) max_uses: u32,
    /// the target user for this invite
    pub(crate) target_user: Option<User>,
    /// the type of user target for this invite
    // todo model Invite: https://discord.com/developers/docs/resources/invite#invite-object-target-user-types
    pub(crate) target_user_type: Option<u8>,
    /// whether or not the invite is temporary (invited users will be kicked on disconnect unless they're assigned a role)
    pub(crate) temporary: bool,
    /// how many times the invite has been used (always will be 0)
    pub(crate) uses: u8,
}

#[async_trait]
impl Update for InviteCreate {
    async fn update(self, _cache: &Cache) {}
}

#[derive(Deserialize, Debug, Clone)]
pub struct InviteDelete {
    /// the channel of the invite
    pub(crate) channel_id: ChannelId,
    /// the guild of the invite
    pub(crate) guild_id: Option<GuildId>,
    /// the unique invite [code](https://discord.com/developers/docs/resources/invite#invite-object)
    pub(crate) code: String,
}

#[async_trait]
impl Update for InviteDelete {
    async fn update(self, _cache: &Cache) {}
}

// Message Events

#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct MessageCreate {
    pub(crate) message: Message,
}

#[async_trait]
impl Update for MessageCreate {
    async fn update(self, cache: &Cache) {
        let channel_type = cache.channel_types.read().await
            .get(&self.message.channel_id)
            .copied();
        match channel_type {
            Some(ChannelType::GuildText) => {
                // todo should this just unwrap?
                if let Some(channel) = cache.channels.write().await.get_mut(&self.message.channel_id) {
                    channel.last_message_id = Some(self.message.id);
                }
            }
            Some(ChannelType::Dm) => {
                if let Some(dm) = cache.dms.write().await.1.get_mut(&self.message.channel_id) {
                    dm.last_message_id = Some(self.message.id);
                }
            }
            Some(ChannelType::GuildNews) => {
                if let Some(news) = cache.news.write().await.get_mut(&self.message.channel_id) {
                    news.last_message_id = Some(self.message.id);
                }
            }
            Some(ChannelType::GuildStore)
            | Some(ChannelType::GuildVoice)
            | Some(ChannelType::GroupDm)
            | Some(ChannelType::GuildCategory)
            | None => {}
        }
        cache.messages.write().await.insert(self.message);
    }
}

/// like `Message` but everything (except for `id`, `channel_id`) is optional
#[derive(Deserialize, Debug, Clone)]
pub struct MessageUpdate {
    pub(crate) id: MessageId,
    pub(crate) channel_id: ChannelId,
    pub(crate) guild_id: Option<Option<GuildId>>,
    pub(crate) author: Option<User>,
    pub(crate) member: Option<Option<GuildMemberUserless>>,
    pub(crate) content: Option<String>,
    pub(crate) timestamp: Option<DateTime<Utc>>,
    pub(crate) edited_timestamp: Option<Option<DateTime<Utc>>>,
    pub(crate) tts: Option<bool>,
    pub(crate) mention_everyone: Option<bool>,
    pub(crate) mentions: Option<Vec<User>>,
    pub(crate) mention_roles: Option<Vec<RoleId>>,
    pub(crate) mention_channels: Option<Vec<ChannelMention>>,
    pub(crate) attachments: Option<Vec<Attachment>>,
    pub(crate) embeds: Option<Vec<Embed>>,
    pub(crate) reactions: Option<Vec<Reaction>>,
    pub(crate) nonce: Option<Option<String>>,
    pub(crate) pinned: Option<bool>,
    pub(crate) webhook_id: Option<Option<WebhookId>>,
    #[serde(rename = "type")]
    pub(crate) message_type: Option<MessageType>,
    pub(crate) activity: Option<Option<MessageActivity>>,
    pub(crate) application: Option<Option<MessageApplication>>,
    pub(crate) message_reference: Option<Option<MessageReference>>,
    pub(crate) flags: Option<Option<MessageFlags>>,
    pub(crate) stickers: Option<Vec<Sticker>>,
    pub(crate) referenced_message: Option<Option<Message>>,
}

impl TryFrom<MessageUpdate> for Message {
    type Error = ();

    fn try_from(update: MessageUpdate) -> Result<Self, Self::Error> {
        fn option(update: MessageUpdate) -> Option<Message> {
            Some(Message {
                id: update.id,
                channel_id: update.channel_id,
                guild_id: update.guild_id.unwrap_or_default(),
                author: update.author?,
                member: update.member.unwrap_or_default(),
                content: update.content?,
                timestamp: update.timestamp?,
                edited_timestamp: update.edited_timestamp.unwrap_or_default(),
                tts: update.tts.unwrap_or_default(),
                mention_everyone: update.mention_everyone.unwrap_or_default(),
                mentions: update.mentions.unwrap_or_default(),
                mention_roles: update.mention_roles.unwrap_or_default(),
                mention_channels: update.mention_channels.unwrap_or_default(),
                attachments: update.attachments.unwrap_or_default(),
                embeds: update.embeds.unwrap_or_default(),
                reactions: update.reactions.unwrap_or_default(),
                nonce: update.nonce.unwrap_or_default(),
                pinned: update.pinned.unwrap_or_default(),
                webhook_id: update.webhook_id.unwrap_or_default(),
                message_type: update.message_type?,
                activity: update.activity.unwrap_or_default(),
                application: update.application.unwrap_or_default(),
                message_reference: update.message_reference.unwrap_or_default(),
                flags: update.flags.unwrap_or_default(),
                stickers: update.stickers.unwrap_or_default(),
                referenced_message: update.referenced_message.unwrap_or_default().map(Box::new),
            })
        }
        option(update).ok_or(())
    }
}

#[async_trait]
impl Update for MessageUpdate {
    async fn update(self, cache: &Cache) {
        let mut guard = cache.messages.write().await;
        match guard.entry(self.id) {
            Entry::Occupied(mut e) => {
                let message = e.get_mut();
                fn update<T>(original: &mut T, new: Option<T>) {
                    if let Some(new) = new { *original = new; }
                }
                update(&mut message.guild_id, self.guild_id);
                update(&mut message.author, self.author);
                update(&mut message.member, self.member);
                update(&mut message.content, self.content);
                update(&mut message.edited_timestamp, self.edited_timestamp);
                update(&mut message.tts, self.tts);
                update(&mut message.mention_everyone, self.mention_everyone);
                update(&mut message.mentions, self.mentions);
                update(&mut message.mention_roles, self.mention_roles);
                update(&mut message.mention_channels, self.mention_channels);
                update(&mut message.attachments, self.attachments);
                update(&mut message.embeds, self.embeds);
                update(&mut message.reactions, self.reactions);
                update(&mut message.nonce, self.nonce);
                update(&mut message.pinned, self.pinned);
                update(&mut message.webhook_id, self.webhook_id);
                update(&mut message.message_type, self.message_type);
                update(&mut message.activity, self.activity);
                update(&mut message.application, self.application);
                update(&mut message.message_reference, self.message_reference);
                update(&mut message.flags, self.flags);
                update(&mut message.stickers, self.stickers);
                if let Some(referenced) = self.referenced_message {
                    message.referenced_message = referenced.map(Box::new);
                }
            }
            Entry::Vacant(e) => {
                if let Ok(message) = self.try_into() {
                    e.insert(message);
                }
            }
        };
    }
}

#[derive(Deserialize, Debug, Copy, Clone)]
pub struct MessageDelete {
    /// the id of the message
    pub id: MessageId,
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
}

#[async_trait]
impl Update for MessageDelete {
    async fn update(self, cache: &Cache) {
        use ChannelType::*;
        cache.messages.write().await.remove(self.id);
        match cache.channel_types.read().await.get(&self.channel_id) {
            Some(GuildText) => {
                if let Some(channel) = cache.channels.write().await.get_mut(self.channel_id) {
                    channel.last_message_id = channel.last_message_id.filter(|&id| id != self.id);
                }
            }
            Some(Dm) => {
                if let Some(channel) = cache.dms.write().await.1.get_mut(self.channel_id) {
                    channel.last_message_id = channel.last_message_id.filter(|&id| id != self.id);
                }
            }
            Some(GuildNews) => {
                if let Some(channel) = cache.news.write().await.get_mut(self.channel_id) {
                    channel.last_message_id = channel.last_message_id.filter(|&id| id != self.id);
                }
            }
            Some(_) | None => {}
        }
        if let Some(id) = self.guild_id {
            if let Some(guild) = cache.guilds.write().await.get_mut(id) {
                match guild.channels.get_mut(self.channel_id) {
                    Some(Channel::Text(text)) => {
                        text.last_message_id = text.last_message_id.filter(|&id| id != self.id);
                    }
                    Some(Channel::Dm(dm)) => {
                        dm.last_message_id = dm.last_message_id.filter(|&id| id != self.id);
                    }
                    Some(Channel::News(news)) => {
                        news.last_message_id = news.last_message_id.filter(|&id| id != self.id);
                    }
                    Some(_) | None => {}
                }
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct MessageDeleteBulk {
    /// the id of the message
    pub ids: Vec<MessageId>,
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
}

#[async_trait]
impl Update for MessageDeleteBulk {
    async fn update(self, cache: &Cache) {
        let Self { ids, channel_id, guild_id } = self;
        let update = |delete: MessageDelete| delete.update(cache);
        tokio::stream::iter(ids)
            .map(|id| MessageDelete { id, channel_id, guild_id })
            .for_each(update)
            .await;
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ReactionAdd {
    /// the id of the user
    pub user_id: UserId,
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the id of the message
    pub message_id: MessageId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// the member who reacted if this happened in a guild
    pub member: Option<GuildMember>,
    /// the emoji used to react
    pub emoji: Emoji,
}


#[async_trait]
impl Update for ReactionAdd {
    async fn update(self, cache: &Cache) {
        if let Some(message) = cache.messages.write().await.get_mut(self.message_id) {
            let idx = message.reactions.iter()
                .position(|reaction| reaction.emoji == self.emoji);
            let me = cache.user.read().await.as_ref()
                .map(|me| me.id == self.user_id)
                .unwrap_or(false);
            if let Some(idx) = idx {
                let reaction = &mut message.reactions[idx];
                reaction.count += 1;
                reaction.me |= me;
            } else {
                message.reactions.push(Reaction {
                    count: 1,
                    me,
                    emoji: self.emoji.clone(),
                });
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ReactionRemove {
    /// the id of the user
    pub user_id: UserId,
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the id of the message
    pub message_id: MessageId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// the emoji used to react
    pub emoji: Emoji,
}

#[async_trait]
impl Update for ReactionRemove {
    async fn update(self, cache: &Cache) {
        if let Some(message) = cache.messages.write().await.get_mut(self.message_id) {
            let idx = message.reactions.iter()
                .position(|reaction| reaction.emoji == self.emoji);
            let me = cache.user.read().await.as_ref()
                .map(|me| me.id == self.user_id)
                .unwrap_or(false);
            if let Some(idx) = idx {
                let reaction = &mut message.reactions[idx];
                reaction.count -= 1;
                reaction.me &= !me;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ReactionType { Add, Remove }

#[derive(Debug, Clone)]
pub struct ReactionUpdate {
    /// whether this reaction was added or removed
    pub kind: ReactionType,
    /// the id of the user
    pub user_id: UserId,
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the id of the message
    pub message_id: MessageId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// the emoji used to react
    pub emoji: Emoji,
}

impl From<ReactionAdd> for ReactionUpdate {
    fn from(add: ReactionAdd) -> Self {
        Self {
            kind: ReactionType::Add,
            user_id: add.user_id,
            channel_id: add.channel_id,
            message_id: add.message_id,
            guild_id: add.guild_id,
            emoji: add.emoji,
        }
    }
}

impl From<ReactionRemove> for ReactionUpdate {
    fn from(remove: ReactionRemove) -> Self {
        Self {
            kind: ReactionType::Remove,
            user_id: remove.user_id,
            channel_id: remove.channel_id,
            message_id: remove.message_id,
            guild_id: remove.guild_id,
            emoji: remove.emoji,
        }
    }
}

/// Sent when a user explicitly removes all reactions from a message.
#[derive(Deserialize, Debug, Copy, Clone)]
pub struct ReactionRemoveAll {
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the id of the message
    pub message_id: MessageId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
}

#[async_trait]
impl Update for ReactionRemoveAll {
    async fn update(self, cache: &Cache) {
        if let Some(message) = cache.messages.write().await.get_mut(self.message_id) {
            message.reactions.clear();
        }
    }
}

/// Sent when a bot removes all instances of a given emoji from the reactions of a message.
#[derive(Deserialize, Debug, Clone)]
pub struct ReactionRemoveEmoji {
    /// the id of the channel
    pub channel_id: ChannelId,
    /// the id of the message
    pub message_id: MessageId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// the emoji that was removed
    pub emoji: Emoji,
}

#[async_trait]
impl Update for ReactionRemoveEmoji {
    async fn update(self, cache: &Cache) {
        if let Some(messsage) = cache.messages.write().await.get_mut(self.message_id) {
            messsage.reactions.retain(|f| f.emoji != self.emoji);
        }
    }
}

// Presence Updates

/// A user's presence is their current state on a guild. This event is sent when a user's presence
/// or info, such as name or avatar, is updated.
///
/// [GUILD_PRESENCES](crate::shard::intents::Intents::GUILD_PRESENCES) is required to receive this.
///
/// The user object within this event can be partial, the only field which must be sent is the `id`
/// field, everything else is optional. Along with this limitation, no fields are required, and the
/// types of the fields are **not** validated. Your client should expect any combination of fields
/// and types within this event.
// todo ^ that's a bit scary
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PresenceUpdate {
    /// the user presence is being updated for
    pub user: User,
    /// id of the guild
    pub guild_id: GuildId,
    /// either "idle", "dnd", "online", or "offline"
    pub status: StatusType,
    /// user's current activities
    pub activities: Vec<Activity>,
    /// user's platform-dependent status
    pub client_status: ClientStatus,
}

impl PartialEq for PresenceUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.user.id == other.user.id &&
            self.guild_id == other.guild_id
    }
}

impl Id for PresenceUpdate {
    type Id = UserId;

    fn id(&self) -> Self::Id {
        self.user.id
    }
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone)]
pub struct ClientStatus {
    /// the user's status set for an active desktop (Windows, Linux, Mac) application session
    pub desktop: Option<StatusType>,
    /// the user's status set for an active mobile (iOS, Android) application session
    pub mobile: Option<StatusType>,
    /// the user's status set for an active web (browser, bot account) application session
    pub web: Option<StatusType>,
}

#[async_trait]
impl Update for PresenceUpdate {
    async fn update(self, cache: &Cache) {
        if let Some(guild) = cache.guilds.write().await.get_mut(self.guild_id) {
            guild.presences.insert(self);
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TypingStart {
    /// id of the channel
    pub channel_id: ChannelId,
    /// id of the guild
    pub guild_id: Option<GuildId>,
    /// id of the user
    pub user_id: UserId,
    // todo Deserialize as DateTime<Utc>
    /// unix time (in seconds) of when the user started typing
    pub timestamp: u64,
    /// the member who started typing if this happened in a guild
    pub member: Option<GuildMember>,
}

#[async_trait]
impl Update for TypingStart {
    async fn update(self, _cache: &Cache) {
        // don't think we need to update anything here?
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct UserUpdate {
    user: User,
}

#[async_trait]
impl Update for UserUpdate {
    async fn update(self, cache: &Cache) {
        // todo make sure this does mean current user
        log::warn!("{:?}", &self);
        *cache.user.write().await = Some(self.user);
    }
}

// Voice Updates

#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct VoiceStateUpdate {
    state: VoiceState,
}

#[async_trait]
impl Update for VoiceStateUpdate {
    async fn update(self, cache: &Cache) {
        if let Some(guild_id) = self.state.guild_id {
            if let Some(map) = cache.members.write().await.get_mut(&self.state.user_id) {
                if let Some(member) = map.get_mut(&guild_id) {
                    member.deaf = self.state.self_deaf;
                    member.mute = self.state.self_mute;
                }
            }
            if let Some(guild) = cache.guilds.write().await.get_mut(guild_id) {
                guild.voice_states.insert(self.state);
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct VoiceServerUpdate {
    /// voice connection token
    pub token: String,
    /// the guild this voice server update is for
    pub guild_id: GuildId,
    /// the voice server host
    pub endpoint: String,
}

#[async_trait]
impl Update for VoiceServerUpdate {
    async fn update(self, _cache: &Cache) {}
}

// Webhook Updates

#[derive(Deserialize, Debug, Clone)]
pub struct WebhookUpdate {
    /// id of the guild
    pub guild_id: GuildId,
    /// id of the channel
    pub channel_id: ChannelId,
}

#[async_trait]
impl Update for WebhookUpdate {
    async fn update(self, _cache: &Cache) {}
}

// Slash Command Updates

#[derive(Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct InteractionCreate {
    pub(crate) interaction: Interaction,
}

#[async_trait]
impl Update for InteractionCreate {
    async fn update(self, _cache: &Cache) {}
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationCommandCreate {
    guild_id: GuildId,
    #[serde(flatten)]
    command: ApplicationCommand,
}

#[async_trait]
impl Update for ApplicationCommandCreate {
    async fn update(self, _cache: &Cache) {}
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationCommandUpdate {
    guild_id: GuildId,
    #[serde(flatten)]
    command: ApplicationCommand,
}

#[async_trait]
impl Update for ApplicationCommandUpdate {
    async fn update(self, _cache: &Cache) {}
}


#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationCommandDelete {
    guild_id: GuildId,
    #[serde(flatten)]
    command: ApplicationCommand,
}

#[async_trait]
impl Update for ApplicationCommandDelete {
    async fn update(self, _cache: &Cache) {}
}
