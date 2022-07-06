use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use tokio::time::Instant;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::MessageChannelExt;
use discorsd::model::ids::MessageId;

use crate::Bot;

#[derive(Debug, Clone)]
pub struct UnpinCommand;

#[async_trait]
impl SlashCommand for UnpinCommand {
    type Bot = Bot;
    type Data = UnpinData;
    type Use = Used;
    const NAME: &'static str = "unpin";

    fn description(&self) -> Cow<'static, str> {
        "Unpins all/some messages from this channel".into()
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<SlashCommandData, Self::Use>, BotError> {
        let interaction = interaction.defer(&state).await?;

        let start = Instant::now();
        let mut pinned = interaction.channel.get_pinned_messages(&state).await?;
        match data {
            UnpinData::All => {}
            UnpinData::Recent { number } => {
                if number >= 0 {
                    pinned.drain(number as usize..);
                } else {
                    pinned.drain(0..(-number) as usize);
                }
            }
            UnpinData::Old { number } => {
                if number >= 0 {
                    pinned.drain(0..pinned.len() - number as usize);
                } else {
                    pinned.drain((-number) as usize..);
                }
            }
            UnpinData::Messages(ids) => {
                pinned.retain(|id| ids.contains(&id.id));
            }
            UnpinData::Exclude(ids) => {
                pinned.retain(|id| !ids.contains(&id.id));
            }
        };

        let (mut ok, mut err) = (0, 0);
        for pin in pinned {
            match pin.unpin(&state).await {
                Ok(_) => ok += 1,
                Err(_) => err += 1,
            }
        }

        let message = match (ok, err) {
            (ok, 0) => format!("✅ Unpinned {} messages in {:?} ✅", ok, start.elapsed()),
            (0, err) => format!("❌ Failed to unpin {} messages in {:?} ❌", err, start.elapsed()),
            (ok, err) => format!("Unpinned {} of {} messages in {:?}", ok, ok + err, start.elapsed()),
        };
        interaction.edit(&state, message).await.map_err(|e| e.into())
    }
}

#[derive(CommandData, Debug)]
pub enum UnpinData {
    #[command(desc = "Unpin all messages in this channel")]
    All,
    #[command(desc = "Unpin the most recent n messages in this channel")]
    Recent {
        #[command(desc = "How many messages to unpin (negative unpins all but this many). Same as old (#pins - number)")]
        number: i64,
    },
    #[command(desc = "Unpin the oldest n messages in this channel")]
    Old {
        #[command(desc = "How many messages to unpin (negative unpins all but this many). Same as recent (#pins - number)")]
        number: i64,
    },
    #[command(desc = "Unpin specific messages in this channel by id")]
    Messages(
        #[command(vararg = "message", va_count = 25, va_req = 1)]
        HashSet<MessageId>
    ),
    #[command(desc = "Unpin all messages in this channel except for certain ids")]
    Exclude(
        #[command(vararg = "message", va_count = 25, va_req = 1)]
        HashSet<MessageId>
    ),
}