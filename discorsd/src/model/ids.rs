//! The `snowflake` types Discord uses to identify different objects.

use std::fmt::{self, Display};
use std::num::ParseIntError;
use std::str::FromStr;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::Error;

use crate::model::ids::sealed::IsId;

const DISCORD_EPOCH: u64 = 1_420_070_400_000;

macro_rules! id_impl {
    ($($id:tt,)+) => {
        $(
            #[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
            pub struct $id(pub u64);

            impl $id {
                /// For every ID that is generated on that process, this number is incremented
                pub fn timestamp(&self) -> DateTime<Utc> {
                    let millis = (self.0 >> 22) + DISCORD_EPOCH;
                    let seconds = millis / 1000;
                    let nanos = (millis % 1000) * 1_000_000;

                    let dt = NaiveDateTime::from_timestamp(seconds as _, nanos as _);
                    DateTime::from_utc(dt, Utc)
                }
            }

            impl Display for $id {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "{}", self.0)
                }
            }

            impl From<DateTime<Utc>> for $id {
                fn from(ts: DateTime<Utc>) -> Self {
                    Self((ts.timestamp_millis() as u64 - DISCORD_EPOCH) << 22)
                }
            }

            impl From<NaiveDateTime> for $id {
                fn from(ts: NaiveDateTime) -> Self {
                    Self((ts.timestamp_millis() as u64 - DISCORD_EPOCH) << 22)
                }
            }

            impl FromStr for $id {
                type Err = ParseIntError;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    Ok(Self(s.parse()?))
                }
            }

            impl<'de> Deserialize<'de> for $id {
                fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    let id = <&'de str>::deserialize(d)?;
                    let id = id.parse().map_err(D::Error::custom)?;
                    Ok(id)
                }
            }

            impl Serialize for $id {
                fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                    let str = self.0.to_string();
                    s.serialize_str(&str)
                }
            }

            impl sealed::IsId for $id {}

            impl Id for $id {
                type Id = Self;

                fn id(&self) -> Self { *self }
            }

            // impl<C: SlashCommand> CommandData<C> for $id {
            //     type Options =
            // }
        )+
    };
}

id_impl!(
    GuildId,
    ChannelId,
    UserId,
    MessageId,
    AttachmentId,
    ApplicationId,
    WebhookId,
    EmojiId,
    RoleId,
    IntegrationId,
    StickerId,
    StickerPackId,
    CommandId,
    InteractionId,
    SkuId,
    TeamId,
);

mod sealed {
    pub trait IsId: Copy + std::hash::Hash + Eq {}
}

pub trait Id: PartialEq {
    type Id: IsId;

    fn id(&self) -> Self::Id;
}
macro_rules! id_eq {
    ($id:ty) => {
        impl PartialEq for $id {
            fn eq(&self, other: &Self) -> bool {
                self.id() == other.id()
            }
        }
    };
}

impl<'a, I: Id> Id for &'a I {
    type Id = I::Id;

    fn id(&self) -> Self::Id { (*self).id() }
}

impl<'a, I: Id> Id for &'a mut I {
    type Id = I::Id;

    fn id(&self) -> Self::Id { (**self).id() }
}