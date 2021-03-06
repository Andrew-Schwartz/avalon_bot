use std::collections::HashMap;

use itertools::{Either, Itertools};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error;

use crate::cache::Cache;
use crate::model::channel::{Channel, OverwriteType};
use crate::model::guild::GuildMember;
use crate::model::ids::*;
pub use crate::model::ids::RoleId;
use crate::model::message::Color;

bitflags! {
    pub struct Permissions: u64 {
		/// Allows creation of instant invites
		///
		/// T, V, S
		const CREATE_INSTANT_INVITE = 0x0000_0000_0001;
		/// Allows kicking members
		const KICK_MEMBERS = 0x0000_0000_0002;
		/// Allows banning members
		const BAN_MEMBERS = 0x0000_0000_0004;
		/// Allows all permissions and bypasses channel permission overwrites
		const ADMINISTRATOR = 0x0000_0000_0008;
		/// Allows management and editing of channels
		///
		/// T, V, S
		const MANAGE_CHANNELS = 0x0000_0000_0010;
		/// Allows management and editing of the guild
		const MANAGE_GUILD = 0x0000_0000_0020;
		/// Allows for the addition of reactions to messages
		///
		/// T
		const ADD_REACTIONS = 0x0000_0000_0040;
		/// Allows for viewing of audit logs
		const VIEW_AUDIT_LOG = 0x0000_0000_0080;
		/// Allows for using priority speaker in a voice channel
		///
		/// V
		const PRIORITY_SPEAKER = 0x0000_0000_0100;
		/// Allows the user to go live
		///
		/// V
		const STREAM = 0x0000_0000_0200;
		/// Allows guild members to view a channel, which includes reading messages in text channels
		///
		/// T, V, S
		const VIEW_CHANNEL = 0x0000_0000_0400;
		/// Allows for sending messages in a channel
		///
		/// T
		const SEND_MESSAGES = 0x0000_0000_0800;
		/// Allows for sending of /tts messages
		///
		/// T
		const SEND_TTS_MESSAGES = 0x0000_0000_1000;
		/// Allows for deletion of other users messages
		///
		/// T
		const MANAGE_MESSAGES = 0x0000_0000_2000;
		/// Links sent by users with this permission will be auto-embedded
		///
		/// T
		const EMBED_LINKS = 0x0000_0000_4000;
		/// Allows for uploading images and files
		///
		/// T
		const ATTACH_FILES = 0x0000_0000_8000;
		/// Allows for reading of message history
		///
		/// T
		const READ_MESSAGE_HISTORY = 0x0000_0001_0000;
		/// Allows for using the @everyone tag to notify all users in a channel, and the `@here` tag
		/// to notify all online users in a channel
		///
		/// T
		const MENTION_EVERYONE = 0x0000_0002_0000;
		/// Allows the usage of custom emojis from other servers
		///
		/// T
		const USE_EXTERNAL_EMOJIS = 0x0000_0004_0000;
		/// Allows for viewing guild insights
		const VIEW_GUILD_INSIGHTS = 0x0000_0008_0000;
		/// Allows for joining of a voice channel
		///
		/// V, S
		const CONNECT = 0x0000_0010_0000;
		/// Allows for speaking in a voice channel
		///
		/// V
		const SPEAK = 0x0000_0020_0000;
		/// Allows for muting members in a voice channel
		///
		/// V, S
		const MUTE_MEMBERS = 0x0000_0040_0000;
		/// Allows for deafening of members in a voice channel
		///
		/// V, S
		const DEAFEN_MEMBERS = 0x0000_0080_0000;
		/// Allows for moving of members between voice channels
		///
		/// V, S
		const MOVE_MEMBERS = 0x0000_0100_0000;
		/// Allows for using voice-activity-detection in a voice channel
		///
		/// V, S
		const USE_VAD = 0x0000_0200_0000;
		/// Allows for modification of own nickname
		const CHANGE_NICKNAME = 0x0000_0400_0000;
		/// Allows for modification of other users nicknames
		const MANAGE_NICKNAMES = 0x0000_0800_0000;
		/// Allows management and editing of roles
		///
		/// T, V, S
		const MANAGE_ROLES = 0x0000_1000_0000;
		/// Allows management and editing of webhooks
		///
		/// T
		const MANAGE_WEBHOOKS = 0x0000_2000_0000;
		/// Allows management and editing of emojis
		const MANAGE_EMOJIS = 0x0000_4000_0000;
		/// Allows members to use slash commands in text channels
		///
		/// T
		const USE_SLASH_COMMANDS = 0x0000_8000_0000;
		/// Allows for requesting to speak in stage channels. (This permission is under active
		/// development and may be changed or removed.)
		///
		/// S
		const REQUEST_TO_SPEAK = 0x0001_0000_0000;
    }
}

impl Permissions {
    pub async fn get(cache: &Cache, member: &GuildMember, channel: &Channel) -> Self {
        let guild = channel.guild_id().unwrap();
        let everyone = cache.everyone_role(&guild).await;

        let base_permissions = Self::base_permissions(
            cache, member, channel.guild_id().unwrap(), &everyone,
        ).await;
        base_permissions.overwrites(member, channel, &everyone).await
    }

    pub async fn get_own(cache: &Cache, channel: &Channel) -> Self {
        let guild = channel.guild_id().unwrap();
        let member = cache.member(guild, cache.own_user().await).await.unwrap();
        Self::get(cache, &member, channel).await
    }

    async fn base_permissions(cache: &Cache, member: &GuildMember, guild: GuildId, everyone: &Role) -> Self {
        let guild = cache.guild(guild).await.unwrap();
        if guild.owner { return Self::all(); }

        let permissions = member.roles.iter()
            .flat_map(|role| guild.roles.get(role))
            .fold(everyone.permissions, |perms, role_perms| perms | role_perms.permissions);
        if permissions.contains(Self::ADMINISTRATOR) {
            Self::all()
        } else {
            permissions
        }
    }

    async fn overwrites(self, member: &GuildMember, channel: &Channel, everyone: &Role) -> Self {
        // ADMINISTRATOR overrides any potential permission overwrites, so there is nothing to do here.
        if self.contains(Self::ADMINISTRATOR) { return Self::all(); }

        let mut perms = self;

        if let Some(overwrites) = channel.overwrites() {
            let (role_overwrites, member_overwrites): (HashMap<_, _>, HashMap<_, _>) = overwrites.iter()
                .partition_map(|overwrite| match overwrite.id {
                    OverwriteType::Role(role) => Either::Left((role, (overwrite.allow, overwrite.deny))),
                    OverwriteType::Member(user) => Either::Right((user, (overwrite.allow, overwrite.deny))),
                });

            // Find `@everyone` role overwrite and apply it.
            if let Some(&(allow, deny)) = role_overwrites.get(&everyone.id) {
                perms &= !deny;
                perms |= allow;
            }

            // Apply role specific overwrites.
            let (allow, deny) = member.roles.iter()
                .flat_map(|id| role_overwrites.get(id))
                .fold(
                    (Self::empty(), Self::empty()),
                    |(allow, deny), &(overwrite_allow, overwrite_deny)| (allow | overwrite_allow, deny | overwrite_deny),
                );
            perms &= !deny;
            perms |= allow;

            // Apply member specific overwrite.
            if let Some(&(allow, deny)) = member_overwrites.get(&member.id()) {
                perms &= !deny;
                perms |= allow;
            }
        }

        perms
    }
}

// can't just use `serde_bitflag!` because the bitflags are received as strings
impl<'de> Deserialize<'de> for Permissions {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bits = <&'de str>::deserialize(d)?
            .parse()
            .map_err(|e| D::Error::custom(format!("Unable to parse bits as u64: {}", e)))?;
        Self::from_bits(bits)
            .ok_or_else(|| D::Error::custom(format!("Unexpected `Permissions` bitflag value {}", bits)))
    }
}

impl Serialize for Permissions {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.bits.serialize(s)
    }
}

/// Roles represent a set of permissions attached to a group of users. Roles have unique names,
/// colors, and can be "pinned" to the side bar, causing their members to be listed separately.
/// Roles are unique per guild, and can have separate permission profiles for the global context
/// (guild) and channel context. The `@everyone` role has the same ID as the guild it belongs to.
/// Roles without colors (`color == 0`) do not count towards the final computed color in the user
/// list.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Role {
    /// role id
    pub id: RoleId,
    /// role name
    pub name: String,
    /// integer representation of hexadecimal color code
    pub color: Color,
    /// if this role is pinned in the user listing
    pub hoist: bool,
    /// position of this role
    pub position: u32,
    /// permission bit set
    pub permissions: Permissions,
    /// whether this role is managed by an integration
    pub managed: bool,
    /// whether this role is mentionable
    pub mentionable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<RoleTags>,
}
id_impl!(Role => RoleId);

pub trait RoleMarkupExt: Id<Id=RoleId> {
    fn mention(&self) -> String {
        format!("<@&{}>", self.id())
    }
}

impl<I: Id<Id=RoleId>> RoleMarkupExt for I {}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RoleTags {
    /// the id of the bot this role belongs to
    bot_id: Option<ApplicationId>,
    /// the id of the integration this role belongs to
    integration_id: Option<InteractionId>,
    // todo docs say the type of this is `null`... idk how to handle that lol. probably have to make
    //  custom visitor based deserializer
    /// whether this is the guild's premium subscriber role
    premium_subscriber: Option<()>,
}
