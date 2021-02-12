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

pub async fn create_guild_commands<B, State>(
    state: State,
    guild: GuildId,
    commands: Vec<Box<dyn SlashCommand<B>>>,
) -> HashMap<CommandId, Box<dyn SlashCommand<B>>>
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
pub trait SlashCommand<B>: Send + Sync + Debug + Downcast + DynClone {
    fn name(&self) -> &'static str;

    fn command(&self) -> Command;

    async fn run(&self,
                 state: Arc<BotState<B>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError>;
}
impl_downcast!(SlashCommand<B>);
clone_trait_object!(<B> SlashCommand<B>);

pub trait SlashCommandExt<B>: SlashCommand<B> {
    fn make<D: Into<Cow<'static, str>>>(&self, description: D, options: TopLevelOption) -> Command {
        Command::new(self.name(), description, options)
    }
}

impl<B, S: SlashCommand<B>> SlashCommandExt<B> for S {}

#[async_trait]
pub trait ReactionCommand<B>: Send + Sync + Debug + Downcast + DynClone {
    fn applies(&self, reaction: &ReactionUpdate) -> bool;

    async fn run(&self,
                 state: Arc<BotState<B>>,
                 reaction: ReactionUpdate,
    ) -> Result<(), BotError>;
}
impl_downcast!(ReactionCommand<B>);
clone_trait_object!(<B> ReactionCommand<B>);