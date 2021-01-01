use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use downcast_rs::{Downcast, impl_downcast};
use dyn_clone::{clone_trait_object, DynClone};
use futures_util::StreamExt;
use once_cell::sync::*;
use serde::export::fmt::Debug;
use serde::export::PhantomData;
use strum::{AsStaticRef, AsStaticStr};

pub use addme::*;
use discorsd::{
    anyhow,
    BotState,
    http::model::*,
};
use discorsd::async_trait;
use discorsd::http::{ClientError, DiscordClient};
use discorsd::shard::dispatch::ReactionUpdate;
pub use info::*;

use crate::Bot;
use crate::commands::ping::PING_COMMAND;
use crate::commands::uptime::UPTIME_COMMAND;

pub mod info;
pub mod addme;
pub mod stop;
pub mod ping;
pub mod uptime;

#[derive(Debug, PartialEq/*, Copy, Clone*/)]
pub enum NotUsed {}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Used {}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionUse<Usability> {
    /// id of the interaction
    pub id: InteractionId,
    /// the id of the command being invoked
    pub command: CommandId,
    /// the guild it was sent from
    pub guild: GuildId,
    /// the channel it was sent from
    pub channel: ChannelId,
    /// guild member data for the invoking user
    pub member: GuildMember,
    /// a continuation token for responding to the interaction
    pub token: String,
    _priv: PhantomData<Usability>,
}

impl InteractionUse<NotUsed> {
    pub fn from(interaction: Interaction) -> (Self, ApplicationCommandInteractionData) {
        let Interaction { id, kind: _kind, data, guild_id, channel_id, member, token } = interaction;
        let this = Self { id, command: data.id, guild: guild_id, channel: channel_id, member, token, _priv: PhantomData };
        (this, data)
    }

    pub async fn respond<Client: AsRef<DiscordClient>>(self, client: &Client, response: InteractionResponse) -> Result<InteractionUse<Used>, ClientError> {
        client.as_ref().create_interaction_response(
            self.id,
            &self.token,
            response,
        ).await.map(|_| self.into())
    }

    pub async fn ack<Client: AsRef<DiscordClient>>(self, client: Client) -> Result<InteractionUse<Used>, ClientError> {
        client.as_ref().create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::Acknowledge,
        ).await.map(|_| self.into())
    }

    pub async fn ack_source<Client: AsRef<DiscordClient>>(self, client: &Client) -> Result<InteractionUse<Used>, ClientError> {
        client.as_ref().create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::AckWithSource,
        ).await.map(|_| self.into())
    }
}

impl InteractionUse<Used> {
    pub async fn edit<State: AsRef<BotState<Bot>>>(&mut self, state: State, message: InteractionMessage) -> Result<(), ClientError> {
        let state = state.as_ref();
        state.client.edit_interaction_response(
            state.application_id().await,
            &self.token,
            message,
        ).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete<State: AsRef<BotState<Bot>>>(self, state: State) -> Result<(), ClientError> {
        let state = state.as_ref();
        state.client.delete_interaction_response(
            state.application_id().await,
            &self.token,
        ).await
    }
}

impl From<InteractionUse<NotUsed>> for InteractionUse<Used> {
    fn from(InteractionUse { id, command: command_id, guild: guild_id, channel: channel_id, member, token, _priv }: InteractionUse<NotUsed>) -> Self {
        Self { id, command: command_id, guild: guild_id, channel: channel_id, member, token, _priv: PhantomData }
    }
}

pub(crate) async fn init_global_commands<State: AsRef<BotState<Bot>>>(state: State) {
    if GLOBAL_IDS.get().is_none() {
        let state = state.as_ref();
        let app = state.application_id().await;
        // let guild = state.bot.guild_id;
        let ids = tokio::stream::iter(&GLOBAL_COMMANDS)
            .then(|command| async move {
                let resp = state.client
                    .create_global_command(app, /*guild,*/ command.command())
                    .await
                    .unwrap_or_else(|_| panic!("when creating `{}`", command.name()));
                (resp.id, *command)
            })
            .collect()
            .await;
        let _ = GLOBAL_IDS.set(ids);
    }
}

pub(crate) async fn create_guild_commands<State>(
    state: State,
    guild: GuildId,
    commands: Vec<Box<dyn SlashCommand>>,
) -> HashMap<CommandId, Box<dyn SlashCommand>>
    where State: AsRef<BotState<Bot>>
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

pub(crate) async fn run_global_commands(
    interaction: Interaction,
    state: Arc<BotState<Bot>>,
) -> anyhow::Result<Result<(), Interaction>> {
    if let Some(command) = GLOBAL_IDS.get().unwrap().get(&interaction.data.id) {
        let (interaction, data) = InteractionUse::from(interaction);
        command.run(state, interaction, data).await?;
        Ok(Ok(()))
    } else {
        Ok(Err(interaction))
    }
}

static GLOBAL_IDS: OnceCell<HashMap<CommandId, &'static dyn SlashCommand>> = OnceCell::new();

const GLOBAL_COMMANDS: [&'static dyn SlashCommand; 3] = [&INFO_COMMAND, &PING_COMMAND, &UPTIME_COMMAND];

#[async_trait]
pub trait SlashCommand: Send + Sync + Debug + Downcast + DynClone {
    fn name(&self) -> &'static str;

    fn command(&self) -> Command;

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 data: ApplicationCommandInteractionData,
    ) -> anyhow::Result<InteractionUse<Used>>;
}
impl_downcast!(SlashCommand);
clone_trait_object!(SlashCommand);

pub trait SlashCommandExt: SlashCommand {
    fn make<D: Into<Cow<'static, str>>>(&self, description: D, options: TopLevelOption) -> Command {
        Command::new(self.name(), description, options)
    }
}

impl<S: SlashCommand> SlashCommandExt for S {}

#[async_trait]
pub trait ReactionCommand: Send + Sync + Debug + Downcast + DynClone {
    fn applies(&self, reaction: &ReactionUpdate) -> bool;

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 reaction: ReactionUpdate,
    ) -> anyhow::Result<()>;
}
impl_downcast!(ReactionCommand);
clone_trait_object!(ReactionCommand);

// this will be somewhere else lol
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, AsStaticStr)]
pub enum GameType {
    Avalon,
    Hangman,
    Kittens,
}

impl GameType {
    pub fn name(&self) -> &'static str { self.as_static() }
}

impl From<ApplicationCommandInteractionData> for GameType {
    fn from(mut data: ApplicationCommandInteractionData) -> Self {
        use GameType::*;
        if data.options.is_empty() {
            Avalon
        } else {
            let game = data.options.remove(0).value.unwrap().unwrap_string();
            match game.as_str() {
                "Avalon" => Avalon,
                "Hangman" => Hangman,
                "Kittens" => Kittens,
                _ => unreachable!(),
            }
        }
    }
}