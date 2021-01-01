use std::convert::TryFrom;

use chrono::{DateTime, Utc};
use num_enum::TryFromPrimitive;
use serde::{de, Deserialize, Serialize, Serializer};
use serde::de::Unexpected;
use serde::ser::SerializeStruct;
use serde_json::value::RawValue;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::http::model::Id;
use crate::http::model::ids::{ApplicationId, ChannelId, GuildId, MessageId, RoleId, UserId, WebhookId};
use crate::http::model::user::User;
use crate::serde_utils::nice_from_str;

// This can be gotten rid of once serde can ser/de an enum tagged by an int
#[derive(Deserialize)]
struct RawChannel<'a>(#[serde(borrow)] &'a RawValue);

impl<'a> RawChannel<'a> {
    fn channel_type(&self) -> Result<u32, Option<char>> {
        // we want to be one level in the object
        let mut nesting = 0;
        // index in `"type":` we are at rn
        //           0123456
        let mut progress = 0;
        let str = self.0.get();
        let char = str.find(|c: char| {
            match c {
                '{' => {
                    progress = 0;
                    nesting += 1
                }
                '}' => {
                    progress = 0;
                    nesting -= 1
                }
                '"' if nesting == 1 && progress == 0 => progress += 1,
                't' if nesting == 1 && progress == 1 => progress += 1,
                'y' if nesting == 1 && progress == 2 => progress += 1,
                'p' if nesting == 1 && progress == 3 => progress += 1,
                'e' if nesting == 1 && progress == 4 => progress += 1,
                '"' if nesting == 1 && progress == 5 => progress += 1,
                ':' if nesting == 1 && progress == 6 => progress += 1,
                _ => progress = 0,
            }
            progress == 7
        }).and_then(|idx| str.chars().nth(idx + 1)); // idx is where ':' is
        if let Some(char) = char {
            char.to_digit(10).ok_or(Some(char))
        } else {
            Err(None)
        }
    }
}

impl<'a> TryFrom<RawChannel<'a>> for Channel {
    type Error = crate::serde_utils::Error;

    fn try_from(raw: RawChannel<'a>) -> Result<Self, Self::Error> {
        let channel_type = raw.channel_type()
            .map_err(|opt| if let Some(char) = opt {
                de::Error::invalid_value(Unexpected::Char(char), &"0..=6")
            } else {
                de::Error::missing_field("type")
            })?;
        let channel_type = ChannelType::try_from(channel_type as u8)
            .map_err(|_| de::Error::invalid_value(Unexpected::Unsigned(channel_type as _), &"0..=6"))?;

        Ok(match channel_type {
            ChannelType::GuildText => Self::Text(nice_from_str(raw.0.get())?),
            ChannelType::Dm => Self::Dm(nice_from_str(raw.0.get())?),
            ChannelType::GuildVoice => Self::Voice(nice_from_str(raw.0.get())?),
            ChannelType::GroupDm => Self::GroupDm(nice_from_str(raw.0.get())?),
            ChannelType::GuildCategory => Self::Category(nice_from_str(raw.0.get())?),
            ChannelType::GuildNews => Self::News(nice_from_str(raw.0.get())?),
            ChannelType::GuildStore => Self::Store(nice_from_str(raw.0.get())?),
        })
    }
}

impl Serialize for Channel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Shim<'a, C> {
            #[serde(flatten)]
            channel: &'a C,
            #[serde(rename = "type")]
            t: u8,
        }

        match self {
            Channel::Text(text) => Shim { channel: text, t: 0 }.serialize(s),
            Channel::Dm(dm) => Shim { channel: dm, t: 1 }.serialize(s),
            Channel::Voice(voice) => Shim { channel: voice, t: 2 }.serialize(s),
            Channel::GroupDm(group_dm) => Shim { channel: group_dm, t: 3 }.serialize(s),
            Channel::Category(category) => Shim { channel: category, t: 4 }.serialize(s),
            Channel::News(news) => Shim { channel: news, t: 5 }.serialize(s),
            Channel::Store(store) => Shim { channel: store, t: 6 }.serialize(s),
        }
    }
}

/// Represents a guild or DM channel within Discord.
#[derive(Deserialize, Debug, Clone)]
#[serde(try_from = "RawChannel")]
pub enum Channel {
    /// a text channel within a server
    Text(TextChannel),
    /// a direct message between users
    Dm(DmChannel),
    /// a voice channel within a server
    Voice(VoiceChannel),
    /// a direct message between multiple users
    GroupDm(GroupDmChannel),
    /// an [organizational category](https://support.discord.com/hc/en-us/articles/115001580171-Channel-Categories-101)
    /// that contains up to 50 channels
    Category(CategoryChannel),
    /// a channel that [users can follow and crosspost into their own server](https://support.discord.com/hc/en-us/articles/360032008192)
    News(NewsChannel),
    /// a channel in which game developers can
    /// [sell their game on Discord](https://discord.com/developers/docs/game-and-server-management/special-channels)
    Store(StoreChannel),
}

impl Channel {
    pub const fn channel_type(&self) -> ChannelType {
        match self {
            Channel::Text(_) => ChannelType::GuildText,
            Channel::Dm(_) => ChannelType::Dm,
            Channel::Voice(_) => ChannelType::GuildVoice,
            Channel::GroupDm(_) => ChannelType::GroupDm,
            Channel::Category(_) => ChannelType::GuildCategory,
            Channel::News(_) => ChannelType::GuildNews,
            Channel::Store(_) => ChannelType::GuildStore,
        }
    }

    pub fn guild_id(&self) -> Option<GuildId> {
        match self {
            Channel::Text(t) => t.guild_id,
            Channel::Voice(v) => v.guild_id,
            Channel::Category(c) => c.guild_id,
            Channel::News(n) => n.guild_id,
            Channel::Store(s) => s.guild_id,
            Channel::Dm(_) | Channel::GroupDm(_) => None,
        }
    }
}

id_eq!(Channel);
impl Id for Channel {
    type Id = ChannelId;

    fn id(&self) -> ChannelId {
        match self {
            Channel::Text(c) => c.id,
            Channel::Dm(c) => c.id,
            Channel::Voice(c) => c.id,
            Channel::GroupDm(c) => c.id,
            Channel::Category(c) => c.id,
            Channel::News(c) => c.id,
            Channel::Store(c) => c.id,
        }
    }
}

// impl AsRef<ChannelId> for Channel {
//     fn as_ref(&self) -> &ChannelId {
//         match self {
//             Channel::Text(c) => c.as_ref(),
//             Channel::Dm(c) => c.as_ref(),
//             Channel::Voice(c) => c.as_ref(),
//             Channel::GroupDm(c) => c.as_ref(),
//             Channel::Category(c) => c.as_ref(),
//             Channel::News(c) => c.as_ref(),
//             Channel::Store(c) => c.as_ref(),
//         }
//     }
// }

/// a text channel within a server
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TextChannel {
    /// the id of this channel
    pub id: ChannelId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// sorting position of the channel
    pub position: u32,
    /// explicit permission overwrites for members and roles
    pub permission_overwrites: Vec<Overwrite>,
    /// the name of the channel (2-100 characters)
    pub name: String,
    /// the channel topic (0-1024 characters)
    pub topic: Option<String>,
    #[serde(default)]
    /// whether the channel is nsfw
    pub nsfw: bool,
    /// the id of the last message sent in this channel (may not point to an existing or valid message)
    pub last_message_id: Option<MessageId>,
    /// amount of seconds a user has to wait before sending another message (0-21600); bots, as well
    /// as users with the permission `manage_messages` or `manage_channel`, are unaffected
    pub rate_limit_per_user: Option<u32>,
    /// id of the parent category for a channel (each parent category can contain up to 50 channels)
    pub parent_id: Option<ChannelId>,
    /// when the last pinned message was pinned. This may be `None` in events such as `GUILD_CREATE`
    /// when a message is not pinned.
    pub last_pin_timestamp: Option<DateTime<Utc>>,
}

id_eq!(TextChannel);
impl Id for TextChannel {
    type Id = ChannelId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

// todo: this should have last_message_id?
/// a direct message between users
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DmChannel {
    /// the id of this channel
    pub id: ChannelId,
    /// the id of the last message sent in this channel (may not point to an existing or valid message)
    pub last_message_id: Option<MessageId>,
    /// the recipients of the DM
    #[serde(rename = "recipients", with = "one_recipient")]
    pub recipient: User,
    /// when the last pinned message was pinned. This may be `None` in events such as `GUILD_CREATE`
    /// when a message is not pinned.
    pub last_pin_timestamp: Option<DateTime<Utc>>,
}

id_eq!(DmChannel);
impl Id for DmChannel {
    type Id = ChannelId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

mod one_recipient {
    use serde::ser::SerializeSeq;
    use serde::{Serializer, Deserializer, Deserialize};

    use crate::http::model::User;

    pub fn serialize<S: Serializer>(recipient: &User, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(1))?;
        seq.serialize_element(recipient)?;
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<User, D::Error> {
        let [id] = <[User; 1]>::deserialize(d)?;
        Ok(id)
    }
}

/// a voice channel within a server
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VoiceChannel {
    /// the id of this channel
    pub id: ChannelId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// sorting position of the channel
    pub position: u32,
    /// explicit permission overwrites for members and roles
    pub permission_overwrites: Vec<Overwrite>,
    /// the name of the channel (2-100 characters)
    pub name: String,
    /// the bitrate (in bits) of the voice channel
    pub bitrate: u32,
    /// the user limit of the voice channel
    pub user_limit: u32,
    /// id of the parent category for a channel (each parent category can contain up to 50 channels)
    pub parent_id: Option<ChannelId>,
}

id_eq!(VoiceChannel);
impl Id for VoiceChannel {
    type Id = ChannelId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

/// a direct message between multiple users
///
/// bots cannot be in these channels
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GroupDmChannel {
    /// the id of this channel
    pub id: ChannelId,
    /// the id of the last message sent in this channel (may not point to an existing or valid message)
    pub last_message_id: Option<MessageId>,
    /// the recipients of the DM
    pub recipients: Vec<User>,
    /// icon hash
    pub icon: Option<String>,
    /// id of the DM creator
    pub owner_id: UserId,
    /// application id of the group DM creator if it is bot-created
    pub application_id: Option<ApplicationId>,
    /// when the last pinned message was pinned. This may be `None` in events such as `GUILD_CREATE`
    /// when a message is not pinned.
    pub last_pin_timestamp: Option<DateTime<Utc>>,
}

id_eq!(GroupDmChannel);
impl Id for GroupDmChannel {
    type Id = ChannelId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

/// an [organizational category](https://support.discord.com/hc/en-us/articles/115001580171-Channel-Categories-101)
/// within a server that contains up to 50 channels
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CategoryChannel {
    /// the id of this channel
    pub id: ChannelId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// sorting position of the channel
    pub position: u32,
    /// explicit permission overwrites for members and roles
    pub permission_overwrites: Vec<Overwrite>,
    /// the name of the channel (2-100 characters)
    pub name: Option<String>,
    /// whether the channel is nsfw
    #[serde(default)]
    pub nsfw: bool,
}

id_eq!(CategoryChannel);
impl Id for CategoryChannel {
    type Id = ChannelId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

/// These are called "Announcement Channels" in the client.
/// A channel that [users can follow and crosspost into their own server](https://support.discord.com/hc/en-us/articles/360032008192).
///
/// Bots can post or publish messages in this type of channel if they have the proper permissions.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NewsChannel {
    /// the id of this channel
    pub id: ChannelId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// sorting position of the channel
    pub position: u32,
    /// explicit permission overwrites for members and roles
    pub permission_overwrites: Vec<Overwrite>,
    /// the name of the channel (2-100 characters)
    pub name: String,
    /// the channel topic (0-1024 characters)
    pub topic: Option<String>,
    /// whether the channel is nsfw
    #[serde(default)]
    pub nsfw: bool,
    /// the id of the last message sent in this channel (may not point to an existing or valid message)
    pub last_message_id: Option<MessageId>,
    /// id of the parent category for a channel (each parent category can contain up to 50 channels)
    pub parent_id: Option<ChannelId>,
    /// when the last pinned message was pinned. This may be `None` in events such as `GUILD_CREATE`
    /// when a message is not pinned.
    pub last_pin_timestamp: Option<DateTime<Utc>>,
}

id_eq!(NewsChannel);
impl Id for NewsChannel {
    type Id = ChannelId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

/// a channel in which game developers can
/// [sell their game on Discord](https://discord.com/developers/docs/game-and-server-management/special-channels)
///
/// Bots can neither send or read messages from this channel type (as it is a store page).
// todo not sure if everything relevant is here
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StoreChannel {
    /// the id of this channel
    pub id: ChannelId,
    /// the id of the guild
    pub guild_id: Option<GuildId>,
    /// sorting position of the channel
    pub position: u32,
    /// explicit permission overwrites for members and roles
    pub permission_overwrites: Vec<Overwrite>,
    /// the name of the channel (2-100 characters)
    pub name: String,
    /// whether the channel is nsfw
    #[serde(default)]
    pub nsfw: bool,
    /// id of the parent category for a channel (each parent category can contain up to 50 channels)
    pub parent_id: Option<ChannelId>,
}

id_eq!(StoreChannel);
impl Id for StoreChannel {
    type Id = ChannelId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

#[derive(Serialize_repr, Deserialize_repr, Debug, TryFromPrimitive, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChannelType {
    /// a text channel within a server
    GuildText = 0,
    /// a direct message between users
    Dm = 1,
    /// a voice channel within a server
    GuildVoice = 2,
    /// a direct message between multiple users
    GroupDm = 3,
    /// an [organizational category](https://support.discord.com/hc/en-us/articles/115001580171-Channel-Categories-101)
    /// that contains up to 50 channels
    GuildCategory = 4,
    /// a channel that [users can follow and crosspost into their own server](https://support.discord.com/hc/en-us/articles/360032008192)
    GuildNews = 5,
    /// a channel in which game developers can
    /// [sell their game on Discord](https://discord.com/developers/docs/game-and-server-management/special-channels)
    GuildStore = 6,
}

/// See [permissions](https://discord.com/developers/docs/topics/permissions#permissions)
/// for more information about the `allow` and `deny` fields.
#[derive(Deserialize, Debug, Clone)]
#[serde(try_from = "RawOverwrite")]
pub struct Overwrite {
    /// role or user id
    pub id: OverwriteType,
    // todo permission struct
    /// permission bit set
    pub allow: u64,
    /// permission bit set
    pub deny: u64,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
pub enum OverwriteType {
    Role(RoleId),
    Member(UserId),
}

impl Serialize for Overwrite {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut overwrite = s.serialize_struct("Overwrite", 4)?;
        match self.id {
            OverwriteType::Role(id) => {
                overwrite.serialize_field("id", &id)?;
                overwrite.serialize_field("type", &0)?;
            }
            OverwriteType::Member(id) => {
                overwrite.serialize_field("id", &id)?;
                overwrite.serialize_field("type", &1)?;
            }
        }
        overwrite.serialize_field("allow", &self.allow)?;
        overwrite.serialize_field("deny", &self.deny)?;
        overwrite.end()
    }
}

// Exists to mediate deserialization to Overwrite
#[derive(Deserialize)]
struct RawOverwrite<'a> {
    #[serde(rename = "type")]
    otype: u8,
    #[serde(borrow)]
    id: &'a RawValue,
    allow: String,
    deny: String,
}

impl<'a> TryFrom<RawOverwrite<'a>> for Overwrite {
    type Error = crate::serde_utils::Error;

    fn try_from(RawOverwrite { otype, id, allow, deny }: RawOverwrite<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: match otype {
                0 => {
                    OverwriteType::Role(nice_from_str(id.get())?)
                }
                1 => {
                    OverwriteType::Member(nice_from_str(id.get())?)
                }
                _ => return Err(de::Error::custom("should only receive `type` of 0 or 1")),
            },
            allow: allow.parse().map_err(de::Error::custom)?,
            deny: deny.parse().map_err(de::Error::custom)?,
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct FollowedChannel {
    /// source channel id
    pub channel_id: ChannelId,
    /// created target webhook id
    pub webhook_id: WebhookId,
}