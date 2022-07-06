use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::*;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::{embed, MessageChannelExt};
use discorsd::model::message::Color;

use crate::{Bot, hangman};
use crate::hangman::random_words::{GuildHist, Wordnik};
use crate::hangman::RandomWord;

// todo can be global now
#[derive(Debug, Clone)]
pub struct HangmanCommand;

#[async_trait]
impl SlashCommand for HangmanCommand {
    type Bot = Bot;
    type Data = HangmanData;
    type Use = Deferred;
    const NAME: &'static str = "hangman";

    fn description(&self) -> Cow<'static, str> {
        "Start a game of Hangman with the desired settings".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<SlashCommandData, Self::Use>, BotError> {
        let guild = interaction.guild().unwrap();
        let deferred = interaction.defer(&state).await?;

        {
            let mut guard = state.bot.hangman_games.write().await;
            let config = guard
                .entry(guild)
                .or_default()
                .config_mut();

            config.source = match data.word_source {
                Source::Guild if !matches!(config.source, RandomWord::Guild(_)) => {
                    let mut guard = state.bot.guild_hist_words.write().await;
                    let ghw = if let Some(ghw) = guard.remove(&guild) {
                        ghw
                    } else {
                        let guild = state.cache.guild(guild).await.unwrap();
                        GuildHist::new(guild)
                    };
                    RandomWord::Guild(ghw)
                }
                Source::Wordnik if !matches!(config.source, RandomWord::Wordnik(_)) => RandomWord::Wordnik(Wordnik::default()),
                _ => std::mem::take(&mut config.source),
            }
        }

        // todo this should be how more errors are handled, should have a nice way of doing this all at once
        if let Err(err) = hangman::start::start(&state, &deferred).await {
            let error = err.display_error(&state).await;
            deferred.channel.send(&state, embed(|e| {
                e.title("Error starting Hangman!");
                e.color(Color::RED);
                e.description(error.to_string());
            })).await?;
            return Err(err);
        }

        Ok(deferred)
    }
}

#[derive(CommandData, Debug)]
pub struct HangmanData {
    #[command(default, desc = "Choose where to get the random word from")]
    word_source: Source,
}

#[derive(CommandDataChoices, Debug)]
pub enum Source {
    Guild,
    // todo: change to guild when that's done
    #[command(default)]
    Wordnik,
}