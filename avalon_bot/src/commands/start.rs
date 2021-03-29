use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use itertools::Itertools;

use command_data_derive::CommandData;
use discorsd::async_trait;
use discorsd::BotState;
use discorsd::commands::*;
use discorsd::errors::BotError;

use crate::{avalon, Bot, hangman};
use crate::games::GameType;

// todo add nice method to make adding/removing a GameType to this
#[derive(Clone, Debug)]
pub struct StartCommand(pub HashSet<GameType>);

#[async_trait]
impl SlashCommandData for StartCommand {
    type Bot = Bot;
    type Data = StartData;
    type Use = Deferred;
    const NAME: &'static str = "start";

    fn description(&self) -> Cow<'static, str> {
        match self.0.iter().exactly_one() {
            Ok(game) => format!("Starts {} in this channel", game).into(),
            Err(_) => "Choose a game to start in this channel".into()
        }
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: StartData,
    ) -> Result<InteractionUse<Self::Use>, BotError> {
        let game = data.game;
        let deferred = interaction.defer(&state).await?;

        match game.unwrap_or_else(|| *self.0.iter().next().unwrap()) {
            GameType::Avalon => avalon::start::start(&state, &deferred).await?,
            GameType::Hangman => hangman::start::start(&state, &deferred).await?,
            GameType::Kittens => todo!(),
        }

        Ok(deferred)
    }
}

#[derive(CommandData)]
#[command(command = "StartCommand")]
pub struct StartData {
    #[command(desc = "Choose the game to start", required = "req", retain = "retain")]
    game: Option<GameType>,
}

fn req(command: &StartCommand) -> bool {
    command.0.len() > 1
}

fn retain(command: &StartCommand, choice: &CommandChoice<&'static str>) -> bool {
    command.0.iter().any(|game| game == choice.value)
}