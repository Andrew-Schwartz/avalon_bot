use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::*;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::ChannelExt;
use discorsd::http::user::UserExt;

use crate::Bot;
use crate::games::GameType;

#[derive(Clone, Debug)]
pub struct RulesCommand;

#[async_trait]
impl SlashCommandData for RulesCommand {
    type Bot = Bot;
    type Data = RulesData;
    const NAME: &'static str = "rules";

    fn description(&self) -> Cow<'static, str> {
        "Explain the rules of a game".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: RulesData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let channel = match data.channel {
            Where::Dm => interaction.user().dm(&state).await?.id,
            Where::Here => interaction.channel,
        };
        // todo write rules
        channel.send(&state, format!("{} rules (wip still)", data.game)).await?;
        interaction.defer(&state).await.map_err(|e| e.into())
    }
}

#[derive(Debug, CommandData)]
pub struct RulesData {
    #[command(default, choices, desc = "The game to explain the rules for")]
    game: GameType,
    #[command(rename = "where", desc = "What channel to explain the rules in (the rules can be long)", default, choices)]
    channel: Where,
}

#[derive(Debug, CommandDataOption)]
pub enum Where {
    #[command(choice = "In this channel", default)]
    Here,
    #[command(choice = "In a dm")]
    Dm,
}