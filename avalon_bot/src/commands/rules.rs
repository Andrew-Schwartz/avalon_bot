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
impl SlashCommand for RulesCommand {
    type Bot = Bot;
    type Data = RulesData;
    type Use = Deferred;
    const NAME: &'static str = "rules";

    fn description(&self) -> Cow<'static, str> {
        "Explain the rules of a game".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: RulesData,
    ) -> Result<InteractionUse<Self::Use>, BotError> {
        let deferred = interaction.defer(&state).await?;
        let channel = match data.channel {
            Where::Dm => deferred.user().dm(&state).await?.id,
            Where::Here => deferred.channel,
        };
        // todo write rules
        channel.send(&state, format!("{} rules (wip still)", data.game)).await?;
        Ok(deferred)
    }
}

#[derive(Debug, CommandData)]
pub struct RulesData {
    #[command(default, desc = "The game to explain the rules for")]
    game: GameType,
    #[command(rename = "where", desc = "What channel to explain the rules in (the rules can be long)", default)]
    channel: Where,
}

#[derive(Debug, CommandDataChoices)]
pub enum Where {
    #[command(choice = "In this channel", default)]
    Here,
    #[command(choice = "In a dm")]
    Dm,
}

// impl OptionCtor for Where {
//     type Data = &'static str;
//
//     fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption {
//         DataOption::String(cdo)
//     }
// }