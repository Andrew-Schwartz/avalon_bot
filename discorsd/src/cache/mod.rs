use std::collections::hash_map::{self, Entry, HashMap};
use std::fmt;
use std::fmt::Debug;
use std::iter::{FromIterator, Map};
use std::marker::PhantomData;

use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::model::channel::{CategoryChannel, Channel, ChannelType, DmChannel, NewsChannel, StoreChannel, TextChannel};
use crate::model::guild::{Guild, GuildMember, UnavailableGuild};
use crate::model::ids::*;
use crate::model::interaction::ApplicationCommand;
use crate::model::message::{Message, Reaction};
use crate::model::permissions::Role;
use crate::model::user::User;
use crate::shard::dispatch::PartialApplication;

#[derive(Default, Debug)]
pub struct Cache {
    pub(crate) user: RwLock<Option<User>>,
    pub(crate) application: OnceCell<PartialApplication>,

    pub(crate) users: RwLock<IdMap<User>>,

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
    pub(crate) interaction_responses: RwLock<HashMap<InteractionId, Message>>,

    pub(crate) commands: RwLock<IdMap<ApplicationCommand>>,
}

impl Cache {
    pub fn application_id(&self) -> ApplicationId {
        // self.application.read().await.unwrap().id
        self.application.get().unwrap().id
    }

    /// Gets the current user.
    ///
    /// # Panics
    ///
    /// If somehow used before [`Ready`](crate::shard::dispatch::Ready) is received.
    pub async fn own_user(&self) -> User {
        self.user.read().await.clone().expect("should not get `bot.user` before `Ready` fires")
    }

    pub async fn user<U: Id<Id=UserId> + Send>(&self, id: U) -> Option<User> {
        self.users.read().await.get(id).cloned()
    }

    pub async fn member<U, G>(&self, guild: G, user: U) -> Option<GuildMember>
        where
            U: Id<Id=UserId> + Send,
            G: Id<Id=GuildId> + Send,
    {
        self.members.read().await.get(&user.id())
            .and_then(|map| map.get(&guild.id()).cloned())
    }

    pub async fn channel<C: Id<Id=ChannelId> + Send>(&self, id: C) -> Option<Channel> {
        let id = id.id();
        let channel_type = self.channel_types.read().await.get(&id).copied();
        match channel_type {
            Some(ChannelType::GuildText) => self.channels.read().await.get(&id).cloned().map(Channel::Text),
            Some(ChannelType::Dm) => self.dms.read().await.1.get(&id).cloned().map(Channel::Dm),
            Some(ChannelType::GuildCategory) => self.categories.read().await.get(&id).cloned().map(Channel::Category),
            Some(ChannelType::GuildNews) => self.news.read().await.get(&id).cloned().map(Channel::News),
            Some(ChannelType::GuildStore) => self.stores.read().await.get(&id).cloned().map(Channel::Store),
            Some(ChannelType::GroupDm) | Some(ChannelType::GuildVoice) | None => None,
        }
    }

    pub async fn text_channel<C: Id<Id=ChannelId> + Send>(&self, id: C) -> Option<TextChannel> {
        self.channels.read().await.get(id).cloned()
    }

    pub async fn guild<G: Id<Id=GuildId> + Send>(&self, id: G) -> Option<Guild> {
        self.guilds.read().await.get(id).cloned()
    }

    pub async fn message<M: Id<Id=MessageId> + Send>(&self, id: M) -> Option<Message> {
        self.messages.read().await.get(id).cloned()
    }

    pub async fn reactions<M: Id<Id=MessageId> + Send>(&self, id: M) -> Vec<Reaction> {
        self.messages.read().await.get(id)
            .map(|m| m.reactions.clone())
            .unwrap_or_default()
    }

    pub async fn guild_channels<G, F, C>(&self, id: G, filter_map: F) -> IdMap<C>
        where
            G: Id<Id=GuildId> + Send,
            C: Into<Channel> + Clone + Id<Id=ChannelId>,
            F: FnMut(&Channel) -> Option<&C> + Send,
    {
        self.guilds.read().await.get(id).iter()
            .flat_map(|g| &g.channels)
            .filter_map(filter_map)
            .cloned()
            .collect()
    }

    pub async fn try_everyone_role<G>(&self, guild: G) -> Option<Role>
        where
            G: Id<Id=GuildId> + Send,
    {
        let guard = self.guilds.read().await;
        guard.get(guild)
            .map(|g| g.roles.iter()
                .find(|r| r.name == "@everyone")
                .expect("all guilds have `@everyone` role"))
            .cloned()
    }

    // /// Assumes that the guild exists
    // pub async fn everyone_role<G>(&self, guild: G) -> Role
    //     where
    //         G: Id<Id=GuildId> + Send,
    // {
    //     self.try_everyone_role(guild).await
    //         .expect("the guild exists")
    // }

    pub async fn command<C: Id<Id=CommandId> + Send>(&self, id: C) -> Option<ApplicationCommand> {
        self.commands.read().await.get(id).cloned()
    }

    pub async fn interaction_response<I: Id<Id=InteractionId> + Send>(&self, id: I) -> Option<Message> {
        self.interaction_responses.read().await.get(&id.id()).cloned()
    }
}

impl Cache {
    pub async fn debug(&self) -> DebugCache<'_> {
        let Self {
            user,
            application,
            users,
            unavailable_guilds,
            guilds,
            members,
            channel_types,
            dms, channels,
            categories,
            news,
            stores,
            messages,
            interaction_responses,
            commands
        } = self;
        #[allow(clippy::eval_order_dependence)]
        DebugCache {
            user: user.read().await,
            application: application.get(),
            users: users.read().await,
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
            interaction_responses: interaction_responses.read().await,
            commands: commands.read().await,
        }
    }
}

#[derive(Debug)]
pub struct DebugCache<'a> {
    user: RwLockReadGuard<'a, Option<User>>,
    application: Option<&'a PartialApplication>,
    users: RwLockReadGuard<'a, IdMap<User>>,
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
    interaction_responses: RwLockReadGuard<'a, HashMap<InteractionId, Message>>,
    commands: RwLockReadGuard<'a, IdMap<ApplicationCommand>>,
}

/// A map of objects, with keys given by the object's id
#[derive(Debug, Clone)]
pub struct IdMap<T: Id>(HashMap<T::Id, T>);

#[allow(clippy::needless_pass_by_value)]
impl<T: Id> IdMap<T> {
    pub fn get<I: Id<Id=T::Id>>(&self, id: I) -> Option<&T> {
        self.0.get(&id.id())
    }

    pub fn contains<I: Id<Id=T::Id>>(&self, id: I) -> bool {
        self.0.contains_key(&id.id())
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

    pub fn remove<I: Id<Id=T::Id>>(&mut self, id: I) -> Option<T> {
        self.0.remove(&id.id())
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
        Self(HashMap::default())
    }
}

impl<T: Id> IntoIterator for IdMap<T> {
    type Item = T;
    #[allow(clippy::type_complexity)]
    type IntoIter = Map<hash_map::IntoIter<T::Id, T>, fn((T::Id, T)) -> T>;

    fn into_iter(self) -> Self::IntoIter {
        // todo: use this when stabilized
        // self.0.into_values()
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
        self.iter().try_for_each(|i| seq.serialize_element(i))?;
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

struct IdMapVisitor<I>(PhantomData<I>);

impl<'de, I> Visitor<'de> for IdMapVisitor<I>
    where I: Id + Deserialize<'de>,
          I::Id: Deserialize<'de>,
{
    type Value = IdMap<I>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a sequence of objects with ids, or a map of ids to objects")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut this = IdMap::new(HashMap::with_capacity(seq.size_hint().unwrap_or(0)));

        while let Some(channel) = seq.next_element()? {
            this.insert(channel);
        }

        Ok(this)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut this = IdMap::new(HashMap::with_capacity(map.size_hint().unwrap_or(0)));

        while let Some((_, v)) = map.next_entry::<I::Id, _>()? {
            this.insert(v);
        }

        Ok(this)
    }
}

impl<'de, I> Deserialize<'de> for IdMap<I> where
    I: Id + Deserialize<'de>,
    I::Id: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(IdMapVisitor(PhantomData))
    }
}

/// Dispatch events need to update the cache when received
#[async_trait::async_trait]
pub trait Update {
    /// update the cache, lazily cloning whatever is needed out of `self`
    async fn update(&self, cache: &Cache);
}
