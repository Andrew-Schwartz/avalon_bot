use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::Arc;

use async_trait::async_trait;
use log::error;
use once_cell::sync::OnceCell;
use tokio::sync::{RwLock, RwLockWriteGuard};

use crate::cache::Cache;
use crate::commands::{ApplicationCommand, ReactionCommand, SlashCommand, SlashCommandRaw};
use crate::errors::BotError;
use crate::http::{ClientResult, DiscordClient};
use crate::http::guild::CommandPermsExt;
use crate::model::commands::InteractionUse;
use crate::model::guild::{Guild, Integration};
use crate::model::ids::*;
use crate::model::interaction::Interaction;
use crate::model::message::Message;
use crate::model::permissions::Role;
use crate::model::user::User;
use crate::shard;
use crate::shard::dispatch::{MessageUpdate, ReactionUpdate};
use crate::shard::model::Identify;
use crate::shard::Shard;

pub type GuildCommands<B> = HashMap<CommandId, Box<dyn SlashCommandRaw<Bot=B>>>;
pub type GuildIdMap<V> = HashMap<GuildId, RwLock<V>>;

pub struct BotState<B: Send + Sync + 'static> {
    pub client: DiscordClient,
    pub cache: Cache,
    pub bot: B,
    pub commands: RwLock<GuildIdMap<GuildCommands<B>>>,
    pub command_names: RwLock<GuildIdMap<HashMap<&'static str, CommandId>>>,
    pub global_commands: OnceCell<HashMap<CommandId, &'static dyn SlashCommandRaw<Bot=B>>>,
    pub global_command_names: OnceCell<HashMap<&'static str, CommandId>>,
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

    // todo update docs
    /// gets the bot's `ApplicationId`. The first time this is called, performs the
    /// [`DiscordClient::application_information`](DiscordClient::application_information)
    /// get request, otherwise recalls the id from the cache.
    ///
    /// panics if [`DiscordClient::application_information`](DiscordClient::application_information)
    /// fails
    pub async fn application_id(&self) -> ApplicationId {
        if let Some(id) = self.cache.application.get() {
            id.id
        } else {
            todo!("if this hasn't errored in a while just get rid of this branch");
            // let app = self.client.application_information().await
            //     .expect("application_information should not fail");
            // let result = self.cache.application.set(PartialApplication { id: app.id, flags: app.flags });
            // // *self.cache.application.write().await = Some(PartialApplication { id: app.id, flags: app.flags });
            // match result {
            //     Ok(()) => app.id,
            //     // just means that the once_cell was set in the meantime, oh well, return the id
            //     Err(partial) => partial.id,
            // }
        }
    }

    /// Get the id of command [C](C) in this [guild](guild).
    ///
    /// # Note
    ///
    /// Locks [BotState::command_names](BotState::command_names) in read mode, meaning this can
    /// cause deadlocks if called while a write guard is held.
    pub async fn try_command_id<C: SlashCommand<Bot=B>>(&self, guild: GuildId) -> Option<CommandId> {
        self.command_names.read().await
            .get(&guild)?
            .read().await
            .get(C::NAME)
            .copied()
    }

    /// Get the id of command [C](C) in this [guild](guild).
    ///
    /// # Note
    ///
    /// Locks [BotState::command_names](BotState::command_names) in read mode, meaning this can
    /// cause deadlocks if called while a write guard is held.
    ///
    /// # Panics
    ///
    /// Panics if the bot is not in this [guild](guild), or if the command [C](C) does not exist
    /// in this guild.
    pub async fn command_id<C: SlashCommand<Bot=B>>(&self, guild: GuildId) -> CommandId {
        *self.command_names.read().await
            .get(&guild)
            .unwrap_or_else(|| panic!("Guild {} exists", guild))
            .read().await
            .get(C::NAME)
            .unwrap_or_else(|| panic!("{} exists", C::NAME))
    }

    /// Get the id of the global command [C](C).
    ///
    /// # Note
    ///
    /// Locks [BotState::global_command_names](BotState::global_command_names) in read mode, meaning
    /// this can cause deadlocks if called while a write guard is held.
    ///
    /// # Panics
    ///
    /// Panics if the bot has not received the [Ready](DispatchEvent::Ready) event yet, or if the
    /// command [C](C) does not exist is not a global command.
    pub async fn global_command_id<C: SlashCommand<Bot=B>>(&self) -> CommandId {
        *self.global_command_names.get()
            .expect("Bot hasn't connected yet")
            .get(C::NAME)
            .unwrap_or_else(|| panic!("{} exists", C::NAME))
    }

    pub async fn enable_command<C: SlashCommand<Bot=B>>(&self, guild: GuildId) -> ClientResult<ApplicationCommand> {
        self.command_id::<C>(guild).await
            .default_permissions(self, guild, true).await
    }

    pub async fn disable_command<C: SlashCommand<Bot=B>>(&self, guild: GuildId) -> ClientResult<ApplicationCommand> {
        self.command_id::<C>(guild).await
            .default_permissions(self, guild, false).await
    }

    #[allow(clippy::needless_lifetimes)]
    pub async fn get_command_mut<'c, C: SlashCommand<Bot=B>>(
        &self,
        guild: GuildId,
        // not ideal that it has to take this instead of the guild.
        commands: &'c mut RwLockWriteGuard<'_, GuildCommands<B>>,
    ) -> (CommandId, &'c mut C) {
        let id = self.command_id::<C>(guild).await;
        commands.get_mut(&id)
            .and_then(|c| c.downcast_mut())
            .map(|command| (id, command))
            .unwrap_or_else(|| panic!("`{}` command exists", C::NAME))
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

// impl<B> Debug for BotState<B> {s
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

    fn global_commands() -> &'static [&'static dyn SlashCommandRaw<Bot=Self>] { &[] }

    fn guild_commands() -> Vec<Box<dyn SlashCommandRaw<Bot=Self>>> { Vec::new() }

    async fn ready(&self, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn resumed(&self, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn guild_create(&self, guild: Guild, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn message_create(&self, message: Message, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn message_update(&self, message: Message, state: Arc<BotState<Self>>, updates: MessageUpdate) -> Result<(), BotError> { Ok(()) }

    async fn interaction(&self, interaction: Interaction, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn reaction(&self, reaction: ReactionUpdate, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn integration_update(&self, guild: GuildId, integration: Integration, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn role_create(&self, guild: GuildId, role: Role, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn role_update(&self, guild: GuildId, role: Role, state: Arc<BotState<Self>>) -> Result<(), BotError> { Ok(()) }

    async fn error(&self, error: BotError, state: Arc<BotState<Self>>) {
        error!("{}", error.display_error(&state).await);
    }
}

#[async_trait]
pub trait BotExt: Bot + 'static {
    async fn run(self) -> shard::ShardResult<()> {
        BotRunner::from(self).run().await
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
            command_names: Default::default(),
            global_commands: Default::default(),
            global_command_names: Default::default(),
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