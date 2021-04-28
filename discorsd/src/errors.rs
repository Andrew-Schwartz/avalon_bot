//! Error handling for `discorsd`, mainly though the [`BotError`](BotError) enum.

use std::fmt::{self, Debug, Display};
use std::ops::Range;

use thiserror::Error;

use crate::BotState;
use crate::commands::{DmSource, GuildSource, SlashCommandRaw};
use crate::http::{ClientError, DisplayClientError};
use crate::model::ids::*;
use crate::model::interaction::{InteractionSource, OptionValue};

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
    NotVoting,
}

impl Display for AvalonError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::TooManyPlayers(n) => write!(f, "Too many players! {} is more than the maximum number of players (10).", n),
            Self::Stopped => f.write_str("Game Already Over"),
            Self::NotVoting => f.write_str("No longer in the voting phase"),
        }
    }
}

#[derive(Error, Debug)]
pub struct CommandParseErrorInfo {
    pub name: String,
    pub id: CommandId,
    pub source: InteractionSource,
    pub error: CommandParseError,
}

impl CommandParseErrorInfo {
    #[allow(clippy::missing_panics_doc)]
    pub async fn display_error<B: Send + Sync>(&self, state: &BotState<B>) -> String {
        let source = match &self.source {
            InteractionSource::Guild(GuildSource { id, member }) => if let Some(guild) = state.cache.guild(id).await {
                format!(
                    "guild `{}` ({}), used by `{}` ({})",
                    guild.name.as_deref().unwrap_or("null"), guild.id, member.nick_or_name(), member.id()
                )
            } else {
                format!(
                    "unknown guild `{}`, used by `{}` ({})",
                    id, member.nick_or_name(), member.id()
                )
            },
            InteractionSource::Dm(DmSource { user }) => format!(
                "dm with `{}#{}` ({})",
                user.username, user.discriminator, user.id
            ),
        };
        match &self.source {
            InteractionSource::Guild(GuildSource { id, .. }) => {
                let guard = state.commands.read().await;
                if let Some(guild_lock) = guard.get(id) {
                    let guard = guild_lock.read().await;
                    self.command_fail_message(&source, guard.get(&self.id).map(|c| &**c))
                } else {
                    format!(
                        "Failed to parse command `{}` in {}, which has no commands: {:?}",
                        self.id, source, self.error,
                    )
                }
            }
            InteractionSource::Dm(_) => {
                let global = state.global_commands.get().unwrap();
                self.command_fail_message(&source, global.get(&self.id).copied())
            }
        }
    }

    fn command_fail_message<B: Send + Sync>(&self, source: &str, command: Option<&dyn SlashCommandRaw<Bot=B>>) -> String {
        if let Some(command) = command {
            format!(
                "Failed to parse command `{}` ({}) in {}: {:?}",
                command.name(), self.id, source, self.error
            )
        } else {
            format!(
                "Failed to parse unknown command `{}` in {}: {:?}",
                self.id, source, self.error,
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
    /// found a vararg option when it was not expected
    UnexpectedVararg(String, usize),
    /// found a single option when expecting to find a vararg
    UnexpectedSingleOption(String, usize),
    MissingOption(String),
    /// Command named `String` didn't have a subcommand option
    NoSubtype(String),
    /// InteractionDataOption::Group(_) when parsing data for an struct
    BadGroupOccurrence,
    /// InteractionDataOption::Command(_) when parsing data for an struct
    BadCommandOccurrence,
    /// InteractionDataOption::Values(_) when parsing data for an enum
    BadValueOccurrence,
}

#[derive(Debug)]
pub struct OptionType {
    pub value: OptionValue,
    pub desired: CommandOptionTypeParsed,
}

/// like [`ApplicationCommandOptionType`](crate::commands::ApplicationCommandOptionType), but more
/// specifically for single option types and with more options (that have been further parsed, such
/// as unsigned ints or message id).
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum CommandOptionTypeParsed {
    String,
    I64,
    U64,
    Usize,
    Boolean,
    UserId,
    ChannelId,
    RoleId,
    MessageId,
    GuildId,
}

impl From<OptionType> for CommandParseError {
    fn from(ot: OptionType) -> Self {
        Self::BadType(ot)
    }
}

impl OptionValue {
    pub const fn parse_error(self, desired_type: CommandOptionTypeParsed) -> OptionType {
        OptionType { value: self, desired: desired_type }
    }
}

#[derive(Debug)]
pub struct UnknownOption {
    pub name: String,
    pub options: &'static [&'static str],
}