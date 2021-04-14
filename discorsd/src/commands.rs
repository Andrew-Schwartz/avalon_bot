use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use downcast_rs::{Downcast, impl_downcast};
use dyn_clone::{clone_trait_object, DynClone};

use crate::BotState;
use crate::commands::FinalizeInteraction;
use crate::errors::{BotError, CommandParseErrorInfo};
pub use crate::model::commands::*;
pub use crate::model::interaction::*;
use crate::shard::dispatch::ReactionUpdate;

#[async_trait]
pub trait SlashCommandData: Sized + Send + Sync + Debug + Downcast + DynClone + SlashCommand {
    type Bot: Send + Sync;
    type Data: CommandData<Self> + Send;
    type Use: NotUnused + Send;

    const NAME: &'static str;
    fn description(&self) -> Cow<'static, str>;
    fn usable_by_everyone(&self) -> bool { true }

    fn options(&self) -> TopLevelOption {
        <Self::Data as CommandData<Self>>::VecArg::tlo_ctor()(Self::Data::make_args(self))
    }

    async fn run(&self,
                 state: Arc<BotState<<Self as SlashCommand>::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<Self::Use>, BotError>;
}

#[allow(clippy::use_self)]
#[async_trait]
impl<Scd: SlashCommandData> SlashCommand for Scd
    where InteractionUse<<Self as SlashCommandData>::Use>: FinalizeInteraction
{
    type Bot = <Self as SlashCommandData>::Bot;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn command(&self) -> Command {
        Command::new(
            Self::NAME,
            self.description(),
            self.options(),
            self.usable_by_everyone(),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: InteractionDataOption,
    ) -> Result<InteractionUse<Used>, BotError> {
        match <<Self as SlashCommandData>::Data as CommandData<Self>>::Options::from_data_option(data) {
            Ok(options) => match <Self as SlashCommandData>::Data::from_options(options) {
                Ok(data) => {
                    let self_use = SlashCommandData::run(self, Arc::clone(&state), interaction, data).await?;
                    self_use.finalize(&state).await.map_err(|e| e.into())
                }
                Err(error) => Err(CommandParseErrorInfo {
                    name: interaction.command_name,
                    id: interaction.command,
                    source: interaction.source,
                    error,
                }.into())
            },
            Err(error) => Err(CommandParseErrorInfo {
                name: interaction.command_name,
                id: interaction.command,
                source: interaction.source,
                error,
            }.into())
        }
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