use std::fmt::{self, Display};

use command_data_derive::CommandDataChoices;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, CommandDataChoices)]
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