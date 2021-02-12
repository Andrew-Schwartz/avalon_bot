use std::sync::Arc;

use command_data_derive::*;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::model::interaction::{ApplicationCommandInteractionData, Command, CommandDataOption, DataOption};

use crate::Bot;
use crate::games::GameType;
use discorsd::http::user::UserExt;
use discorsd::http::channel::ChannelExt;

#[derive(Clone, Debug)]
pub struct RulesCommand;

#[async_trait]
impl SlashCommand<Bot> for RulesCommand {
    fn name(&self) -> &'static str {
        "rules"
    }

    fn command(&self) -> Command {
        self.make(
            "Explain the rules of a game",
            RulesData::args(),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let data = RulesData::from_data(data, interaction.guild)?;
        let channel = match data.channel {
            Where::Dm => interaction.member.dm(&state).await?.id,
            Where::Here => interaction.channel
        };
        channel.send(&state, format!("{} rules (wip still)", data.game)).await?;
        interaction.ack(&state).await.map_err(|e| e.into())
    }
}

#[derive(Debug, CommandData)]
struct RulesData {
    #[command(default)]
    #[command(choices, desc = "The game to explain the rules for")]
    game: GameType,
    #[command(rename = "where", default)]
    #[command(choices, desc = "What channel to explain the rules in (the rules can be long)")]
    channel: Where,
}

#[derive(Debug, CommandDataOption)]
enum Where {
    #[command(choice = "In this channel", default)]
    Here,
    #[command(choice = "In a dm")]
    Dm,
}