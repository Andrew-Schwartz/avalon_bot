use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use discorsd::{BotState, http::ClientResult};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::model::ids::*;
use discorsd::model::message::Color;
use discorsd::shard::dispatch::{ReactionType, ReactionUpdate};

use crate::{async_trait, Bot, create_command};
use crate::avalon::AvalonPlayer;
use crate::games::GameType;

#[derive(Debug, Copy, Clone)]
pub struct StopCommand(pub GameType);

impl StopCommand {
    pub const CONFIRM: char = '✅';
    pub const CANCEL: char = '❌';
}

#[async_trait]
impl SlashCommandData for StopCommand {
    type Bot = Bot;
    type Data = ();
    type Use = Used;
    const NAME: &'static str = "stop";

    fn description(&self) -> Cow<'static, str> {
        format!(
            "Stop the current game of {}. Requires 2 additional players to confirm.",
            self.0.name()
        ).into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 _data: (),
    ) -> Result<InteractionUse<Self::Use>, BotError> {
        let guild = interaction.guild().unwrap();
        let message = format!(
            "React {} to confirm stopping the game or {} to cancel this.\
                \nNote: 2 other players must confirm for the game to be stopped.",
            Self::CONFIRM, Self::CANCEL
        );
        let interaction = interaction.respond(&state, message).await?;
        let start = Instant::now();
        // todo this works pretty lit (and fast), make a dedicated fn to do this somewhere
        let message = loop {
            tokio::time::delay_for(Duration::from_millis(5)).await;
            if let Some(message) = state.cache.interaction_response(&interaction).await {
                println!("DONE: {:?}", start.elapsed());
                break message
            }
            println!("start.elapsed() = {:?}", start.elapsed());
        };
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
                .unwrap().game_ref()
                .players.iter()
                .map(AvalonPlayer::id)
                .collect();
            let mut reaction_commands = state.reaction_commands.write().await;
            let vote = StopVoteCommand(id, guild, players, interaction.user().id, self.0);
            reaction_commands.push(Box::new(vote));
        }
        {
            let guard = state.commands.read().await;
            let mut commands = guard.get(&guild).unwrap()
                .write().await;
            state.client.delete_guild_command(
                state.application_id().await,
                guild,
                interaction.command,
            ).await?;
            commands.remove(&interaction.command);
        }
        Ok(interaction)
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
                let mut commands = commands.get(&self.1).unwrap()
                    .write().await;
                create_command(&*state, self.1, &mut commands, StopCommand(self.4)).await?;
            }
        }

        Ok(())
    }
}