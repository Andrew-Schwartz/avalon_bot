use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;

use crate::Bot;
use crate::error::GameError;

#[derive(Copy, Clone, Debug)]
pub struct PingCommand;

#[async_trait]
impl SlashCommand for PingCommand {
    type Bot = Bot;
    type Data = ();
    type Use = Used;
    const NAME: &'static str = "ping";

    fn description(&self) -> Cow<'static, str> {
        "pongs, and says how long it took".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<AppCommandData, Unused>,
                 _: (),
    ) -> Result<InteractionUse<AppCommandData, Used>, BotError<GameError>> {
        let start = Instant::now();
        let embed = embed(|e| e.title("Pong!"));
        let mut used = interaction.respond(&state, embed.clone()).await?;
        used.edit(&state, embed.build(|e|
            e.footer_text(format!("Took {:?} to respond", start.elapsed()))
        )).await?;
        Ok(used)
    }
}