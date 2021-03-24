use std::fmt::{self, Debug, Display};
use std::ops::Range;

use thiserror::Error;

use crate::BotState;
use crate::http::{ClientError, DisplayClientError};
use crate::model::ids::*;
use crate::model::interaction::{ApplicationCommandOptionType, OptionValue};

#[derive(Error, Debug)]
pub enum BotError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Game(#[from] GameError),
    #[error(transparent)]
    CommandParse(#[from] CommandParseErrorInfo),
    #[error("Error converting `chrono::time::Duration` to `std::time::Duration`")]
    Chrono,
}

impl BotError {
    pub async fn display_error<B: Send + Sync>(&self, state: &BotState<B>) -> DisplayBotError<'_> {
        match self {
            Self::Client(e) => DisplayBotError::Client(e.display_error(state).await),
            Self::Game(e) => DisplayBotError::Game(e),
            Self::CommandParse(e) => DisplayBotError::CommandParse(e.display_error(state).await),
            Self::Chrono => DisplayBotError::Chrono,
        }
    }
}

pub enum DisplayBotError<'a> {
    Client(DisplayClientError<'a>),
    Game(&'a GameError),
    CommandParse(String),
    Chrono,
}

impl Display for DisplayBotError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Client(e) => write!(f, "{}", e),
            Self::Game(e) => write!(f, "{}", e),
            Self::CommandParse(e) => f.write_str(e),
            Self::Chrono => f.write_str("Error converting `chrono::time::Duration` to `std::time::Duration`"),
        }
    }
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
    Avalon(#[from] AvalonError)
}

impl Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Avalon(e) => write!(f, "{}", e),
        }
    }
}

bot_error_from!(AvalonError, GameError);

#[derive(Error, Debug)]
pub enum AvalonError {
    TooManyPlayers(usize),
    Stopped,
}

impl Display for AvalonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::TooManyPlayers(n) => write!(f, "Too many players! {} is more than the maximum number of players (10).", n),
            Self::Stopped => f.write_str("Game Already Over"),
        }
    }
}

#[derive(Error, Debug)]
pub struct CommandParseErrorInfo {
    pub id: CommandId,
    pub guild: GuildId,
    pub error: CommandParseError,
}

impl CommandParseErrorInfo {
    pub async fn display_error<B: Send + Sync>(&self, state: &BotState<B>) -> String {
        let guild = if let Some(guild) = state.cache.guild(self.guild).await {
            format!("guild `{}` ({})", guild.name.as_deref().unwrap_or("null"), self.guild)
        } else {
            format!("unknown guild `{}`", self.guild)
        };
        let guard = state.commands.read().await;
        if let Some(guild_lock) = guard.get(&self.guild) {
            let guard = guild_lock.read().await;
            if let Some(command) = guard.get(&self.id).cloned() {
                format!(
                    "Failed to parse command `{}` ({}) in {}: {:?}",
                    command.name(), self.id, guild, self.error
                )
            } else {
                format!(
                    "Failed to parse unknown command `{}` in {}: {:?}",
                    self.id, guild, self.error,
                )
            }
        } else {
            format!(
                "Failed to parse command `{}` in {}, which has no commands: {:?}",
                self.id, guild, self.error,
            )
        }
    }
}

impl Display for CommandParseErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub enum CommandParseError {
    BadType(OptionType),
    UnknownOption(UnknownOption),
    EmptyOption(String),
    BadOrder(String, usize, Range<usize>),
    MissingOption(String),
    /// Command named `String` didn't have a subcommand option
    NoSubtype(String),
}

pub trait CommandParseErrorWithInfo {
    fn with_info(self, guild: GuildId) -> CommandParseErrorInfo;
}

impl CommandParseErrorWithInfo for (CommandParseError, CommandId) {
    fn with_info(self, guild: GuildId) -> CommandParseErrorInfo {
        CommandParseErrorInfo { id: self.1, guild, error: self.0 }
    }
}

#[derive(Debug)]
pub struct OptionType {
    pub value: OptionValue,
    pub desired: ApplicationCommandOptionType,
}

impl From<OptionType> for CommandParseError {
    fn from(ot: OptionType) -> Self {
        Self::BadType(ot)
    }
}

impl OptionValue {
    pub const fn parse_error(self, desired_type: ApplicationCommandOptionType) -> OptionType {
        OptionType { value: self, desired: desired_type }
    }
}

#[derive(Debug)]
pub struct UnknownOption {
    pub name: String,
    pub options: &'static [&'static str],
}