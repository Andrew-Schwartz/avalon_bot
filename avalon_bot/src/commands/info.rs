use std::sync::Arc;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::model::message::Color;

use crate::Bot;
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct InfoCommand;

pub const INFO_COMMAND: InfoCommand = InfoCommand;

#[async_trait]
impl SlashCommandData for InfoCommand {
    type Bot = Bot;
    type Data = ();
    const NAME: &'static str = "info";

    fn description(&self) -> Cow<'static, str> {
        "Get some information about this bot".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 _data: (),
    ) -> Result<InteractionUse<Used>, BotError> {
        let user = state.user().await;
        interaction.respond(
            &state.client, embed(|e| {
                e.title("Avalon Bot");
                e.color(Color::GOLD);
                e.url("https://github.com/Andrew-Schwartz/avalon_bot");
                let url = format!(
                    "https://discord.com/oauth2/authorize?scope=applications.commands%20bot&client_id={}&permissions=67497024",
                    user.id
                );
                e.description(format!(
                    "I can run games of Avalon and Hangman for you (maybe more in the future).\
                    \n\nTo add me to a server, go to {}.\
                    \n\nI'm running on Andrew's Raspberry Pi, so I should be online most of the time :)\
                    \n\nTo see my code, click the title up there.", url));
                e.timestamp_now();
            }),
        ).await.map_err(|e| e.into())
    }
}