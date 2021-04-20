use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use downcast_rs::{Downcast, impl_downcast};
use dyn_clone::{clone_trait_object, DynClone};

use crate::BotState;
use crate::commands::FinalizeInteraction;
use crate::errors::{BotError, CommandParseErrorInfo};
use crate::http::ClientResult;
pub use crate::model::commands::*;
use crate::model::ids::{CommandId, GuildId};
pub use crate::model::interaction::*;
use crate::shard::dispatch::ReactionUpdate;

#[async_trait]
pub trait SlashCommand: Sized + Send + Sync + Debug + Downcast + DynClone + SlashCommandRaw<Bot=<Self as SlashCommand>::Bot> {
    type Bot: Send + Sync;
    type Data: CommandData<Self> + Send;
    type Use: NotUnused + Send;

    const NAME: &'static str;
    fn description(&self) -> Cow<'static, str>;
    fn default_permissions(&self) -> bool { true }

    fn options(&self) -> TopLevelOption {
        <Self::Data as CommandData<Self>>::VecArg::tlo_ctor()(Self::Data::make_args(self))
    }

    async fn run(&self,
                 state: Arc<BotState<<Self as SlashCommandRaw>::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<Self::Use>, BotError>;
}

#[allow(clippy::use_self)]
#[async_trait]
impl<Scd: SlashCommand> SlashCommandRaw for Scd
    where InteractionUse<<Self as SlashCommand>::Use>: FinalizeInteraction
{
    type Bot = <Self as SlashCommand>::Bot;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn command(&self) -> Command {
        Command::new(
            Self::NAME,
            self.description(),
            self.options(),
            self.default_permissions(),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: InteractionDataOption,
    ) -> Result<InteractionUse<Used>, BotError> {
        match <<Self as SlashCommand>::Data as CommandData<Self>>::Options::from_data_option(data) {
            Ok(options) => match <Self as SlashCommand>::Data::from_options(options) {
                Ok(data) => {
                    let self_use = SlashCommand::run(self, Arc::clone(&state), interaction, data).await?;
                    self_use.finalize(&state).await.map_err(|e| e.into())
                }
                Err(error) => {
                    let interaction = interaction.respond(
                        state,
                        ephemeral(format!("Error parsing command: ```rs\n{:?}```", error)),
                    ).await?;
                    Err(CommandParseErrorInfo {
                        name: interaction.command_name,
                        id: interaction.command,
                        source: interaction.source,
                        error,
                    }.into())
                }
            },
            Err(error) => {
                let interaction = interaction.respond(
                    state,
                    ephemeral(format!("Error parsing command: ```rs\n{:?}```", error)),
                ).await?;
                Err(CommandParseErrorInfo {
                    name: interaction.command_name,
                    id: interaction.command,
                    source: interaction.source,
                    error,
                }.into())
            }
        }
    }
}

#[async_trait]
pub trait SlashCommandRaw: Send + Sync + Debug + Downcast + DynClone {
    type Bot: Send + Sync;

    fn name(&self) -> &'static str;

    fn command(&self) -> Command;

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: InteractionDataOption,
    ) -> Result<InteractionUse<Used>, BotError>;
}
impl_downcast!(SlashCommandRaw assoc Bot);

impl<'clone, B> Clone for Box<dyn SlashCommandRaw<Bot=B> + 'clone> {
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

/// Extension methods for SlashCommands to create/edit/delete them
#[async_trait]
pub trait SlashCommandExt: SlashCommandRaw {
    /// Edit [command](command) by id, updating its description, options, and default_permissions.
    ///
    /// Note: the command's name is not edited
    async fn edit_command<State, B>(
        &mut self,
        state: State,
        guild: GuildId,
        command: CommandId,
    ) -> ClientResult<ApplicationCommand>
        where
            State: AsRef<BotState<B>> + Send,
            B: Send + Sync + 'static
    {
        let Command { description, options, default_permission, .. } = self.command();
        let state = state.as_ref();
        state.client.edit_guild_command(
            state.application_id().await,
            guild,
            command,
            None,
            Some(description.as_ref()),
            Some(options),
            Some(default_permission),
        ).await
    }
}

#[async_trait]
impl<C: SlashCommandRaw> SlashCommandExt for C {}