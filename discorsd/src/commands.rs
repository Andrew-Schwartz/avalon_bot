use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use downcast_rs::{Downcast, impl_downcast};
use dyn_clone::{clone_trait_object, DynClone};
use futures::StreamExt;

use crate::BotState;
use crate::errors::BotError;
pub use crate::model::commands::*;
use crate::model::guild::GuildId;
pub use crate::model::interaction::*;
use crate::shard::dispatch::ReactionUpdate;

// todo this really shouldn't be here
pub async fn create_guild_commands<B, State>(
    state: State,
    guild: GuildId,
    commands: Vec<Box<dyn SlashCommand<Bot=B>>>,
) -> HashMap<CommandId, Box<dyn SlashCommand<Bot=B>>>
    where
        B: Send + Sync + 'static,
        State: AsRef<BotState<B>> + Send,
{
    let state = state.as_ref();
    let app = state.application_id().await;
    tokio::stream::iter(commands)
        .then(|command| async move {
            let resp = state.client
                .create_guild_command(app, guild, command.command())
                .await
                .unwrap_or_else(|_| panic!("when creating `{}`", command.name()));
            (resp.id, command)
        })
        .collect()
        .await
}

#[async_trait]
pub trait SlashCommandData: Sized + Send + Sync + Debug + Downcast + DynClone {
    type Bot: Send + Sync;
    type Data: DataExt + CommandArgs<Self> + Send;

    const NAME: &'static str;
    fn description(&self) -> Cow<'static, str>;

    fn options(&self) -> TopLevelOption {
        Self::Data::args(self)
    }

    async fn run(&self,
                 state: Arc<BotState<<Self as SlashCommand>::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<Used>, BotError>;
}

#[allow(clippy::use_self)]
#[async_trait]
impl<Scd: SlashCommandData> SlashCommand for Scd
    where <Self as SlashCommandData>::Bot: Send + Sync
{
    type Bot = <Self as SlashCommandData>::Bot;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn command(&self) -> Command {
        Command::new(Self::NAME, self.description(), self.options())
    }

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let data = Scd::Data::from_data(data, interaction.guild().unwrap())?;
        SlashCommandData::run(self, state, interaction, data).await
    }
}

#[async_trait]
pub trait SlashCommand: Send + Sync + Debug + Downcast + DynClone {
    type Bot: Send + Sync;

    fn name(&self) -> &'static str;

    fn command(&self) -> Command;

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError>;
}
impl_downcast!(SlashCommand assoc Bot);
// clone_trait_object!(SlashCommand);

impl<'clone, B> Clone for Box<dyn SlashCommand<Bot=B> + 'clone> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}

#[async_trait]
pub trait ReactionCommand<B: Send + Sync>: Send + Sync + Debug + Downcast + DynClone {
    fn applies(&self, reaction: &ReactionUpdate) -> bool;

    async fn run(&self,
                 state: Arc<BotState<B>>,
                 reaction: ReactionUpdate,
    ) -> Result<(), BotError>;
}
impl_downcast!(ReactionCommand<B> where B: Send + Sync);
clone_trait_object!(<B> ReactionCommand<B> where B: Send + Sync);