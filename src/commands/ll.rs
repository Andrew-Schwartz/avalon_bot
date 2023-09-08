use std::borrow::Cow;
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::MessageChannelExt;
use discorsd::model::ids::*;
use discorsd::model::message::TextMarkup;

use crate::Bot;
use crate::error::GameError;

#[derive(Copy, Clone, Debug)]
pub struct LowLevelCommand;

#[async_trait]
impl SlashCommand for LowLevelCommand {
    type Bot = Bot;
    type Data = Data;
    type Use = Used;
    const NAME: &'static str = "ll";

    fn description(&self) -> Cow<'static, str> {
        "perform a raw rest request to Discord".into()
    }

    fn default_permissions(&self) -> bool {
        false
    }

    async fn run(
        &self,
        state: Arc<BotState<Bot>>,
        interaction: InteractionUse<AppCommandData, Unused>,
        data: Self::Data,
    ) -> Result<InteractionUse<AppCommandData, Self::Use>, BotError<GameError>> {
        fn format<D: Debug>(d: D) -> Vec<String> {
            let mut vec = Vec::new();

            // pretty-printed result, with markdown escaped
            let mut string = format!("{d:#?}").replace('`', r"\`");
            while !string.is_empty() {
                const LEN: usize = "```rs\n```".len();
                let mut i = 2000 - LEN;
                while !string.is_char_boundary(i) {
                    i -= 1;
                }
                vec.push(string.drain(0..i).collect::<String>().code_block("rs"));
            }

            vec
        }
        let this_guild = interaction.guild().expect("ll only exists in testing server");

        let mut responses = match data {
            Data::Get(get) => match get {
                Get::User { user } => {
                    if let Some(user) = state.cache.user(user).await {
                        format(user)
                    } else if let Ok(user) = state.client.get_user(user).await {
                        format(user)
                    } else {
                        vec![String::from("Unknown user")]
                    }
                }
                Get::Message { channel, message_id, just_content } => {
                    if let Some(message) = state.cache.message(message_id).await {
                        if just_content { vec![message.content] } else { format(message) }
                    } else if let Ok(message) = state.client.get_message(channel, message_id).await {
                        if just_content { vec![message.content] } else { format(message) }
                    } else {
                        vec![String::from("Unknown message")]
                    }
                }
                Get::Member { user, guild } => {
                    let guild = guild.unwrap_or(this_guild);
                    if let Some(member) = state.cache.member(guild, user).await {
                        format(member)
                    } else if let Ok(member) = state.cache_guild_member(guild, user).await {
                        format(member)
                    } else {
                        vec![String::from("Unknown guild member")]
                    }
                }
                Get::Roles { guild } => {
                    let guild = guild.unwrap_or(this_guild);
                    if let Ok(roles) = state.client.get_guild_roles(guild).await {
                        format(roles)
                    } else {
                        vec![String::from("Unknown guild")]
                    }
                }
                Get::Guild { guild } => {
                    // todo
                    let guild = guild.unwrap_or(this_guild);
                    if let Some(guild) = state.cache.guild(guild).await {
                        format(guild)
                    }
                    // else if let Ok() = state.client.get_guild {
                    //
                    // }
                    else {
                        vec![String::from("Unknown guild")]
                    }
                }
            },
            Data::Post(post) => match post {
                Post::Message { channel, message } => {
                    match channel.send_unchecked(&state, message).await {
                        Ok(resp) => format(resp),
                        Err(e) => vec!["Failed to send message".into(), e.to_string()],
                    }
                }
                Post::MessageExternal { channel, message } => {
                    match ChannelId::from_str(&channel) {
                        Ok(channel) => match channel.send_unchecked(&state, message).await {
                            Ok(resp) => format(resp),
                            Err(e) => vec!["Failed to send message".into(), e.to_string()],
                        },
                        Err(e) => vec!["Failed to parse channel".into(), e.to_string()]
                    }
                }
            },
        };
        let interaction = interaction.respond(&state, responses.remove(0)).await?;
        for response in responses {
            interaction.followup(&state, response).await?;
        }

        Ok(interaction)
    }
}

// #[derive(Debug)]
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
        message_id: MessageId,
        #[command(default)]
        just_content: bool,
    },
    Member {
        user: UserId,
        #[command(desc = "The guild to fetch the member for, or this guild if not set.")]
        guild: Option<GuildId>,
    },
    Roles {
        #[command(desc = "The guild to fetch the roles for, or this guild if not set.")]
        guild: Option<GuildId>,
    },
    Guild {
        #[command(desc = "The guild to fetch, or this guild if not set.")]
        guild: Option<GuildId>,
    },
}

#[derive(CommandData, Debug)]
pub enum Post {
    Message {
        channel: ChannelId,
        message: String,
    },
    #[command(desc = "Send message in another server")]
    MessageExternal {
        // not ChannelId so clientside validation doesn't get rid of channel id's outside the guild
        channel: String,
        message: String,
    },
}