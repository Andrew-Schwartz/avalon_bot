use std::convert::TryFrom;
use std::fmt::{self, Display};

use serde::{de, Deserialize, Serialize, Serializer};
use serde::ser::{Error, SerializeMap};
use serde_json::value::RawValue;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::model::emoji::Emoji;
use crate::model::ids::{ApplicationId, ChannelId, GuildId, UserId};
use crate::serde_utils::BoolExt;
use crate::serde_utils::nice_from_str;
use crate::shard::dispatch::DispatchPayload;
use crate::shard::intents::Intents;

#[derive(Deserialize, Debug)]
#[serde(try_from = "RawPayload")]
pub(crate) enum Payload {
    /// Receive: An event was dispatched.
    Dispatch {
        event: DispatchPayload,
        seq_num: u64,
    },
    /// Send/Receive: Fired periodically by the client to keep the connection alive.
    Heartbeat(Heartbeat),
    /// Send: Starts a new session during the initial handshake.
    Identify(Identify),
    /// Send: Update the client's presence.
    UpdateStatus(UpdateStatus),
    /// Send: Used to join/leave or move between voice channels.
    #[allow(dead_code)]
    UpdateVoiceStatus(UpdateVoiceStatus),
    /// Send: Resume a previous session that was disconnected.
    Resume(Resume),
    /// Receive: You should attempt to reconnect and resume immediately.
    Reconnect,
    /// Send: Request information about offline guild members in a large guild.
    RequestGuildMembers(RequestGuildMembers),
    /// Receive: The session has been invalidated. You should reconnect and identify/resume accordingly.
    ///
    /// The `bool` indicates whether the session may be resumable
    InvalidSession(bool),
    /// Receive: Sent immediately after connecting, contains the heartbeat_interval to use.
    Hello(HelloPayload),
    /// Receive: Sent in response to receiving a heartbeat to acknowledge that it has been received.
    HeartbeatAck,
}

impl Payload {
    const fn opcode(&self) -> u8 {
        match self {
            Self::Dispatch { .. } => 0,
            Self::Heartbeat(_) => 1,
            Self::Identify(_) => 2,
            Self::UpdateStatus(_) => 3,
            Self::UpdateVoiceStatus(_) => 4,
            Self::Resume(_) => 6,
            Self::Reconnect => 7,
            Self::RequestGuildMembers(_) => 8,
            Self::InvalidSession(_) => 9,
            Self::Hello(_) => 10,
            Self::HeartbeatAck => 11,
        }
    }
}

impl Serialize for Payload {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("op", &self.opcode())?;
        match &self {
            Self::Identify(identify) => map.serialize_entry("d", identify)?,
            Self::Heartbeat(Heartbeat { seq_num }) => map.serialize_entry("d", seq_num)?,
            Self::UpdateStatus(status) => map.serialize_entry("d", status)?,
            Self::UpdateVoiceStatus(status) => map.serialize_entry("d", status)?,
            Self::Resume(resume) => map.serialize_entry("d", resume)?,
            Self::RequestGuildMembers(rgm) => map.serialize_entry("d", rgm)?,
            Self::Dispatch { .. }
            | Self::Reconnect
            | Self::InvalidSession(_)
            | Self::Hello(_)
            | Self::HeartbeatAck => return Err(S::Error::custom("should not be serialized")),
        };
        map.end()
    }
}

// Exists to mediate deserialization to Payload
#[derive(Deserialize)]
struct RawPayload<'a> {
    op: u8,
    d: &'a RawValue,
    s: Option<u64>,
    t: Option<&'a str>,
}

impl<'a> TryFrom<RawPayload<'a>> for Payload {
    type Error = crate::serde_utils::Error;

    fn try_from(raw: RawPayload<'a>) -> Result<Self, Self::Error> {
        let RawPayload { op, d, s, t } = raw;
        match op {
            0 => {
                // guaranteed to be present in dispatch events
                // it worked in kotlin for over a year so I think we're good
                let s = s.unwrap();
                let t = t.unwrap();

                let json = format!(r#"{{"t":"{}","d":{}}}"#, t, d);

                match nice_from_str(&json) {
                    Ok(event) => Ok(Self::Dispatch { event, seq_num: s }),
                    Err(e) => Err(e)
                }
            }
            1 => {
                let seq_num = nice_from_str(d.get())?;
                Ok(Self::Heartbeat(Heartbeat { seq_num }))
            }
            7 => {
                Ok(Self::Reconnect)
            }
            9 => {
                let resumable = nice_from_str(d.get())?;
                Ok(Self::InvalidSession(resumable))
            }
            10 => {
                Ok(Self::Hello(nice_from_str(d.get())?))
            }
            11 => {
                Ok(Self::HeartbeatAck)
            }
            2 => Err(de::Error::custom("`Identify` should not be received")),
            3 => Err(de::Error::custom("`UpdateStatus` should not be received")),
            4 => Err(de::Error::custom("`UpdateVoiceStatus` should not be received")),
            6 => Err(de::Error::custom("`Resume` should not be received")),
            8 => Err(de::Error::custom("`RequestGuildMembers` should not be received")),
            _ => Err(de::Error::custom(format!("Unrecognized opcode {}", op))),
        }
    }
}

/// Sent on connection to the websocket. Defines the heartbeat interval that the client should heartbeat to.
#[derive(Deserialize, Debug)]
pub struct HelloPayload {
    /// the interval (in milliseconds) the client should heartbeat with
    pub heartbeat_interval: u64,
}

// ser/de is handled by the implementation on `Payload`
#[derive(Debug)]
pub struct Heartbeat {
    pub seq_num: u64,
}

impl From<Heartbeat> for Payload {
    fn from(heartbeat: Heartbeat) -> Self {
        Self::Heartbeat(heartbeat)
    }
}

impl Display for Heartbeat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Heartbeat {{ {} }}", self.seq_num)
    }
}

/// Used to trigger the initial handshake with the gateway.
#[derive(Serialize, Debug, Clone)]
pub struct Identify {
    /// authentication token
    pub(crate) token: String,
    /// connection properties (doesn't really seem to matter)
    properties: ConnectionProperties,
    /// whether this connection supports compression of packets
    #[serde(skip_serializing_if = "Option::is_none")]
    compress: Option<bool>,
    /// value between 50 and 250, total number of members where the gateway will stop sending offline members in the guild member list
    #[serde(skip_serializing_if = "Option::is_none")]
    large_threshold: Option<u8>,
    /// used for Guild Sharding
    #[serde(skip_serializing_if = "Option::is_none")]
    shard: Option<(u32, u32)>,
    /// presence structure for initial presence information
    #[serde(skip_serializing_if = "Option::is_none")]
    presence: Option<UpdateStatus>,
    /// enables dispatching of guild subscription events (presence and typing events)
    #[serde(skip_serializing_if = "Option::is_none")]
    guild_subscriptions: Option<bool>,
    /// the Gateway Intents you wish to receive
    intents: Intents,
}

impl Identify {
    /// Used to create an instance of [Identify]. This struct uses the builder pattern to configure
    /// the optional fields. [intents](Indentify::intents) defaults to all non-priveleged intents.
    // todo remove when CLion can tell where `all()` comes from
    //noinspection ALL
    pub fn new(token: String) -> Self {
        Self {
            token,
            properties: Default::default(),
            compress: None,
            large_threshold: None,
            shard: None,
            presence: None,
            guild_subscriptions: None,
            intents: Intents::all() ^ Intents::PRIVELEGED,
        }
    }

    /// Set the bot's precence when initially connecting.
    pub fn presence(mut self, presence: UpdateStatus) -> Self {
        self.presence = Some(presence);
        self
    }

    /// Override the default intents (all non-privileged intents).
    pub const fn intents(mut self, intents: Intents) -> Self {
        self.intents = intents;
        self
    }
}

impl From<Identify> for Payload {
    fn from(identify: Identify) -> Self {
        Self::Identify(identify)
    }
}

impl Display for Identify {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut debug_struct = f.debug_struct("Identify");
        debug_struct.field("properties", &self.properties);
        if let Some(compress) = &self.compress {
            debug_struct.field("compress", compress);
        }
        if let Some(large_threshold) = &self.large_threshold {
            debug_struct.field("large_threshold", large_threshold);
        }
        if let Some(shard) = &self.shard {
            debug_struct.field("shard", shard);
        }
        if let Some(presence) = &self.presence {
            debug_struct.field("presence", presence);
        }
        if let Some(guild_subscriptions) = &self.guild_subscriptions {
            debug_struct.field("guild_subscriptions", guild_subscriptions);
        }
        debug_struct.field("intents", &self.intents);
        debug_struct.finish()
    }
}

/// Use the impl of Default
#[derive(Serialize, Debug, Clone)]
pub struct ConnectionProperties {
    #[serde(rename = "$os")]
    os: String,
    #[serde(rename = "$browser")]
    browser: String,
    #[serde(rename = "$device")]
    device: String,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            os: "windows".into(),
            browser: "AvBotR".into(),
            device: "AvBotR".into(),
        }
    }
}

/// Used to replay missed events when a disconnected client resumes.
#[derive(Serialize, Debug)]
pub struct Resume {
    /// session token
    pub token: String,
    /// session id
    pub session_id: String,
    /// last sequence number received
    pub seq: u64,
}

impl From<Resume> for Payload {
    fn from(resume: Resume) -> Self {
        Self::Resume(resume)
    }
}

/// Don't display the token, this impl is used in `Shard::send`
impl Display for Resume {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Resume")
            .field("session_id", &self.session_id)
            .field("seq", &self.seq)
            .finish()
    }
}

/// Used to request all members for a guild or a list of guilds. When initially connecting, the
/// gateway will only send offline members if a guild has less than the `large_threshold` members
/// (value in the [Identify]). If a client wishes to receive additional members, they need to
/// explicitly request them via this operation. The server will send [Guild Members Chunk todo]
/// events in response with up to 1000 members per chunk until all members that match the request
/// have been sent.
///
/// This struct follows the builder pattern for configuring what to request. Create an instance with
/// `all`, `query`, or `users`.
// todo check that GUILD_PRESENCES, GUILD_MEMBERS intents are true depending on whats here
#[derive(Serialize, Debug)]
pub struct RequestGuildMembers {
    /// id of the guild to get members for
    guild_id: GuildId,
    /// string that username starts with, or an empty string to return all members
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<String>,
    /// maximum number of members to send matching the `query`; a limit of `0` can be used with an
    /// empty string `query` to return all members
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u32>,
    /// used to specify if we want the presences of the matched members
    #[serde(skip_serializing_if = "bool::is_false")]
    presences: bool,
    /// used to specify which users you wish to fetch
    #[serde(skip_serializing_if = "Option::is_none")]
    user_ids: Option<Vec<UserId>>,
    /// nonce to identify the [Guild Members Chunk] response todo
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<String>,
}

#[allow(dead_code)]
impl RequestGuildMembers {
    pub fn all(guild: GuildId) -> Self {
        Self::query(guild, "")
    }

    pub fn query(guild_id: GuildId, query: impl Into<String>) -> Self {
        Self {
            guild_id,
            query: Some(query.into()),
            limit: Some(0),
            presences: false,
            user_ids: None,
            nonce: None,
        }
    }

    pub fn users(guild_id: GuildId, users: &[UserId]) -> Self {
        Self {
            guild_id,
            query: None,
            limit: None,
            presences: false,
            user_ids: Some(users.to_vec()),
            nonce: None,
        }
    }

    pub const fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub const fn presences(mut self, presences: bool) -> Self {
        self.presences = presences;
        self
    }

    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }
}

impl From<RequestGuildMembers> for Payload {
    fn from(rgm: RequestGuildMembers) -> Self {
        Self::RequestGuildMembers(rgm)
    }
}

// todo a builder for this
/// Sent by the client to indicate a presence or status update.
#[derive(Serialize, Debug, Clone)]
pub struct UpdateStatus {
    /// unix time (in milliseconds) of when the client went idle, or null if the client is not idle
    pub since: Option<u64>,
    /// null, or the user's activities
    pub activities: Option<Vec<Activity>>,
    /// the user's new status
    pub status: StatusType,
    /// whether or not the client is afk
    pub afk: bool,
}

impl From<UpdateStatus> for Payload {
    fn from(us: UpdateStatus) -> Self {
        Self::UpdateStatus(us)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum StatusType {
    /// Online
    Online,
    /// Do Not Disturb
    Dnd,
    /// AFK
    Idle,
    /// Invisible and shown as offline
    Invisible,
    /// Offline
    Offline,
}

/// Bots are only able to send name, type, and optionally url.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Activity {
    /// the activity's name
    pub name: String,
    /// activity type
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    /// stream url, is validated when type is 1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// unix timestamp of when the activity was added to the user's session
    pub created_at: Option<u64>,
    /// unix timestamps for start and/or end of the game
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    /// application id for the game
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// what the player is currently doing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// the user's current party status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// the emoji used for a custom status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<Emoji>,
    /// information for the current party of the player
    #[serde(skip_serializing_if = "Option::is_none")]
    pub party: Option<Party>,
    /// images for the presence and their hover texts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Assets>,
    /// secrets for Rich Presence joining and spectating
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Secrets>,
    /// whether or not the activity is an instanced game session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<bool>,
    /// activity flags `OR`d together, describes what the payload includes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<ActivityFlags>,
}

impl Activity {
    /// create an activity with a [name](Self::name) and [`activity_type`](Self::activity_type), two of the three fields
    /// bots are able to send
    pub fn for_bot<N, /*O, U*/>(name: N, activity_type: ActivityType) -> Self
        where N: Into<String>,
    // O: Into<Option<U>>,
    // U: Into<String>
    {
        Self {
            name: name.into(),
            activity_type,
            url: None,
            /*url.into().map(|u| u.into()),*/
            created_at: None,
            timestamps: None,
            application_id: None,
            details: None,
            state: None,
            emoji: None,
            party: None,
            assets: None,
            secrets: None,
            instance: None,
            flags: None,
        }
    }
}

#[derive(Deserialize_repr, Serialize_repr, Debug, Clone)]
#[repr(u8)]
pub enum ActivityType {
    /// Format: `Playing {name}`
    Game = 0,
    /// Format: `Streaming {details}`
    ///
    /// The streaming type currently only supports Twitch and YouTube. Only https://twitch.tv/ and
    /// https://youtube.com/ urls will work.
    Steaming = 1,
    /// Format: `Listening to {name}`
    Listening = 2,
    /// Format: `{emoji} {name}`
    Custom = 4,
    /// Format: `Competing in {name}`
    Competing = 5,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Timestamps {
    /// unix time (in milliseconds) of when the activity started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u64>,
    /// unix time (in milliseconds) of when the activity ends
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Party {
    /// the id of the party
    pub id: String,
    /// used to show the party's current and maximum size
    ///
    /// (current_size, max_size)
    pub size: (u64, u64),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Assets {
    /// the id for a large asset of the activity, usually a snowflake
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    /// text displayed when hovering over the large image of the activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    /// the id for a small asset of the activity, usually a snowflake
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    /// text displayed when hovering over the small image of the activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Secrets {
    /// the secret for joining a party
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    /// the secret for spectating a game
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectate: Option<String>,
    /// the secret for a specific instanced match
    #[serde(rename = "match")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#match: Option<String>,
}

bitflags! {
    pub struct ActivityFlags: u8 {
        const INSTANCE = 1 << 0;
        const JOIN = 1 << 1;
        const SPECTATE = 1 << 2;
        const JOIN_REQUEST = 1 << 3;
        const SYNC = 1 << 4;
        const PLAY = 1 << 5;
    }
}
serde_bitflag!(ActivityFlags, u8);

/// Sent when a client wants to join, move, or disconnect from a voice channel.
#[derive(Serialize, Debug)]
pub struct UpdateVoiceStatus {
    /// id of the guild
    pub guild_id: GuildId,
    /// id of the voice channel client wants to join (`None` if disconnecting)
    pub channel_id: Option<ChannelId>,
    /// is the client muted
    pub self_mute: bool,
    /// is the client deafened
    pub self_deaf: bool,
}
