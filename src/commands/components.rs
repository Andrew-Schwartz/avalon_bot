use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::*;
use discorsd::{async_trait, BotState};
use discorsd::commands::{ButtonCommand, ButtonPressData, InteractionUse, MenuCommand, MenuSelectData, SlashCommand, SlashCommandData, Unused, Used};
use discorsd::errors::BotError;
use discorsd::http::interaction;
use discorsd::model::interaction_response::message;

use crate::Bot;
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
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 data: Data,
    ) -> Result<InteractionUse<SlashCommandData, Used>, BotError> {
        match data.component {
            ComponentType::Button => {
                interaction.respond(&state, message(|m| {
                    m.content("Message with buttons");
                    m.buttons(&state, vec![Box::new(TestButton) as _]);
                })).await.map_err(|e| e.into())
            }
            ComponentType::Menu => {
                interaction.respond(&state, message(|m| {
                    m.content("Message with a menu!");
                    m.menu(&state, TestMenu);
                })).await.map_err(|e| e.into())
            }
            ComponentType::Both => {
                interaction.respond(&state, message(|m| {
                    m.content("Message with a button and a message!");
                    m.buttons(&state, vec![Box::new(TestButton) as _]);
                    m.menu(&state, TestMenu);
                })).await.map_err(|e| e.into())
            }
        }
    }
}

#[derive(Copy, Clone)]
struct TestButton;

#[async_trait]
impl ButtonCommand for TestButton {
    type Bot = Bot;

    fn label(&self) -> String {
        "Click Me".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError> {
        let message = format!("click id = {:?}", interaction.data.custom_id);
        interaction.respond(state, message).await
            .map_err(|e| e.into())
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

    fn num_values(&self) -> (Option<u8>, Option<u8>) {
        (Some(1), Some(5))
    }

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<MenuSelectData<Self::Data>, Unused>,
    ) -> Result<InteractionUse<MenuSelectData<Self::Data>, Used>, BotError> {
        let chosen = interaction.data.values.iter()
            .list_grammatically(|d| format!("{:?}", d), "and");
        interaction.respond(state, format!("You selected: {}", chosen)).await
            .map_err(|e| e.into())
    }
}