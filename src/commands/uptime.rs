use std::borrow::Cow;
use std::fmt::{self, Display};
use std::sync::Arc;

use chrono::Utc;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::model::interaction_response::message;
use discorsd::model::message::Color;

use crate::Bot;
use crate::error::GameError;

#[derive(Copy, Clone, Debug)]
pub struct UptimeCommand;

#[async_trait]
impl SlashCommand for UptimeCommand {
    type Bot = Bot;
    type Data = ();
    type Use = Used;
    const NAME: &'static str = "uptime";

    fn description(&self) -> Cow<'static, str> {
        "How long has this bot been online for?".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<AppCommandData, Unused>,
                 _: (),
    ) -> Result<InteractionUse<AppCommandData, Used>, BotError<GameError>> {
        let msg = if let Some(ready) = state.bot.first_log_in.get().copied() {
            let embed = embed(|e| {
                e.color(Color::GOLD);
                e.title(Duration(Utc::now().signed_duration_since(ready)).to_string());
            });
            // `map_or_else` tries to move `embed` in both branches, so it doesn't work
            if let Some(resume) = *state.bot.log_in.read().await {
                embed.build(|e| e.add_field("Time since last reconnect", Duration(Utc::now().signed_duration_since(resume))))
            } else {
                embed
            }.into()
        } else {
            log::warn!("somehow not connected, yet /uptime ran???");
            message(|m| {
                m.content("Not yet connected, somehow :/");
            })
        };
        interaction.respond(&state, msg).await.map_err(Into::into)
    }
}

struct Duration(chrono::Duration);

impl Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dur = self.0;
        let days = dur.num_days();
        if days > 0 {
            if days == 1 { write!(f, "1 day, ")? } else { write!(f, "{days} days, ")? }
            dur = dur - chrono::Duration::days(days);
        }
        let hours = dur.num_hours();
        if hours > 0 {
            if hours == 1 { write!(f, "1 hour, ")? } else { write!(f, "{hours} hours, ")? }
            dur = dur - chrono::Duration::hours(hours);
        }
        let mins = dur.num_minutes();
        if mins > 0 {
            if mins == 1 { write!(f, "1 minute, ")? } else { write!(f, "{mins} minutes, ")? }
            dur = dur - chrono::Duration::minutes(mins);
        }
        let secs = dur.num_seconds();
        dur = dur - chrono::Duration::seconds(secs);
        let millis = dur.num_milliseconds();
        write!(f, "{secs}.{millis} seconds")
    }
}