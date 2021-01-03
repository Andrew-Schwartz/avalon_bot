use thiserror::Error;
use crate::http::ClientError;
use crate::http::model::{OptionValue, ApplicationCommandOptionType};

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

#[derive(Error, Debug)]
pub enum GameError {
    #[error("Error in Avalon: {0}")]
    Avalon(#[from] AvalonError)
}

#[derive(Error, Debug)]
pub enum AvalonError {
    #[error("Too many players! {0} is more than the maximum number of players (10).")]
    TooManyPlayers(usize),
}

#[derive(Error, Debug)]
pub enum CommandParseError {
    #[error("Error parsing command option {0:?}")]
    Option(OptionParseError),
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