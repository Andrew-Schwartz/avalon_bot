use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use itertools::Itertools;

use command_data_derive::CommandData;
use discorsd::{BotState, http::ClientResult};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::model::ids::*;
use discorsd::model::message::Color;
use discorsd::shard::dispatch::{ReactionType, ReactionUpdate};

use crate::{async_trait, Bot};
use crate::avalon::AvalonPlayer;
use crate::games::GameType;
use crate::utils::IterExt;

#[derive(Debug, Clone, Default)]
pub struct StopCommand {
    pub games: HashSet<GameType>,
    pub default_permissions: bool,
}

impl StopCommand {
    pub fn insert(&mut self, game: GameType) -> Option<GameType> {
        self.default_permissions = true;
        self.games.replace(game)
    }

    pub fn remove(&mut self, game: GameType) -> Option<GameType> {
        let removed = self.games.remove(&game);
        if self.games.is_empty() {
            self.default_permissions = false;
        }
        removed.then(|| game)
    }
}

impl StopCommand {
    pub const CONFIRM: char = '✅';
    pub const CANCEL: char = '❌';
}

#[derive(CommandData)]
#[command(command = "StopCommand")]
pub struct StopData {
    #[command(desc = "The game to stop", required = "req", retain = "retain")]
    game: Option<GameType>,
}

fn req(command: &StopCommand) -> bool {
    command.games.len() > 1
}

fn retain(command: &StopCommand, choice: &CommandChoice<&'static str>) -> bool {
    command.games.iter().any(|game| game == choice.value)
}

#[async_trait]
impl SlashCommand for StopCommand {
    type Bot = Bot;
    type Data = StopData;
    type Use = Used;
    const NAME: &'static str = "stop";

    fn description(&self) -> Cow<'static, str> {
        format!(
            "Stop the current game{} in this server. Requires 2 additional players to confirm.",
            if self.games.is_empty() {
                "".to_string()
            } else {
                format!(
                    " of {}",
                    match self.games.iter().exactly_one() {
                        Ok(game) => game.to_string(),
                        Err(games) => games.into_iter()
                            .list_grammatically(GameType::to_string, "or"),
                    }
                )
            }
        ).into()
    }

    fn default_permissions(&self) -> bool {
        self.default_permissions
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: StopData,
    ) -> Result<InteractionUse<Self::Use>, BotError> {
        let game = data.game.unwrap_or_else(|| *self.games.iter().exactly_one().unwrap());

        match game {
            GameType::Avalon => {
                let guild = interaction.guild().unwrap();
                let message = format!(
                    "React {} to confirm stopping the game or {} to cancel this.\n\
                     Note: 2 other players must confirm for the game to be stopped.",
                    Self::CONFIRM, Self::CANCEL
                );
                let interaction = interaction.respond(&state, message).await?;
                let message = interaction.get_message(
                    &state.cache,
                    Duration::from_millis(5),
                    Duration::from_secs(2),
                ).await.unwrap();
                let id = message.id;
                let s = Arc::clone(&state);
                tokio::spawn(async move {
                    message.react(&s, Self::CONFIRM).await?;
                    message.react(&s, Self::CANCEL).await?;
                    ClientResult::Ok(())
                });
                {
                    let players = state.bot.avalon_games.read().await
                        .get(&guild)
                        .unwrap()
                        .game_ref()
                        .players.iter()
                        .map(AvalonPlayer::id)
                        .collect();
                    let mut reaction_commands = state.reaction_commands.write().await;
                    let vote = StopVoteCommand(id, guild, players, interaction.user().id, GameType::Avalon);
                    reaction_commands.push(Box::new(vote));
                }
                {
                    let guard = state.commands.read().await;
                    let mut commands = guard.get(&guild).unwrap()
                        .write().await;
                    let this_cmd = commands.get_mut(&interaction.command)
                        .unwrap()
                        .downcast_mut::<Self>()
                        .unwrap();
                    this_cmd.remove(GameType::Avalon);
                    this_cmd.edit_command(&state, guild, interaction.command).await?;
                }
                Ok(interaction)
            }
            GameType::Hangman => todo!(),
            GameType::Kittens => todo!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StopVoteCommand(MessageId, pub GuildId, Vec<UserId>, UserId, GameType);

#[allow(clippy::use_self)]
#[async_trait]
impl ReactionCommand<Bot> for StopVoteCommand {
    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        reaction.message_id == self.0 &&
            self.2.contains(&reaction.user_id)
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<(), BotError> {
        let mut guard = state.bot.avalon_games.write().await;
        let avalon = guard.get_mut(&self.1).unwrap();
        let game = avalon.game_mut();
        let (confirms, cancels) = &mut game.stop_votes;
        match reaction.emoji.as_unicode().and_then(|s| s.chars().next()) {
            Some(StopCommand::CONFIRM) => {
                *confirms += match reaction.kind {
                    ReactionType::Add => 1,
                    ReactionType::Remove => -1,
                }
            }
            Some(StopCommand::CANCEL) => {
                *cancels += match reaction.kind {
                    ReactionType::Add => if reaction.user_id == self.3 {
                        // the person who ran `/stop` can cancel it by themself
                        2
                    } else { 1 },
                    ReactionType::Remove => -1,
                }
            }
            _ => {}
        }
        if *confirms >= 2 {
            let guard = state.commands.read().await;
            let commands = guard.get(&self.1).unwrap()
                .write().await;
            // state.client.delete_message(reaction.channel_id, self.0).await?;
            avalon.game_over(&state, self.1, commands, embed(|e| {
                e.title("Manually ended");
                e.color(Color::GOLD);
            })).await?;
        } else if *cancels >= 2 {
            state.client.delete_message(reaction.channel_id, self.0).await?;
            game.stop_votes = (0, 0);
            {
                let mut reaction_commands = state.reaction_commands.write().await;
                reaction_commands.retain(|rc|
                    !matches!(rc.downcast_ref::<StopVoteCommand>(), Some(also_self) if also_self == self)
                );
            }
            {
                let commands = state.commands.read().await;
                let mut commands = commands.get(&self.1).unwrap().write().await;
                let (stop_id, stop_command) = state.get_command_mut::<StopCommand>(self.1, &mut commands).await;
                stop_command.insert(self.4);
                stop_command.edit_command(&state, self.1, stop_id).await?;
            }
        }

        Ok(())
    }
}