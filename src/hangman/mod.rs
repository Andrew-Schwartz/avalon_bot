use std::collections::{BTreeSet, HashMap};
use std::collections::hash_map::Entry;
use std::sync::Arc;

use command_data_derive::CommandDataChoices;
use discorsd::{async_trait, BotState};
use discorsd::commands::{ButtonCommand, InteractionPayload, InteractionUse, Unused, Used};
use discorsd::errors::BotError;
use discorsd::http::channel::{embed, RichEmbed};
use discorsd::model::components::ButtonStyle;
use discorsd::model::ids::{MessageId, UserId};
use discorsd::model::interaction::{ButtonPressData, Token};
use discorsd::model::interaction_response::message;
use discorsd::model::message::Color;
use itertools::Itertools;

use crate::Bot;
use crate::hangman::guess::GuessCommand;
use crate::hangman::random_words::{channel_hist_word, server_hist_word, wordnik_word};

pub mod random_words;
pub mod guess;

#[derive(CommandDataChoices, Debug, Copy, Clone)]
pub enum Source {
    // todo: change to guild when that's done
    Wordnik,
    #[command(default)]
    Channel,
    Server,
}

pub async fn start<D: InteractionPayload + Send + Sync>(
    state: &BotState<Bot>,
    word_source: Source,
    interaction: InteractionUse<D, Unused>,
) -> Result<InteractionUse<D, Used>, BotError> {
    let channel = interaction.channel;
    let mut game_guard = state.bot.hangman_games.write().await;

    match game_guard.entry(channel) {
        Entry::Occupied(_) => interaction.respond(&state, message(|m| {
            m.ephemeral();
            m.embed(|e| {
                e.title("Hangman is already running in this channel");
                e.description("If the Hangman message has been deleted, press the button to re-start the game");
                e.color(Color::RED);
            });
            m.button(state, RestartGame(word_source), |b| {
                b.label("Restart Game");
                b.style(ButtonStyle::Secondary);
            });
        })).await.map_err(Into::into),
        Entry::Vacant(vacant) => {
            let res = match word_source {
                Source::Wordnik => wordnik_word(&state.client.client).await,
                Source::Channel => channel_hist_word(state, channel, interaction.guild()).await,
                Source::Server => server_hist_word(state, interaction.guild().ok_or(channel)).await,
            };
            let (word, source) = match res {
                Ok(word) => word,
                Err(err) => return interaction.respond(&state, message(|m| {
                    m.ephemeral();
                    m.embed(|e| {
                        e.title("Error getting word!");
                        e.description(format!("{err}"));
                        e.color(Color::RED);
                    });
                    m.button(state, RestartGame(word_source), |b| {
                        b.label("Restart Game");
                        b.style(ButtonStyle::Secondary);
                    });
                })).await.map_err(Into::into)
            };
            let mut hangman = Hangman {
                token: Token(String::new()),
                word,
                source,
                guesses: BTreeSet::new(),
                wrong: 0,
                feedback: String::new(),
                questioners: HashMap::new(),
            };
            let interaction = interaction.respond(&state, hangman.embed()).await?;
            let message = interaction.get_message(&state).await?;
            message.react(&state, '❓').await?;
            hangman.token = interaction.token.clone();
            vacant.insert(hangman);

            state.reaction_commands.write().await
                .push(Box::new(GuessCommand(channel, message.id, interaction.token.clone())));

            Ok(interaction)
        }
    }
}

#[derive(Debug, Clone)]
struct RestartGame(Source);

#[async_trait]
impl ButtonCommand for RestartGame {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError> {
        state.bot.hangman_games.write().await
            .remove(&interaction.channel);
            // .map(|h| h.word_source)
            // .unwrap_or_default();

        start(&state, self.0, interaction).await
    }
}

#[derive(Debug)]
pub struct Hangman {
    pub token: Token,
    pub word: String,
    pub source: String,
    pub guesses: BTreeSet<char>,
    pub wrong: usize,
    pub feedback: String,
    pub questioners: HashMap<UserId, MessageId>,
}

impl Hangman {
    pub fn embed(&self) -> RichEmbed {
        embed(|e| {
            e.title(format!("The hangman is hungry!\n{} letter word.", self.word.len()));
            e.description(format!("```\n{}\n```", ASCII_ART[self.wrong]));
            let revealed = self.word.chars()
                .map(|c| if self.guesses.contains(&c) { c } else { '_' })
                .join(" ");
            e.footer_text(format!("{}\n{}", revealed, self.feedback));
        })
    }
}

pub const ASCII_ART: [&str; 6] = [
    r"+-------------+
|             |
|
|
|
|
|
|
|
|        +---------+
|        |         |
+--------+---------+--------+
|                           |",
    r"+-------------+
|             |
|             O
|
|
|
|
|
|
|        +---------+
|        |         |
+--------+---------+--------+
|                           |",
    r"+-------------+
|             |
|             O
|             |
|             +
|             |
|             +
|
|
|        +---------+
|        |         |
+--------+---------+--------+
|                           |",
    r"+-------------+
|             |
|             O
|           \ | /
|            \+/
|             |
|             +
|
|
|        +---------+
|        |         |
+--------+---------+--------+
|                           |",
    r"+-------------+
|             |
|           \ O /
|            \|/
|             +
|             |
|             +
|            / \
|           /   \
|        +---------+
|        |         |
+--------+---------+--------+
|                           |",
    r"+-------------+
|             |
|           \ X /
|            \|/
|             +
|             |
|             +
|            / \
|           /   \
|
+---------------------------+
|                           |",
];