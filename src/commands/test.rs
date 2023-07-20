use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

// use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::model::interaction_response::message;

use crate::Bot;

#[derive(Debug, Copy, Clone)]
pub struct TestCommand;

// #[derive(Debug, CommandData)]
// pub struct TestData {
//     #[command(desc = "The upper limit of the random number range")]
//     upper: i64,
// }

#[async_trait]
impl SlashCommand for TestCommand {
    type Bot = Bot;
    type Data = ();
    type Use = Used;
    const NAME: &'static str = "test";

    fn description(&self) -> Cow<'static, str> {
        "Test stuff".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 (): Self::Data,
    ) -> Result<InteractionUse<SlashCommandData, Self::Use>, BotError> {
        println!("interaction = {:#?}", interaction);
        interaction.respond(&state, message(|m| {
            m.content("asdsad");
            m.embed(|e| {
                e.image(Path::new("images/avalon/avalonLogo.png"));
                e.title("TItesl");
            });
        })).await
            .map_err(|e| e.into())
    }
}