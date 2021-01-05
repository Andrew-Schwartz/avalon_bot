use std::fmt::{self, Debug, Display};

use thiserror::Error;

use crate::http::ClientError;
use crate::http::model::{ApplicationCommandOptionType, CommandId, GuildId, OptionValue};

#[derive(Error, Debug)]
pub enum BotError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Game(#[from] GameError),
    #[error(transparent)]
    CommandParse(#[from] CommandParseError),
    #[error("Error converting `chrono::time::Duration` to `std::time::Duration`")]
    Chrono,
}

// since GameError is an enum, want to be able to Into its variants into BotError (maybe others too)
macro_rules! bot_error_from {
    ($e2:ty, $e1:ty) => {
        impl From<$e2> for BotError {
            fn from(e2: $e2) -> Self {
                let e1: $e1 = e2.into();
                e1.into()
            }
        }
    };
}

#[derive(Error, Debug)]
pub enum GameError {
    #[error("Error in Avalon: {0}")]
    Avalon(#[from] AvalonError)
}

bot_error_from!(AvalonError, GameError);

#[derive(Error, Debug)]
pub enum AvalonError {
    #[error("Too many players! {0} is more than the maximum number of players (10).")]
    TooManyPlayers(usize),
    #[error("Game Already Over")]
    Stopped,
}

#[derive(Error, Debug)]
pub struct CommandParseError {
    pub id: CommandId,
    pub guild: GuildId,
    pub kind: CommandParseErrorKind,
}

impl Display for CommandParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// #[derive(Error)]
pub enum CommandParseErrorKind {
    // #[error("Error parsing command option {0:?}")]
    Option(OptionParseError),
}

impl Debug for CommandParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Option(ope) => write!(f, "{:?}", ope)
        }
    }
}

#[derive(Debug)]
pub struct OptionParseError {
    pub value: OptionValue,
    pub desired: ApplicationCommandOptionType,
}

impl OptionValue {
    pub fn parse_error(self, desired_type: ApplicationCommandOptionType) -> OptionParseError {
        OptionParseError { value: self, desired: desired_type }
    }
}