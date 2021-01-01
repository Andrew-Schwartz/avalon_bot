use std::fmt::{self, Display};
use std::sync::Arc;

use chrono::Utc;

use discorsd::{anyhow, async_trait, BotState};
use discorsd::http::channel::embed;
use discorsd::http::interaction::message;
use discorsd::http::model::{ApplicationCommandInteractionData, Color, Command, TopLevelOption};

use crate::Bot;
use crate::commands::{InteractionUse, NotUsed, SlashCommand, SlashCommandExt, Used};

#[derive(Copy, Clone, Debug)]
pub struct UptimeCommand;

pub const UPTIME_COMMAND: UptimeCommand = UptimeCommand;

#[async_trait]
impl SlashCommand for UptimeCommand {
    fn name(&self) -> &'static str { "uptime" }

    fn command(&self) -> Command {
        self.make(
            "How long has this bot been online for?",
            TopLevelOption::Empty,
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 _: ApplicationCommandInteractionData,
    ) -> anyhow::Result<InteractionUse<Used>> {
        let msg = if let Some(ready) = state.bot.first_log_in.get().cloned() {
            let embed = embed(|e| {
                e.color(Color::GOLD);
                e.title(Duration(Utc::now().signed_duration_since(ready)));
            });
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
        interaction.respond(&state, msg.with_source()).await.map_err(|e| e.into())
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