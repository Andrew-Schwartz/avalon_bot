use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::*;
use discorsd::async_trait;
use discorsd::commands::*;
use discorsd::http::channel::{ChannelExt, embed};
use discorsd::model::message::Color;

use crate::{Bot, hangman};
use crate::avalon::{BotError, BotState, InteractionUse, Unused, Used};
use crate::hangman::random_words::{GuildHist, Wordnik};
use crate::hangman::RandomWord;

#[derive(Debug, Clone)]
pub struct HangmanCommand;

#[async_trait]
impl SlashCommandData for HangmanCommand {
    type Bot = Bot;
    type Data = HangmanData;
    const NAME: &'static str = "hangman";

    fn description(&self) -> Cow<'static, str> {
        "Start a game of Hangman with the desired settings".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<Used>, BotError> {
        let guild = interaction.guild().unwrap();
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

        let used = interaction.defer(&state).await?;
        // todo this should be how more errors are handled, should have a nice way of doing this all at once
        if let Err(err) = hangman::start::start(Arc::clone(&state), &used).await {
            let error = err.display_error(&state).await;
            used.channel.send(&state, embed(|e| {
                e.title("Error starting Hangman!");
                e.color(Color::RED);
                e.description(error.to_string());
            })).await?;
            return Err(err);
        }

        Ok(used)
    }
}

#[derive(CommandData, Debug)]
pub struct HangmanData {
    #[command(default, desc = "Choose where to get the random word from")]
    word_source: Source,
}

#[derive(CommandDataOption, Debug)]
pub enum Source {
    Guild,
    // todo: change to guild when that's done
    #[command(default)]
    Wordnik,
}