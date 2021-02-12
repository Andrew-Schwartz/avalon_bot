use command_data_derive::CommandDataOption;
use std::fmt::{Display, self};

// this will be somewhere else lol
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, CommandDataOption)]
pub enum GameType {
    #[command(default)]
    Avalon,
    Hangman,
    #[command(choice = "Exploding Kittens")]
    Kittens,
}

impl GameType {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Avalon => "Avalon",
            Self::Hangman => "Hangman",
            Self::Kittens => "Exploding Kittens",
        }
    }
}

impl Display for GameType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}