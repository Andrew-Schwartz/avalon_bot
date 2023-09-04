use std::fmt;
use std::fmt::Display;
use discorsd::bot_error_from;
use discorsd::errors::BotError;
use discorsd::model::ids::{ChannelId, GuildId};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GameError {
    Avalon(#[from] AvalonError),
    Hangman(#[from] HangmanError),
}

impl From<GameError> for BotError<GameError> {
    fn from(e: GameError) -> Self {
        BotError::Custom(e)
    }
}

impl Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Avalon(e) => write!(f, "{e}"),
            Self::Hangman(e) => write!(f, "{e}"),
        }
    }
}

bot_error_from!(AvalonError => E = GameError);
bot_error_from!(HangmanError => E = GameError);

#[derive(Error, Debug)]
pub enum AvalonError {
    TooManyPlayers(usize),
    Stopped,
    NotVoting,
}

impl Display for AvalonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::TooManyPlayers(n) => write!(f, "Too many players! {n} is more than the maximum number of players (10)."),
            Self::Stopped => f.write_str("Game Already Over"),
            Self::NotVoting => f.write_str("No longer in the voting phase"),
        }
    }
}

#[derive(Error, Debug)]
pub enum HangmanError {
    NoWords(ChannelId, Option<GuildId>),
}

impl Display for HangmanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoWords(c, Some(g)) => write!(f, "No suitable words found in https://discord.com/channels/{g}/{c}"),
            Self::NoWords(c, None) => write!(f, "No suitable words found in https://discord.com/channels/@me/{c}"),
        }
    }
}