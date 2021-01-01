use std::collections::hash_map::{self, Entry, HashMap};
use std::fmt;
use std::iter::{Map, FromIterator};
use std::marker::PhantomData;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::http::model::*;
use crate::http::model::Application;

pub(crate) mod update;

#[derive(Default, Debug)]
pub struct Cache {
    pub(crate) user: RwLock<Option<User>>,
    pub(crate) application: RwLock<Option<Application>>,

    // todo?
    // pub(crate) users: RwLock<IdMap<User>>,

    pub(crate) unavailable_guilds: RwLock<IdMap<UnavailableGuild>>,
    pub(crate) guilds: RwLock<IdMap<Guild>>,
    pub(crate) members: RwLock<HashMap<UserId, HashMap<GuildId, GuildMember>>>,

    pub(crate) channel_types: RwLock<HashMap<ChannelId, ChannelType>>,
    pub(crate) channels: RwLock<IdMap<TextChannel>>,
    // like this because of updates that just contain a channel id
    pub(crate) dms: RwLock<(HashMap<UserId, ChannelId>, IdMap<DmChannel>)>,
    pub(crate) categories: RwLock<IdMap<CategoryChannel>>,
    pub(crate) news: RwLock<IdMap<NewsChannel>>,
    pub(crate) stores: RwLock<IdMap<StoreChannel>>,

    pub(crate) messages: RwLock<IdMap<Message>>,
}

impl Cache {
    /// gets the current user
    ///
    /// panics if somehow used before [Ready](crate::shard::dispatch::Ready) is received
    pub async fn user(&self) -> User {
        self.user.read().await.clone().expect("should not get `bot.user` before `Ready` fires")
    }

    pub async fn channel<C: Id<Id=ChannelId>>(&self, id: C) -> Option<TextChannel> {
        self.channels.read().await.get(id).cloned()
    }

    pub async fn message<M: Id<Id=MessageId>>(&self, id: M) -> Option<Message> {
        self.messages.read().await.get(id).cloned()
    }

    pub async fn reactions<M: Id<Id=MessageId>>(&self, id: M) -> Vec<Reaction> {
        self.messages.read().await.get(id)
            .map(|m| m.reactions.clone())
            .unwrap_or_default()
    }

    pub async fn guild_channels<G, F, C>(&self, id: G, filter_map: F) -> IdMap<C> where
        G: Id<Id=GuildId>,
        C: Into<Channel> + Clone + Id<Id=ChannelId>,
        F: FnMut(&Channel) -> Option<&C>,
    {
        self.guilds.read().await.get(id).iter()
            .flat_map(|g| &g.channels)
            .filter_map(filter_map)
            .cloned()
            .collect()
    }
}

impl Cache {
    pub async fn debug(&self) -> DebugCache<'_> {
        let Self { user, application, unavailable_guilds, guilds, members, channel_types, dms, channels, categories, news, stores, messages } = self;
        #[allow(clippy::eval_order_dependence)]
        DebugCache {
            user: user.read().await,
            application: application.read().await,
            // users: users.read().await,
            unavailable_guilds: unavailable_guilds.read().await,
            guilds: guilds.read().await,
            members: members.read().await,
            channel_types: channel_types.read().await,
            channels: channels.read().await,
            dms: dms.read().await,
            categories: categories.read().await,
            news: news.read().await,
            stores: stores.read().await,
            messages: messages.read().await,
        }
    }
}

#[derive(Debug)]
pub struct DebugCache<'a> {
    user: RwLockReadGuard<'a, Option<User>>,
    application: RwLockReadGuard<'a, Option<Application>>,
    // users: RwLockReadGuard<'a, IdMap<User>>,
    unavailable_guilds: RwLockReadGuard<'a, IdMap<UnavailableGuild>>,
    guilds: RwLockReadGuard<'a, IdMap<Guild>>,
    members: RwLockReadGuard<'a, HashMap<UserId, HashMap<GuildId, GuildMember>>>,
    channel_types: RwLockReadGuard<'a, HashMap<ChannelId, ChannelType>>,
    channels: RwLockReadGuard<'a, IdMap<TextChannel>>,
    dms: RwLockReadGuard<'a, (HashMap<UserId, ChannelId>, IdMap<DmChannel>)>,
    categories: RwLockReadGuard<'a, IdMap<CategoryChannel>>,
    news: RwLockReadGuard<'a, IdMap<NewsChannel>>,
    stores: RwLockReadGuard<'a, IdMap<StoreChannel>>,
    messages: RwLockReadGuard<'a, IdMap<Message>>,
}

#[derive(Debug, Clone)]
pub struct IdMap<T: Id>(HashMap<T::Id, T>);

impl<T: Id> IdMap<T> {
    pub fn get<I: Id<Id=T::Id>>(&self, id: I) -> Option<&T> {
        self.0.get(&id.id())
    }

    pub fn insert(&mut self, new: T) {
        self.0.insert(new.id(), new);
    }

    pub fn extend<I: IntoIterator<Item=T>>(&mut self, new: I) {
        self.0.extend(
            new.into_iter()
                .map(|t| (t.id(), t))
        );
    }

    pub fn get_mut<I: Id<Id=T::Id>>(&mut self, id: I) -> Option<&mut T> {
        self.0.get_mut(&id.id())
    }

    pub fn entry<I: Id<Id=T::Id>>(&mut self, id: I) -> Entry<T::Id, T> {
        self.0.entry(id.id())
    }

    pub fn remove<I: Id<Id=T::Id>>(&mut self, id: I) {
        self.0.remove(&id.id());
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> hash_map::Values<T::Id, T> {
        self.0.values()
    }

    pub(crate) fn new(map: HashMap<T::Id, T>) -> Self {
        Self(map)
    }
}

impl<T: Id> Default for IdMap<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Id> IntoIterator for IdMap<T> {
    type Item = T;
    #[allow(clippy::type_complexity)]
    type IntoIter = Map<hash_map::IntoIter<T::Id, T>, fn((T::Id, T)) -> T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().map(|(_, t)| t)
    }
}

impl<'a, T: Id> IntoIterator for &'a IdMap<T> {
    type Item = &'a T;
    type IntoIter = hash_map::Values<'a, T::Id, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<I: Id + Serialize> Serialize for IdMap<I> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        self.iter()
            .map(|i| seq.serialize_element(i))
            .collect::<Result<(), S::Error>>()?;
        seq.end()
    }
}

impl<I: Id> FromIterator<I> for IdMap<I> {
    fn from_iter<T: IntoIterator<Item=I>>(iter: T) -> Self {
        let map = iter.into_iter()
            .map(|i| (i.id(), i))
            .collect();
        Self(map)
    }
}

impl<I: Id> FromIterator<(I::Id, I)> for IdMap<I> {
    fn from_iter<T: IntoIterator<Item=(I::Id, I)>>(iter: T) -> Self {
        Self(HashMap::from_iter(iter))
    }
}

struct IdMapVisitor<T>(PhantomData<T>);

impl<'de, T: Id + Deserialize<'de>> Visitor<'de> for IdMapVisitor<T> {
    type Value = IdMap<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a sequence of channels")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut map = IdMap::new(HashMap::with_capacity(seq.size_hint().unwrap_or(0)));

        while let Some(channel) = seq.next_element()? {
            map.insert(channel);
        }

        Ok(map)
    }
}

impl<'de, I: Id + Deserialize<'de>> Deserialize<'de> for IdMap<I> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_seq(IdMapVisitor(PhantomData))
    }
}