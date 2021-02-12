use std::fmt;

use crate::model::ids::*;
use crate::model::emoji::{CustomEmoji, Emoji};

pub trait MarkupExt: AsRef<str> {
    fn underline(&self) -> String {
        format!("__{}__", self.as_ref())
    }
}

impl<S: AsRef<str>> MarkupExt for S {}

pub trait UserMarkupExt: Id<Id=UserId> {
    fn ping(&self) -> String {
        format!("<@{}>", self.id())
    }

    fn ping_nick(&self) -> String {
        format!("<@!{}>", self.id())
    }
}

impl<I: Id<Id=UserId>> UserMarkupExt for I {}

pub trait ChannelMarkupExt: Id<Id=ChannelId> {
    fn mention(&self) -> String {
        format!("<#{}>", self.id())
    }
}

impl<I: Id<Id=ChannelId>> ChannelMarkupExt for I {}

pub trait RoleMarkupExt: Id<Id=RoleId> {
    fn mention(&self) -> String {
        format!("<@&{}>", self.id())
    }
}

impl<I: Id<Id=RoleId>> RoleMarkupExt for I {}

impl fmt::Display for CustomEmoji {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.animated {
            write!(f, "<a:{}:{}>", self.name, self.id)
        } else {
            write!(f, "<:{}:{}>", self.name, self.id)
        }
    }
}

impl fmt::Display for Emoji {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Custom(c) => c.fmt(f),
            Self::Unicode { name } => f.write_str(name),
        }
    }
}