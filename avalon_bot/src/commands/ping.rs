use std::sync::Arc;
use std::time::Instant;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;

use crate::Bot;
use std::borrow::Cow;

#[derive(Copy, Clone, Debug)]
pub struct PingCommand;

pub const PING_COMMAND: PingCommand = PingCommand;

#[async_trait]
impl SlashCommandData for PingCommand {
    type Bot = Bot;
    type Data = ();
    const NAME: &'static str = "ping";

    fn description(&self) -> Cow<'static, str> {
        "pongs, and says how long it took".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 _: (),
    ) -> Result<InteractionUse<Used>, BotError> {
        let start = Instant::now();
        let embed = embed(|e| e.title("Pong!"));
        let mut used = interaction.respond(&state, embed.clone()).await?;
        used.edit(&state, embed.build(|e|
            e.footer_text(format!("Took {:?} to respond", start.elapsed()))
        )).await?;
        Ok(used)
    }
}