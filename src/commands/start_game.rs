use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::{AppCommandData, InteractionUse, SlashCommand, Unused, Used};
use discorsd::errors::BotError;
use discorsd::model::ids::GuildId;

use crate::{Bot, coup, hangman};
use crate::coup::StartingCoins;
use crate::error::GameError;
use crate::hangman::Source;

#[derive(CommandData)]
pub enum StartGame {
    Coup {
        #[command(default, desc = "How many coins each player starts with (defaults to 2)")]
        starting_coins: StartingCoins,
    },
    Avalon,
    Hangman {
        #[command(default, desc = "Choose where to get the random word from")]
        word_source: Source,
    },
}

#[derive(Clone, Debug)]
pub struct StartGameCommand(pub GuildId);

#[async_trait]
impl SlashCommand for StartGameCommand {
    type Bot = Bot;
    type Data = StartGame;
    type Use = Used;
    const NAME: &'static str = "start";

    fn description(&self) -> Cow<'static, str> {
        "Start a game!".into()
    }

    async fn run(
        &self,
        state: Arc<BotState<Bot>>,
        interaction: InteractionUse<AppCommandData, Unused>,
        data: Self::Data,
    ) -> Result<InteractionUse<AppCommandData, Self::Use>, BotError<GameError>> {
        match data {
            StartGame::Coup { starting_coins } => coup::start_setup(&state, starting_coins, self.0, interaction).await,
            StartGame::Avalon => todo!(),
            // StartGame::Avalon => avalon2::start_setup(),
            // StartGame::Hangman => todo!("Start Hangman"),
            // StartGame::Kittens => todo!("Start Kittens"),
            StartGame::Hangman { word_source } => hangman::start(&state, word_source, interaction).await
        }
    }
}