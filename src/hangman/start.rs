use std::sync::Arc;

use discorsd::BotState;
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::MessageChannelExt;

use crate::Bot;
use crate::hangman::guess::GuessCommand;
use crate::hangman::hangman_command::HangmanCommand;

pub async fn start(state: &Arc<BotState<Bot>>, interaction: &InteractionUse<SlashCommandData, Deferred>) -> Result<(), BotError> {
    let guild = interaction.guild().unwrap();
    let mut guard = state.bot.hangman_games.write().await;
    // todo when started with StartCommand this doesn't exist
    // probably want to call AddMe::hangman first (maybe just in SC)
    let hangman = guard.get_mut(&guild).unwrap();
    let ghw_guard = state.bot.guild_hist_words.write().await;
    let game = hangman.start(interaction.channel, ghw_guard).await;

    let message = game.channel.send(&state, game.embed()).await?;
    message.react(&state, '❓').await?;
    let message_id = message.id;
    game.message = Some(message);

    // remove hangman command (start command too?)
    {
        // state.disable_command::<HangmanCommand>(guild).await?;
        // todo edit start command
    }

    // set up reaction commands
    {
        let mut commands = state.reaction_commands.write().await;
        commands.push(Box::new(GuessCommand(guild, game.players.clone(), message_id)));
    }

    Ok(())
}