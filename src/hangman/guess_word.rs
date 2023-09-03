use std::sync::Arc;

use discorsd::{async_trait, BotState};
use discorsd::commands::{ButtonCommand, InteractionUse, ModalCommand, Unused, Used};
use discorsd::errors::BotError;
use discorsd::model::components::{ComponentId, TextInput};
use discorsd::model::interaction::ButtonPressData;
use discorsd::model::interaction_response::{message, modal, ModalBuilder};
use discorsd::model::message::{Color, TextMarkup};
use itertools::Itertools;

use crate::Bot;

#[derive(Debug, Copy, Clone)]
pub struct GuessButton(pub usize);

#[async_trait]
impl ButtonCommand for GuessButton {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError> {
        let guard = state.bot.hangman_games.read().await;
        let Some(game) = guard.get(&interaction.channel) else {
            return interaction.respond(&state, message(|m| {
                m.ephemeral();
                m.embed(|e| {
                    e.color(Color::RED);
                    e.title("No Hangman in this channel :(");
                });
            })).await.map_err(Into::into);
        };
        let value = game.word.chars()
            .map(|c| if game.guesses.contains(&c) { c } else { '_' })
            .collect::<String>();
        interaction.respond_modal(
            &state,
            modal(
                &state,
                GuessModal,
                ModalBuilder::with_input(
                    "Guess the word!",
                    TextInput::new_short("Word")
                        .min_max_length(self.0, self.0)
                        .value(value)
                ),
            ),
        ).await.map_err(Into::into)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct GuessModal;

#[async_trait]
impl ModalCommand for GuessModal {
    type Bot = Bot;
    type Values = String;

    async fn run(
        &self,
        state: Arc<BotState<<Self as ModalCommand>::Bot>>,
        interaction: InteractionUse<ComponentId, Unused>,
        guess: String,
    ) -> Result<InteractionUse<ComponentId, Used>, BotError> {
        if guess.chars().any(|c| !c.is_ascii_alphabetic()) {
            return interaction.respond(&state, message(|m| {
                m.ephemeral();
                m.embed(|e| {
                    e.color(Color::RED);
                    e.title("You can only guess letters");
                    let bold_illegal = guess.chars().map(|c| if c.is_ascii_alphabetic() {
                        c.to_string()
                    } else {
                        c.bold()
                    }).collect::<String>();
                    let bold_illegal = bold_illegal.replace("****", "");
                    e.description(format!("Illegal characters highlighted: {bold_illegal}"));
                });
            })).await.map_err(Into::into);
        }

        let mut games_guard = state.bot.hangman_games.write().await;
        let channel = interaction.channel;
        let Some(game) = games_guard.get_mut(&channel) else {
            return interaction.respond(&state, message(|m| {
                m.ephemeral();
                m.embed(|e| {
                    e.color(Color::RED);
                    e.title("No Hangman in this channel :(");
                });
            })).await.map_err(Into::into);
        };

        let interaction = interaction.delete(&state).await?;

        let win = guess == game.word;
        let _win /* == win */ = game.handle_end_game(
            &state,
            win,
            false,
        ).await?;
        if win {
            game.guesses.extend(guess.chars());
            game.feedback = format!(r#"Correct! "{guess}" is the word!"#);
            game.token.edit(&state, game.message(&state)).await?;
            games_guard.remove(&channel);
        } else {
            // if there's only one letter left & the guess only has one new letter, mark that as one of the guessed letters
            let not_yet_guessed = game.word.chars()
                .filter(|c| !game.guesses.contains(c))
                .count();
            if not_yet_guessed == 1 {
                let new_letters = guess.chars()
                    .filter(|c| !game.word.contains(*c))
                    .collect_vec();
                if let &[new_letter] = new_letters.as_slice() {
                    let reaction = new_letter as u32 - 'a' as u32 + '🇦' as u32;
                    game.guesses.insert(new_letter);
                    game.token.edit(&state, game.message(&state)).await?;
                    game.message.react(&state, char::from_u32(reaction).unwrap()).await?;
                }
            }
            game.wrong += 1;
            game.feedback = format!(r#"Incorrect! "{guess}" is not the word!"#);
            game.token.edit(&state, game.message(&state)).await?;
        }

        Ok(interaction)
    }
}