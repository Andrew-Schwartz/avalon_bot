use std::borrow::Cow;
use std::fmt::{self, Display};
use std::sync::Arc;

use chrono::Utc;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::http::interaction::message;
use discorsd::model::message::Color;

use crate::Bot;

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
                 interaction: InteractionUse<Unused>,
                 _: (),
    ) -> Result<InteractionUse<Used>, BotError> {
        let msg = if let Some(ready) = state.bot.first_log_in.get().cloned() {
            let embed = embed(|e| {
                e.color(Color::GOLD);
                e.title(Duration(Utc::now().signed_duration_since(ready)).to_string());
            });
            // `map_or_else` tries to move `embed` in both branches, so it doesn't work
            if let Some(resume) = *state.bot.log_in.read().await {
                message(|m| m.embed_with(embed, |e| {
                    e.add_field("Time since last reconnect", Duration(Utc::now().signed_duration_since(resume)));
                }))
            } else {
                message(|m| m.embed(|e| *e = embed))
            }
        } else {
            log::warn!("somehow not connected, yet /uptime ran???");
            message(|m| {
                m.content("Not yet connected, somehow :/")
            })
        };
        interaction.respond(&state, msg).await.map_err(|e| e.into())
    }
}

struct Duration(chrono::Duration);

impl Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dur = self.0;
        let days = dur.num_days();
        if days > 0 {
            if days == 1 { write!(f, "1 day, ")? } else { write!(f, "{} days, ", days)? }
            dur = dur - chrono::Duration::days(days);
        }
        let hours = dur.num_hours();
        if hours > 0 {
            if hours == 1 { write!(f, "1 hour, ")? } else { write!(f, "{} hours, ", hours)? }
            dur = dur - chrono::Duration::hours(hours);
        }
        let mins = dur.num_minutes();
        if mins > 0 {
            if mins == 1 { write!(f, "1 minute, ")? } else { write!(f, "{} minutes, ", mins)? }
            dur = dur - chrono::Duration::minutes(mins);
        }
        let secs = dur.num_seconds();
        dur = dur - chrono::Duration::seconds(secs);
        let millis = dur.num_milliseconds();
        write!(f, "{}.{} seconds", secs, millis)
    }
}