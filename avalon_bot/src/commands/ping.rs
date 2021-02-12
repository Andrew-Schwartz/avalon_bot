use discorsd::{async_trait, BotState};
use std::sync::Arc;
use discorsd::model::interaction::{ApplicationCommandInteractionData, Command, TopLevelOption};
use crate::Bot;
use discorsd::http::channel::{ChannelExt, embed};
use std::time::Instant;
use discorsd::errors::BotError;
use discorsd::commands::*;

#[derive(Copy, Clone, Debug)]
pub struct PingCommand;

pub const PING_COMMAND: PingCommand = PingCommand;

#[async_trait]
impl SlashCommand<Bot> for PingCommand {
    fn name(&self) -> &'static str { "ping" }

    fn command(&self) -> Command {
        self.make(
            "pongs, and says how long it took",
            TopLevelOption::Empty,
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 _: ApplicationCommandInteractionData
    ) -> Result<InteractionUse<Used>, BotError> {
        let start = Instant::now();
        let mut resp = interaction.channel.send(&state, embed(|e| e.title("Pong!"))).await?;
        let embed = resp.embeds.remove(0);
        resp.edit(&state, embed.build(|e|
            e.footer_text(format!("Took {:?} to respond", start.elapsed()))
        )).await?;
        interaction.ack(&state).await.map_err(|e| e.into())
    }
}