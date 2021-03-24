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

// todo is there any way to make SlashCommandData work for this? probably will have to make
//  StartData more complicated, perhaps the proc macro too
// #[async_trait]
// impl SlashCommand<Bot> for StartCommand {
//     fn name(&self) -> &'static str { "start" }
//
//     fn command(&self) -> Command {
//         let (description, options): (Cow<'static, str>, _) = match self.0.iter().exactly_one() {
//             Ok(game) => (format!("Starts {} immediately in this channel", game).into(), TopLevelOption::Empty),
//             Err(_) => ("Choose a game to start in this channel".into(), StartData::args::<Self, _>(&self)),
//         };
//         Command::new(self.name(), description, options)
//     }
//
//     async fn run(&self,
//                  state: Arc<BotState<Bot>>,
//                  interaction: InteractionUse<Unused>,
//                  data: ApplicationCommandInteractionData,
//     ) -> Result<InteractionUse<Used>, BotError> {
//         let game = match self.0.iter().exactly_one() {
//             Ok(game) => *game,
//             Err(_) => StartData::from_data(data, interaction.guild().unwrap())?.game.unwrap(),
//         };
//         let used = interaction.defer(&state).await?;
//
//         match game {
//             GameType::Avalon => avalon::start::start(state, &used).await?,
//             GameType::Hangman => hangman::start::start(state, &used).await?,
//             GameType::Kittens => todo!(),
//         }
//
//         Ok(used)
//     }
// }

#[async_trait]
impl SlashCommandData for StartCommand {
    type Bot = Bot;
    type Data = StartData;
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
                 data: StartData
    ) -> Result<InteractionUse<Used>, BotError> {
        let game = data.game;
        let used = interaction.defer(&state).await?;

        match game.unwrap_or_else(|| *self.0.iter().next().unwrap()) {
            GameType::Avalon => avalon::start::start(state, &used).await?,
            GameType::Hangman => hangman::start::start(state, &used).await?,
            GameType::Kittens => todo!(),
        }

        Ok(used)
    }
}

#[derive(CommandData)]
#[command(type = "StartCommand")]
pub struct StartData {
    #[command(choices, desc = "Choose the game to start", required = "req", retain = "retain")]
    game: Option<GameType>,
}

fn req(command: &StartCommand) -> bool {
    command.0.len() > 1
}

fn retain(command: &StartCommand, choice: &CommandChoice<&'static str>) -> bool {
    command.0.iter().any(|game| game == choice.value)
}