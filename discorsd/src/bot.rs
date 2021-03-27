use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::Arc;

use async_trait::async_trait;
use log::error;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;

use crate::cache::Cache;
use crate::commands::{ReactionCommand, SlashCommand};
use crate::errors::BotError;
use crate::http::{ClientResult, DiscordClient};
use crate::model::commands::InteractionUse;
use crate::model::guild::{Guild, Integration};
use crate::model::ids::*;
use crate::model::interaction::Interaction;
use crate::model::message::Message;
use crate::model::user::User;
use crate::shard;
use crate::shard::dispatch::{MessageUpdate, PartialApplication, ReactionUpdate};
use crate::shard::model::Identify;
use crate::shard::Shard;

pub type GuildCommands<B> = HashMap<CommandId, Box<dyn SlashCommand<Bot=B>>>;

pub struct BotState<B: Send + Sync + 'static> {
    pub client: DiscordClient,
    pub cache: Cache,
    pub bot: B,
    pub commands: RwLock<HashMap<GuildId, RwLock<GuildCommands<B>>>>,
    pub global_commands: OnceCell<HashMap<CommandId, &'static dyn SlashCommand<Bot=B>>>,
    pub reaction_commands: RwLock<Vec<Box<dyn ReactionCommand<B>>>>,
}

impl<B: Send + Sync> AsRef<BotState<B>> for BotState<B> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<B: Send + Sync> BotState<B> {
    /// gets the current user
    ///
    /// panics if somehow used before [`Ready`](crate::shard::dispatch::Ready) is received
    pub async fn user(&self) -> User {
        self.cache.own_user().await
    }

    /// gets the bot's `ApplicationId`. The first time this is called, performs the
    /// [`DiscordClient::application_information`](DiscordClient::application_information)
    /// get request, otherwise recalls the id from the cache.
    ///
    /// panics if [`DiscordClient::application_information`](DiscordClient::application_information)
    /// fails
    pub async fn application_id(&self) -> ApplicationId {
        let id = {
            self.cache.application.read().await.map(|app| app.id)
        };
        if let Some(id) = id {
            id
        } else {
            let app = self.client.application_information().await
                .expect("application_information should not fail");
            *self.cache.application.write().await = Some(PartialApplication { id: app.id, flags: app.flags });
            app.id
        }
    }
}

impl<B: Debug + Send + Sync> Debug for BotState<B> {
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
    fn token(&self) -> &str;

    fn identify(&self) -> Identify { Identify::new(self.token().to_string()) }

    // todo make this const generic array?
    fn global_commands() -> &'static [&'static dyn SlashCommand<Bot=Self>] { &[] }

    async fn ready(&self, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn resumed(&self, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn guild_create(&self, guild: Guild, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn message_create(&self, message: Message, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn message_update(&self, message: Message, state: Arc<BotState<Self>>, updates: MessageUpdate) -> Result<(), BotError> { Ok(()) }

    async fn interaction(&self, interaction: Interaction, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn reaction(&self, reaction: ReactionUpdate, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn integration_update(&self, guild: GuildId, integration: Integration, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn error(&self, error: BotError, state: Arc<BotState<Self>>) {
        error!("{}", error.display_error(&state).await);
    }
}

#[async_trait]
pub trait BotExt: Bot + 'static {
    async fn run(self) -> shard::ShardResult<()> {
        BotRunner::from(self).run().await
    }

    /// The first time connecting to a guild, run this to delete any commands Discord has saved from
    /// the last time the bot was started
    async fn clear_old_commands(
        guild: GuildId,
        state: &BotState<Self>,
    ) -> ClientResult<()> {
        let mut commands = state.commands.write().await;
        let first_time = !commands.contains_key(&guild);
        let mut commands = commands.entry(guild)
            .or_default()
            .write().await;
        if first_time {
            let app = state.application_id().await;
            match state.client.get_guild_commands(app, guild).await {
                Ok(old_commands) => {
                    for command in old_commands {
                        let delete = state.client
                            .delete_guild_command(app, guild, command.id)
                            .await;
                        if let Err(e) = delete {
                            error!("{}", e.display_error(state).await);
                        }
                        commands.remove(&command.id);
                    }
                }
                Err(e) => error!("{}", e.display_error(state).await)
            }
        }
        Ok(())
    }

    async fn slash_command(interaction: Interaction, state: Arc<BotState<Self>>) -> Result<(), BotError> {
        let (interaction, data) = InteractionUse::from(interaction);

        let command = state.global_commands.get().unwrap().get(&interaction.command);
        if let Some(command) = command {
            command.run(state, interaction, data).await?;
        } else {
            let command = {
                let guard = state.commands.read().await;
                // todo fix this unwrap lol
                let commands = guard.get(&interaction.guild().unwrap()).unwrap().read().await;
                commands.get(&interaction.command).cloned()
            };
            if let Some(command) = command {
                command.run(state, interaction, data).await?;
            }
        };
        Ok(())
    }
}

#[async_trait]
impl<B: Bot + 'static> BotExt for B {}

struct BotRunner<B: Bot + 'static> {
    shards: Vec<Shard<B>>,
}

impl<B: Bot + 'static> From<B> for BotRunner<B> {
    fn from(bot: B) -> Self {
        let state = Arc::new(BotState {
            client: DiscordClient::single(bot.token().to_string()),
            cache: Default::default(),
            bot,
            commands: Default::default(),
            global_commands: Default::default(),
            reaction_commands: Default::default(),
        });
        // todo more than one shard
        let shard = Shard::new(Arc::clone(&state));
        Self {
            shards: vec![shard]
        }
    }
}

impl<B: Bot + 'static> BotRunner<B> {
    async fn run(self) -> shard::ShardResult<()> {
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
                Ok((id, _handle)) => {
                    error!("Shard {:?} finished (this should be unreachable?)", id);
                    // handle.unwrap();
                }
                Err(e) => {
                    error!("this is awkward, I didn't expect {}", e);
                }
            }
        }
        unreachable!()
        // Err(ShardError::Other("Shouldn't stop running".into()))
    }
}