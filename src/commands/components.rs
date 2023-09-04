use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::*;
use discorsd::{async_trait, BotState};
use discorsd::commands::{ButtonCommand, InteractionUse, MenuCommand, SlashCommand, AppCommandData, Unused, Used};
use discorsd::errors::BotError;
use discorsd::model::interaction_response::message;
use discorsd::model::interaction::{ButtonPressData, MenuSelectData};

use crate::Bot;
use crate::error::GameError;
use crate::utils::ListIterGrammatically;

#[derive(Debug, Copy, Clone)]
pub struct ComponentsCommand;

#[derive(CommandData, Debug)]
pub struct Data {
    component: ComponentType,
}

#[derive(CommandDataChoices, Debug)]
pub enum ComponentType {
    Button,
    Menu,
    Both,
}

#[async_trait]
impl SlashCommand for ComponentsCommand {
    type Bot = Bot;
    type Data = Data;
    type Use = Used;
    const NAME: &'static str = "components";

    fn description(&self) -> Cow<'static, str> {
        "Test out the new message components".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<AppCommandData, Unused>,
                 data: Data,
    ) -> Result<InteractionUse<AppCommandData, Used>, BotError<GameError>> {
        match data.component {
            ComponentType::Button => {
                interaction.respond(&state, message(|m| {
                    m.content("Message with buttons");
                    m.button(&state, TestButton, |b| b.label("Click Me!"));
                })).await.map_err(Into::into)
            }
            ComponentType::Menu => {
                interaction.respond(&state, message(|m| {
                    m.content("Message with a menu!");
                    m.menu(&state, TestMenu, |m| m.min_max_values(1, 5));
                })).await.map_err(Into::into)
            }
            ComponentType::Both => {
                interaction.respond(&state, message(|m| {
                    m.content("Message with a button and a message!");
                    m.button(&state, TestButton, |b| b.label("Click Me!"));
                    m.menu(&state, TestMenu, |m| m.min_max_values(1, 5));
                })).await.map_err(Into::into)
            }
        }
    }
}

#[derive(Copy, Clone)]
struct TestButton;

#[async_trait]
impl ButtonCommand for TestButton {
    type Bot = Bot;

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError<GameError>> {
        let message = format!("click id = {:?}", interaction.data.custom_id);
        interaction.respond(state, message).await
            .map_err(Into::into)
    }
}

#[derive(Copy, Clone)]
struct TestMenu;

#[derive(Debug)]
#[derive(MenuCommand)]
enum TestMenuData {
    Assassin,
    Merlin,
    Mordred,
    Morgana,
    Oberon,
    Percival,
}

#[async_trait]
impl MenuCommand for TestMenu {
    type Bot = Bot;
    type Data = TestMenuData;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        data: Vec<Self::Data>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        let chosen = data.iter()
            .list_grammatically(|d| format!("{:?}", d), "and");
        interaction.respond(state, format!("You selected: {}", chosen)).await
            .map_err(Into::into)
    }
}