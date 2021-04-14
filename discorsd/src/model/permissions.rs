use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error;

use crate::model::ids::*;
pub use crate::model::ids::RoleId;
use crate::model::message::Color;

bitflags! {
    pub struct Permissions: u64 {
		/// Allows creation of instant invites
		///
		/// T, V, S
		const CREATE_INSTANT_INVITE = 0x000000001;
		/// Allows kicking members
		const KICK_MEMBERS = 0x000000002;
		/// Allows banning members
		const BAN_MEMBERS = 0x000000004;
		/// Allows all permissions and bypasses channel permission overwrites
		const ADMINISTRATOR = 0x000000008;
		/// Allows management and editing of channels
		///
		/// T, V, S
		const MANAGE_CHANNELS = 0x000000010;
		/// Allows management and editing of the guild
		const MANAGE_GUILD = 0x000000020;
		/// Allows for the addition of reactions to messages
		///
		/// T
		const ADD_REACTIONS = 0x000000040;
		/// Allows for viewing of audit logs
		const VIEW_AUDIT_LOG = 0x000000080;
		/// Allows for using priority speaker in a voice channel
		///
		/// V
		const PRIORITY_SPEAKER = 0x000000100;
		/// Allows the user to go live
		///
		/// V
		const STREAM = 0x000000200;
		/// Allows guild members to view a channel, which includes reading messages in text channels
		///
		/// T, V, S
		const VIEW_CHANNEL = 0x000000400;
		/// Allows for sending messages in a channel
		///
		/// T
		const SEND_MESSAGES = 0x000000800;
		/// Allows for sending of /tts messages
		///
		/// T
		const SEND_TTS_MESSAGES = 0x000001000;
		/// Allows for deletion of other users messages
		///
		/// T
		const MANAGE_MESSAGES = 0x000002000;
		/// Links sent by users with this permission will be auto-embedded
		///
		/// T
		const EMBED_LINKS = 0x000004000;
		/// Allows for uploading images and files
		///
		/// T
		const ATTACH_FILES = 0x000008000;
		/// Allows for reading of message history
		///
		/// T
		const READ_MESSAGE_HISTORY = 0x000010000;
		/// Allows for using the @everyone tag to notify all users in a channel, and the `@here` tag
		/// to notify all online users in a channel
		///
		/// T
		const MENTION_EVERYONE = 0x000020000;
		/// Allows the usage of custom emojis from other servers
		///
		/// T
		const USE_EXTERNAL_EMOJIS = 0x000040000;
		/// Allows for viewing guild insights
		const VIEW_GUILD_INSIGHTS = 0x000080000;
		/// Allows for joining of a voice channel
		///
		/// V, S
		const CONNECT = 0x000100000;
		/// Allows for speaking in a voice channel
		///
		/// V
		const SPEAK = 0x000200000;
		/// Allows for muting members in a voice channel
		///
		/// V, S
		const MUTE_MEMBERS = 0x000400000;
		/// Allows for deafening of members in a voice channel
		///
		/// V, S
		const DEAFEN_MEMBERS = 0x000800000;
		/// Allows for moving of members between voice channels
		///
		/// V, S
		const MOVE_MEMBERS = 0x001000000;
		/// Allows for using voice-activity-detection in a voice channel
		///
		/// V, S
		const USE_VAD = 0x002000000;
		/// Allows for modification of own nickname
		const CHANGE_NICKNAME = 0x004000000;
		/// Allows for modification of other users nicknames
		const MANAGE_NICKNAMES = 0x008000000;
		/// Allows management and editing of roles
		///
		/// T, V, S
		const MANAGE_ROLES = 0x010000000;
		/// Allows management and editing of webhooks
		///
		/// T
		const MANAGE_WEBHOOKS = 0x020000000;
		/// Allows management and editing of emojis
		const MANAGE_EMOJIS = 0x040000000;
		/// Allows members to use slash commands in text channels
		///
		/// T
		const USE_SLASH_COMMANDS = 0x080000000;
		/// Allows for requesting to speak in stage channels. (This permission is under active
		/// development and may be changed or removed.)
		///
		/// S
		const REQUEST_TO_SPEAK = 0x100000000;
    }
}

// can't just use `serde_bitflag` because the bitflags are received as strings
impl<'de> Deserialize<'de> for Permissions {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		let bits = String::deserialize(d)?
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

id_eq!(Role);
impl Id for Role {
	type Id = RoleId;

	fn id(&self) -> Self::Id {
		self.id
	}
}