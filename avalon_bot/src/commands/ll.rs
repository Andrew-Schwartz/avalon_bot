use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::CommandData;
use discorsd::async_trait;
use discorsd::commands::{SlashCommandData, Used};
use discorsd::model::ids::*;

use crate::avalon::{BotError, BotState, InteractionUse, Unused};
use crate::Bot;

#[derive(Copy, Clone, Debug)]
pub struct LowLevelCommand;

#[async_trait]
impl SlashCommandData for LowLevelCommand {
    type Bot = Bot;
    type Data = Data;
    type Use = Used;
    const NAME: &'static str = "rest";

    fn description(&self) -> Cow<'static, str> {
        "perform a raw rest request to Discord".into()
    }

    async fn run(
        &self,
        state: Arc<BotState<Bot>>,
        interaction: InteractionUse<Unused>,
        data: Self::Data,
    ) -> Result<InteractionUse<Self::Use>, BotError> {
        let response = match data {
            Data::Get(get) => match get {
                Get::User { user } => state.client
                    .get_user(user).await
                    .map_or_else(|_| String::from("Unknown user"),
                                 |user| format!("{:?}", user)),
                Get::Message { channel, message_id } => {
                    ({
                        let state = Arc::clone(&state);
                        || async move {
                            let message = message_id.parse().ok()?;
                            let message = state.client.get_message(channel, message)
                                .await.ok()?;
                            Some(format!("{:?}", message))
                        }
                    })().await.unwrap_or_else(|| String::from("Unknown message"))
                }
            },
            Data::Post(_) => todo!(),
        };
        interaction.respond(state, response).await.map_err(|e| e.into())
    }
}

#[derive(CommandData, Debug)]
pub enum Data {
    Get(Get),
    Post(Post),
    // Delete(String),
}

#[derive(CommandData, Debug)]
pub enum Get {
    User { user: UserId },
    Message {
        channel: ChannelId,
        message_id: String,
    },
}

#[derive(CommandData, Debug)]
pub enum Post {
    User { user: UserId },
    Message {
        channel: ChannelId,
        message: String,
    },
}