use std::borrow::Cow;
use std::fmt::Debug;
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
    const NAME: &'static str = "ll";

    fn description(&self) -> Cow<'static, str> {
        "perform a raw rest request to Discord".into()
    }

    fn usable_by_everyone(&self) -> bool {
        false
    }

    async fn run(
        &self,
        state: Arc<BotState<Bot>>,
        interaction: InteractionUse<Unused>,
        data: Self::Data,
    ) -> Result<InteractionUse<Self::Use>, BotError> {
        fn format<D: Debug>(d: D) -> Vec<String> {
            let mut vec = Vec::new();

            // pretty-printed result, with markdown escaped
            let mut string = format!("{:#?}", d).replace('`', r"\`");
            while !string.is_empty() {
                const LEN: usize = "```rs\n```".len();
                let mut i = 2000 - LEN;
                while !string.is_char_boundary(i) {
                    i -= 1
                }
                vec.push(format!("```rs\n{}```", string.drain(0..i).collect::<String>()));
            }

            vec
        }

        let mut responses = match data {
            Data::Get(get) => match get {
                Get::User { user } => {
                    if let Some(user) = state.cache.user(user).await {
                        format(user)
                    } else if let Some(user) = state.client.get_user(user).await.ok() {
                        format(user)
                    } else {
                        vec![String::from("Unknown user")]
                    }
                }
                Get::Message { channel, message_id, just_content } => {
                    if let Ok(message) = message_id.parse() {
                        if let Some(message) = state.cache.message(message).await {
                            if just_content { vec![message.content] } else { format(message) }
                        } else if let Some(message) = state.client.get_message(channel, message).await.ok() {
                            if just_content { vec![message.content] } else { format(message) }
                        } else {
                            vec![String::from("Unknown message")]
                        }
                    } else {
                        vec![String::from("Invalid message id")]
                    }
                }
            },
            Data::Post(_) => todo!(),
        };
        let interaction = interaction.respond(&state, responses.remove(0)).await?;
        for response in responses {
            interaction.followup(&state, response).await?;
        }

        Ok(interaction)
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
        #[command(default)]
        just_content: bool,
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