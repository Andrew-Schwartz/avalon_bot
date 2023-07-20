use command_data_derive::CommandDataChoices;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, CommandDataChoices)]
pub enum GameType {
    #[command(default)]
    Avalon,
    Coup,
    Hangman,
    #[command(choice = "Exploding Kittens")]
    Kittens,
}

impl GameType {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Avalon => "Avalon",
            Self::Coup => "Coup",
            Self::Hangman => "Hangman",
            Self::Kittens => "Exploding Kittens",
        }
    }
}