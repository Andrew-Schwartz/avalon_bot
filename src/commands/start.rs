use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use itertools::Itertools;

use command_data_derive::CommandData;
use discorsd::async_trait;
use discorsd::BotState;
use discorsd::commands::*;
use discorsd::errors::BotError;

use crate::{avalon, Bot};
use crate::commands::stop::StopCommand;
use crate::games::GameType;

#[derive(Clone, Debug)]
pub struct StartCommand {
    games: HashSet<GameType>,
    default_permissions: bool,
}

impl Default for StartCommand {
    fn default() -> Self {
        Self { games: set!(GameType::Hangman), default_permissions: true }
    }
}

impl StartCommand {
    pub fn insert(&mut self, game: GameType) -> Option<GameType> {
        self.default_permissions = true;
        self.games.replace(game)
    }

    pub fn remove(&mut self, game: GameType) -> Option<GameType> {
        let removed = self.games.remove(&game);
        if self.games.is_empty() {
            self.default_permissions = false;
        }
        removed.then_some(game)
    }
}

#[derive(CommandData)]
#[command(command = "StartCommand")]
pub struct StartData {
    #[command(desc = "Choose the game to start", required = "req", retain = "retain")]
    game: Option<GameType>,
}

fn req(command: &StartCommand) -> bool {
    command.games.len() > 1
}

fn retain(command: &StartCommand, choice: GameType) -> bool {
    command.games.contains(&choice)
}

#[async_trait]
impl SlashCommand for StartCommand {
    type Bot = Bot;
    type Data = StartData;
    type Use = Deferred;
    const NAME: &'static str = "start";

    fn description(&self) -> Cow<'static, str> {
        match self.games.iter().exactly_one() {
            Ok(game) => format!("Starts {game} in this channel").into(),
            Err(_) => "Choose a game to start in this channel".into()
        }
    }

    fn default_permissions(&self) -> bool {
        self.default_permissions
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<AppCommandData, Unused>,
                 data: StartData,
    ) -> Result<InteractionUse<AppCommandData, Self::Use>, BotError> {
        let deferred = interaction.defer(&state).await?;
        let guild = deferred.guild().unwrap();

        let game = data.game.unwrap_or_else(|| *self.games.iter().exactly_one().unwrap());
        {
            let commands = state.commands.read().await;
            let mut commands = commands.get(&guild).unwrap()
                .write().await;
            let (stop_id, stop_cmd) = state.get_command_mut::<StopCommand>(guild, &mut commands).await;
            stop_cmd.insert(game);
            stop_cmd.edit_command(&state, guild, stop_id).await?;
        }

        match game {
            GameType::Avalon => avalon::start::start(&state, &deferred).await?,
            GameType::Hangman => todo!(),
            GameType::Coup => todo!(),
            GameType::Kittens => todo!(),
        }

        Ok(deferred)
    }
}