use std::fmt::{self, Debug, Display};
use std::sync::Arc;

use async_trait::async_trait;
use log::error;

use crate::cache::Cache;
use crate::http::DiscordClient;
use crate::http::model::{ApplicationId, Interaction, User, Integration, GuildId};
use crate::http::model::guild::Guild;
use crate::http::model::message::Message;
use crate::shard::{Shard, ShardError};
use crate::shard;
use crate::shard::dispatch::{MessageUpdate, ReactionUpdate};
use crate::shard::model::Identify;

pub struct BotState<B> {
    pub client: DiscordClient,
    pub cache: Cache,
    pub bot: B,
}

impl<B> AsRef<BotState<B>> for BotState<B> {
    fn as_ref(&self) -> &BotState<B> {
        self
    }
}

impl<B> BotState<B> {
    /// gets the current user
    ///
    /// panics if somehow used before [Ready](crate::shard::dispatch::Ready) is received
    pub async fn user(&self) -> User {
        self.cache.own_user().await
    }

    /// gets the bot's `ApplicationId`. The first time this is called, performs the
    /// [DiscordClient](DiscordClient)::[application_information](DiscordClient::application_information)
    /// get request, otherwise recalls the id from the cache.
    ///
    /// panics if [DiscordClient](DiscordClient)::[application_information](DiscordClient::application_information)
    /// fails
    pub async fn application_id(&self) -> ApplicationId {
        let id = {
            let guard = self.cache.application.read().await;
            guard.as_ref().map(|app| app.id)
        };
        if let Some(id) = id {
            id
        } else {
            let app = self.client.application_information().await
                .expect("application_information should not fail");
            let id = app.id;
            *self.cache.application.write().await = Some(app);
            id
        }
    }
}

impl<B: Debug> Debug for BotState<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BotState")
            .field("client", &self.client)
            .field("cache", &self.cache)
            .field("bot", &self.bot)
            .finish()
    }
}

// impl<B> Debug for BotState<B> {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         f.debug_struct("BotState")
//             .field("client", &self.client)
//             .field("cache", &self.cache)
//             .finish()
//     }
// }

#[allow(unused)]
#[async_trait]
pub trait Bot: Send + Sync + Sized {
    type Error: Display + Send;

    fn token(&self) -> &str;

    fn identify(&self) -> Identify { Identify::new(self.token().to_string()) }

    async fn ready(&self, state: Arc<BotState<Self>>) -> Result<(), Self::Error> { Ok(()) }

    async fn resumed(&self, state: Arc<BotState<Self>>) -> Result<(), Self::Error> { Ok(()) }

    async fn guild_create(&self, guild: Guild, state: Arc<BotState<Self>>) -> Result<(), Self::Error> { Ok(()) }

    async fn message_create(&self, message: Message, state: Arc<BotState<Self>>) -> Result<(), Self::Error> { Ok(()) }

    async fn message_update(&self, message: Message, state: Arc<BotState<Self>>, updates: MessageUpdate) -> Result<(), Self::Error> { Ok(()) }

    async fn interaction(&self, interaction: Interaction, state: Arc<BotState<Self>>) -> Result<(), Self::Error> { Ok(()) }

    async fn reaction(&self, reaction: ReactionUpdate, state: Arc<BotState<Self>>) -> Result<(), Self::Error> { Ok(()) }

    async fn integration_update(&self, guild: GuildId, integration: Integration, state: Arc<BotState<Self>>) -> Result<(), Self::Error> { Ok(()) }

    async fn error(&self, error: Self::Error, state: Arc<BotState<Self>>) {
        error!("{}", error);
    }
}

#[async_trait]
pub trait BotExt: Bot + 'static {
    async fn run(self) -> shard::Result<()> {
        BotRunner::from(self)?
            .run().await
    }
}

#[async_trait]
impl<B: Bot + 'static> BotExt for B {}

struct BotRunner<B: Bot> {
    shards: Vec<Shard<B>>,
}

impl<B: Bot + 'static> BotRunner<B> {
    // todo make this From<B> now that its not async
    fn from(bot: B) -> shard::Result<Self> {
        let state = Arc::new(BotState {
            client: DiscordClient::single(bot.token().to_string()),
            cache: Default::default(),
            bot,
        });
        // todo more than one shard
        let shard = Shard::new(Arc::clone(&state))?;
        Ok(Self {
            // state,
            shards: vec![shard]
        })
    }

    async fn run(self) -> shard::Result<()> {
        let mut handles = Vec::new();
        for mut shard in self.shards {
            let handle = tokio::spawn(async move {
                (shard.shard_info, shard.run().await)
            });
            handles.push(handle);
        }
        // todo maybe this should be try_join or smth, so that if it can restart the second even if
        //  the first is still going?
        for handle in handles {
            match handle.await {
                Ok((id, handle)) => {
                    error!("Shard {:?} finished", id);
                    handle.unwrap();
                }
                Err(e) => {
                    error!("this is awkward, I didn't expect {}", e);
                }
            }
        }
        Err(ShardError::Other("Shouldn't stop running".into()))
    }
}