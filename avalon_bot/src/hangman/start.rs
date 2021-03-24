use std::sync::Arc;

use discorsd::BotState;
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::ChannelExt;

use crate::{Bot, delete_command};
use crate::hangman::guess::GuessCommand;
use crate::hangman::hangman_command::HangmanCommand;

pub async fn start(state: Arc<BotState<Bot>>, used: &InteractionUse<Used>) -> Result<(), BotError> {
    let guild = used.guild().unwrap();
    let mut guard = state.bot.hangman_games.write().await;
    // todo when started with StartCommand this doesn't exist
    // probably want to call AddMe::hangman first (maybe just in SC)
    let hangman = guard.get_mut(&guild).unwrap();
    let ghw_guard = state.bot.guild_hist_words.write().await;
    let game = hangman.start(used.channel, ghw_guard).await;

    let message = game.channel.send(&state.client, game.embed()).await?;
    message.react(&state, '❓').await?;
    let message_id = message.id;
    game.message = Some(message);

    // remove hangman command (start command too?)
    {
        let guild = guild;
        let commands = state.commands.read().await;
        if let Some(commands) = commands.get(&guild) {
            let mut commands = commands.write().await;
            delete_command(&state, guild, &mut commands, |c| c.is::<HangmanCommand>()).await?;
        }
    }

    // set up reaction commands
    {
        let mut commands = state.reaction_commands.write().await;
        commands.push(Box::new(GuessCommand(guild, game.players.clone(), message_id)));
    }

    Ok(())
}