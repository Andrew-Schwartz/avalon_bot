use std::sync::Arc;

use discorsd::{async_trait, BotState};
use discorsd::commands::ReactionCommand;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::model::emoji::Emoji;
use discorsd::model::ids::{ChannelId, MessageId};
use discorsd::model::interaction::Token;
use discorsd::model::message::Color;
use discorsd::shard::dispatch::{ReactionType, ReactionUpdate};

use crate::Bot;
use crate::hangman::ASCII_ART;

#[derive(Debug, Clone)]
pub struct GuessCommand(pub ChannelId, pub MessageId, pub Token);

#[async_trait]
impl ReactionCommand<Bot> for GuessCommand {
    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        let add_letter = reaction.kind == ReactionType::Add &&
            reaction.message_id == self.1 &&
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
        add_letter || remove_question
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<(), BotError> {
        let channel = self.0;

        let mut games = state.bot.hangman_games.write().await;
        let game = games.get_mut(&channel).unwrap();

        let guess = reaction.emoji.as_unicode().unwrap().chars().next().unwrap();
        if guess == '❓' {
            if reaction.user_id == state.cache.own_user().await.id { return Ok(()) }
            match reaction.kind {
                ReactionType::Add => {
                    let message = self.2.followup(&state, "React with a letter to guess!").await?;
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
        }
        game.token.edit(&state, game.embed()).await?;

        // handle win
        if game.word.chars().all(|c| game.guesses.contains(&c)) {
            game.token.followup(&state, embed(|e| {
                e.color(Color::GOLD);
                e.title("You win!");
                e.description(format!("The word was {}.\n{}", game.word, game.source));
            })).await?;
            games.remove(&channel);
        } else if game.wrong == ASCII_ART.len() - 1 {
            game.token.followup(&state, embed(|e| {
                e.color(Color::RED);
                e.title("You lose and the hangman gets to eat");
                e.description(format!("The word was {}.\n{}", game.word, game.source));
            })).await?;
            games.remove(&channel);
        }

        Ok(())
    }
}