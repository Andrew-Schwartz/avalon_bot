use serde::{Deserialize, Serialize};

use crate::model::guild::GuildMember;
use crate::model::ids::*;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VoiceState {
    /// the guild id this voice state is for
    pub guild_id: Option<GuildId>,
    /// the channel id this user is connected to
    pub channel_id: Option<GuildId>,
    /// the user id this voice state is for
    pub user_id: UserId,
    /// the guild member this voice state is for
    pub member: Option<GuildMember>,
    /// the session id for this voice state
    pub session_id: String,
    /// whether this user is deafened by the server
    pub deaf: bool,
    /// whether this user is muted by the server
    pub mute: bool,
    /// whether this user is locally deafened
    pub self_deaf: bool,
    /// whether this user is locally muted
    pub self_mute: bool,
    /// whether this user is streaming using "Go Live"
    pub self_stream: Option<bool>,
    /// whether this user's camera is enabled
    pub self_video: bool,
    /// whether this user is muted by the current user
    pub suppress: bool,
}

id_impl!(VoiceState => user_id: UserId);

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VoiceRegion {
    /// unique ID for the region
    pub id: String,
    /// name of the region
    pub name: String,
    /// true if this is a vip-only server
    pub vip: bool,
    /// true for a single server that is closest to the current user's client
    pub optimal: bool,
    /// whether this is a deprecated voice region (avoid switching to these)
    pub deprecated: bool,
    /// whether this is a custom voice region (used for events/etc)
    pub custom: bool,
}