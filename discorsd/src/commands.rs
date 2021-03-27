use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use downcast_rs::{Downcast, impl_downcast};
use dyn_clone::{clone_trait_object, DynClone};
use futures::StreamExt;

use crate::BotState;
use crate::errors::{BotError, CommandParseErrorInfo};
pub use crate::model::commands::*;
use crate::model::guild::GuildId;
use crate::model::ids::CommandId;
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
pub trait SlashCommandData: Sized + Send + Sync + Debug + Downcast + DynClone
// where <<Self as SlashCommandData>::Data as CommandData<Self>>::Data: std::marker::Send,
{
    type Bot: Send + Sync;
    type Data: CommandData<Self> + Send;

    const NAME: &'static str;
    fn description(&self) -> Cow<'static, str>;

    fn options(&self) -> TopLevelOption {
        <Self::Data as CommandData<Self>>::VecArg::tlo_ctor()(Self::Data::make_args(self))
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
// where
// <Self as SlashCommandData>::Bot: Send + Sync,
// <<Self as SlashCommandData>::Data as CommandData<Self>>::Options: Send,
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
                 data: InteractionDataOption,
    ) -> Result<InteractionUse<Used>, BotError> {
        match <<Self as SlashCommandData>::Data as CommandData<Self>>::Options::from_data_option(data) {
            Ok(options) => match <Self as SlashCommandData>::Data::from_options(options) {
                Ok(data) => SlashCommandData::run(self, state, interaction, data).await,
                Err(error) => Err(CommandParseErrorInfo {
                    guild: interaction.guild().expect("Will be source later"),
                    name: interaction.command_name,
                    id: interaction.command,
                    error,
                }.into())
            },
            Err(error) => Err(CommandParseErrorInfo {
                guild: interaction.guild().expect("Will be source later"),
                name: interaction.command_name,
                id: interaction.command,
                error,
            }.into())
        }
        // match <Self as SlashCommandData>::Data::from_data(data) {
        //     Ok(data) => SlashCommandData::run(self, state, interaction, data).await,
        //     Err(error) => Err(CommandParseErrorInfo {
        //         guild: interaction.guild().expect("I'll make this Source later :)"),
        //         name: interaction.command_name,
        //         id: interaction.command,
        //         error
        //     }.into())
        // }
        // let data = <<Self as SlashCommandData>::Data as CommandData<Self>>::Data::from_data_option(data);
        // match data {
        //     Ok(data) => {
        //         match <Self as SlashCommandData>::Data::from_data(data) {
        //             Ok(data) => SlashCommandData::run(self, state, interaction, data).await,
        //             // error in the actual contents of the data, though it was the right type
        //             Err(error) => Err(CommandParseErrorInfo {
        //                 guild: interaction.guild().expect("I'll do the Source thing later"),
        //                 name: interaction.command_name,
        //                 id: interaction.command,
        //                 error,
        //             }.into()),
        //         }
        //     }
        //     // error getting to right type of data (eg the InteractionDataOption wasn't a
        //     // CommandOption or whatever type it was supposed to be)
        //     Err(error) => Err(CommandParseErrorInfo {
        //         guild: interaction.guild().expect("I'll do this later"),
        //         name: interaction.command_name,
        //         id: interaction.command,
        //         error,
        //     }.into())
        // }
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
                 data: InteractionDataOption,
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