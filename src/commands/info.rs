use std::borrow::Cow;
use std::sync::Arc;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::model::message::Color;
use discorsd::model::permissions::Permissions;

use crate::Bot;

#[derive(Clone, Debug)]
pub struct InfoCommand;

#[async_trait]
impl SlashCommand for InfoCommand {
    type Bot = Bot;
    type Data = ();
    type Use = Used;
    const NAME: &'static str = "info";

    fn description(&self) -> Cow<'static, str> {
        "Get some information about this bot".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 _data: (),
    ) -> Result<InteractionUse<SlashCommandData, Used>, BotError> {
        let user = state.user().await;
        interaction.respond(
            &state.client, embed(|e| {
                e.title("Avalon Bot");
                e.color(Color::GOLD);
                e.url("https://github.com/Andrew-Schwartz/avalon_bot");
                let perms = Permissions::ADD_REACTIONS
                    | Permissions::VIEW_CHANNEL
                    | Permissions::SEND_MESSAGES
                    | Permissions::MANAGE_MESSAGES
                    | Permissions::ATTACH_FILES
                    | Permissions::READ_MESSAGE_HISTORY
                    | Permissions::USE_EXTERNAL_EMOJIS
                    | Permissions::MANAGE_ROLES;
                let url = format!(
                    "https://discord.com/oauth2/authorize?scope=applications.commands%20bot&client_id={}&permissions={}",
                    user.id, perms.bits()
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