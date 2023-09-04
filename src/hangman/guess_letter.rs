use std::sync::Arc;

use discorsd::{async_trait, BotState};
use discorsd::commands::ReactionCommand;
use discorsd::errors::BotError;
use discorsd::model::emoji::Emoji;
use discorsd::model::interaction::Token;
use discorsd::model::message::ChannelMessageId;
use discorsd::shard::dispatch::{ReactionType, ReactionUpdate};

use crate::Bot;
use crate::error::GameError;
use crate::hangman::ASCII_ART;

#[derive(Debug, Clone)]
pub struct GuessCommand(pub ChannelMessageId, pub Token);

#[async_trait]
impl ReactionCommand for GuessCommand {
    type Bot = Bot;

    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        let letter = reaction.message_id == self.0.message &&
            match &reaction.emoji {
                Emoji::Custom(_) => false,
                Emoji::Unicode { name } => {
                    name.chars().next()
                        .filter(|c| ('🇦'..'🇿').contains(c))
                        .is_some() || name == "❓"
                }
            };
        let remove_question = reaction.kind == ReactionType::Remove &&
            match &reaction.emoji {
                Emoji::Custom(_) => false,
                Emoji::Unicode { name } => name == "❓"
            };
        letter || remove_question
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<(), BotError<GameError>> {
        let channel = self.0.channel;

        let mut games = state.bot.hangman_games.write().await;
        let game = games.get_mut(&channel).unwrap();

        let guess = reaction.emoji.as_unicode().unwrap().chars().next().unwrap();
        if guess == '❓' {
            if reaction.user_id == state.cache.own_user().await.id { return Ok(()) }
            match reaction.kind {
                ReactionType::Add => {
                    let message = self.1.followup(&state, "React with a letter to guess!").await?;
                    game.questioners.insert(reaction.user_id, message.id);
                }
                ReactionType::Remove => {
                    if let Some(message) = game.questioners.remove(&reaction.user_id) {
                        state.client.delete_followup_message(
                            state.application_id(),
                            game.token.clone(),
                            message,
                        ).await?;
                    }
                }
            }
            return Ok(());
        }

        if reaction.kind == ReactionType::Remove {
            return self.0.react(&state, guess).await.map_err(Into::into);
        }

        let guess = std::char::from_u32(guess as u32 - ('🇦' as u32 - 'a' as u32)).unwrap();

        if !game.guesses.contains(&guess) {
            let count = game.word.chars().filter(|&c| c == guess).count();
            game.feedback = if count == 0 {
                game.wrong += 1;
                format!("There are no {guess}'s in the word.")
            } else {
                game.guesses.insert(guess);
                let (verb, plural) = match count {
                    1 => ("is", ""),
                    _ => ("are", "'s"),
                };
                format!("Correct! There {verb} {count} {guess}{plural} in the word.")
            };

            game.token.edit(&state, game.message(&state)).await?;

            let game_over = game.handle_end_game(
                &state,
                game.word.chars().all(|c| game.guesses.contains(&c)),
                game.wrong == ASCII_ART.len() - 1,
            ).await?;
            if game_over {
                games.remove(&channel);
            }
        }

        Ok(())
    }
}