use std::collections::{BTreeSet, HashMap};
use std::collections::hash_map::Entry;
use std::sync::Arc;

use command_data_derive::CommandDataChoices;
use discorsd::{async_trait, BotState};
use discorsd::commands::{ButtonCommand, InteractionPayload, InteractionUse, Unused, Used};
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::http::ClientResult;
use discorsd::model::components::ButtonStyle;
use discorsd::model::ids::{MessageId, UserId};
use discorsd::model::interaction::{ButtonPressData, Token};
use discorsd::model::interaction_response::{InteractionMessage, message};
use discorsd::model::message::{ChannelMessageId, Color};
use itertools::Itertools;

use crate::Bot;
use crate::hangman::guess_letter::GuessCommand;
use crate::hangman::guess_word::GuessButton;
use crate::hangman::random_words::{channel_hist_word, server_hist_word, wordnik_word};

pub mod random_words;
pub mod guess_letter;
pub mod guess_word;

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
                message: ChannelMessageId { channel, message: MessageId(0) },
                word,
                source,
                guesses: BTreeSet::new(),
                wrong: 0,
                feedback: format!("React with a letter to guess!"),
                questioners: HashMap::new(),
            };
            let interaction = interaction.respond(&state, hangman.message(state)).await?;
            let message = interaction.get_message(&state).await?;
            message.react(&state, '❓').await?;
            hangman.token = interaction.token.clone();
            hangman.message.message = message.id;

            state.reaction_commands.write().await
                .push(Box::new(GuessCommand( hangman.message, interaction.token.clone())));

            vacant.insert(hangman);

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
    pub message: ChannelMessageId,
    pub word: String,
    pub source: String,
    pub guesses: BTreeSet<char>,
    pub wrong: usize,
    pub feedback: String,
    pub questioners: HashMap<UserId, MessageId>,
}

impl Hangman {
    pub async fn handle_end_game(
        &self,
        state: &BotState<Bot>,
        win: bool,
        lose: bool,
    ) -> ClientResult<bool> {
        if win {
            self.token.followup(&state, embed(|e| {
                e.color(Color::GOLD);
                e.title("You win!");
                e.description(format!("The word was {}.\n{}", self.word, self.source));
            })).await?;
            Ok(true)
        } else if lose {
            self.token.followup(&state, embed(|e| {
                e.color(Color::RED);
                e.title("You lose and the hangman gets to eat");
                e.description(format!("The word was {}.\n{}", self.word, self.source));
            })).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn message(&self, state: &BotState<Bot>) -> InteractionMessage {
        message(|m| {
            m.embed(|e| {
                e.title(format!("The hangman is hungry!\n{} letter word.", self.word.len()));
                e.description(format!("```\n{}\n```", ASCII_ART[self.wrong]));
                let revealed = self.word.chars()
                    .map(|c| if self.guesses.contains(&c) { c } else { '_' })
                    .join(" ");
                e.footer_text(format!("{}\n{}", revealed, self.feedback));
            });
            m.button(state, GuessButton(self.word.len()), |b| b.label("Guess word"));
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