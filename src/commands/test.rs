use std::borrow::Cow;
use std::sync::Arc;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use command_data_derive::CommandData;
use rand::Rng;

use crate::Bot;

#[derive(Debug, Copy, Clone)]
pub struct Test;

#[derive(Debug, CommandData)]
pub struct TestData {
    #[command(desc = "The lower limit of the random number range")]
    lower: i64,
    #[command(desc = "The upper limit of the random number range")]
    upper: i64,
}

#[async_trait]
impl SlashCommand for Test {
    type Bot = Bot;
    type Data = TestData;
    type Use = Used;
    const NAME: &'static str = "random-number";

    fn description(&self) -> Cow<'static, str> {
        "Generate a random number".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 Self::Data { lower, upper }: Self::Data,
    ) -> Result<InteractionUse<SlashCommandData, Self::Use>, BotError> {
        let n = rand::thread_rng().gen_range(lower..upper);
        interaction.respond(state, n.to_string())
            .await.map_err(|e| e.into())
    }
}