use std::sync::Arc;

use discorsd::{async_trait, BotState};
use discorsd::http::interaction;
use discorsd::http::model::{ApplicationCommandInteractionData, Color, Command, TopLevelOption};

use crate::Bot;
use crate::commands::{InteractionUse, NotUsed, SlashCommand, SlashCommandExt, Used};
use discorsd::errors::BotError;

#[derive(Clone, Debug)]
pub struct InfoCommand;

pub const INFO_COMMAND: InfoCommand = InfoCommand;

#[async_trait]
impl SlashCommand for InfoCommand {
    fn name(&self) -> &'static str { "info" }

    fn command(&self) -> Command {
        self.make("Get some information about this bot", TopLevelOption::Empty)
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 _data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let user = state.user().await;
        interaction.respond(
            &state.client,
            interaction::message(|m| m.embed(|e| {
                e.title("Avalon Bot");
                e.color(Color::GOLD);
                // todo update url
                e.url("https://github.com/Andrew-Schwartz/AvalonBot");
                let url = format!(
                    "https://discord.com/oauth2/authorize?scope=applications.commands%20bot&client_id={}&permissions=67497024",
                    user.id
                );
                e.description(format!(
                    "I can run games of Avalon and Hangman for you (maybe more in the future).\
                    \n\nTo add me to a server, go to {}.\
                    \n\nI'm running on Andrew's Raspberry Pi, so I should be online most of the time :)\
                    \n\nTo see my code, click the title up there (jk its not updated yet).", url));
                e.timestamp_now();
            })).with_source(),
        ).await.map_err(|e| e.into())
    }
}