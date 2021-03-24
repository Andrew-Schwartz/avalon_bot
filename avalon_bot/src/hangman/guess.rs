use std::sync::Arc;

use discorsd::async_trait;
use discorsd::commands::ReactionCommand;
use discorsd::http::channel::embed;
use discorsd::model::emoji::Emoji;
use discorsd::model::ids::{GuildId, MessageId};
use discorsd::model::message::Color;
use discorsd::shard::dispatch::{ReactionType, ReactionUpdate};

use crate::avalon::{BotError, BotState};
use crate::Bot;
use crate::hangman::{ASCII_ART, HangmanPlayers};

#[derive(Debug, Clone)]
pub struct GuessCommand(pub GuildId, pub HangmanPlayers, pub MessageId);

#[async_trait]
impl ReactionCommand<Bot> for GuessCommand {
    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        reaction.kind == ReactionType::Add &&
            reaction.message_id == self.2 &&
            self.1.matches(reaction.user_id) &&
            match &reaction.emoji {
                Emoji::Custom(_) => false,
                Emoji::Unicode { name } => {
                    name.chars().next()
                        .filter(|c| ('🇦'..'🇿').contains(c))
                        .is_some()
                }
            }
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<(), BotError> {
        let guild = self.0;
        let guess = reaction.emoji.as_unicode().unwrap().chars().next().unwrap();
        let guess = std::char::from_u32(guess as u32 - ('🇦' as u32 - 'a' as u32)).unwrap();

        let mut games = state.bot.hangman_games.write().await;
        let hangman = games.get_mut(&guild).unwrap();
        let game = hangman.game_mut();

        println!("guess = {:?}", guess);
        if !game.guesses.contains(&guess) {
            let count = game.word.chars().filter(|&c| c == guess).count();
            game.feedback = if count == 0 {
                game.wrong += 1;
                format!("There are no {}'s in the word.", guess)
            } else {
                game.guesses.insert(guess);
                let (verb, plural) = match count {
                    1 => ("is", ""),
                    _ => ("are", "'s"),
                };
                format!("Correct! There {} {} {}{} in the word.", verb, count, guess, plural)
            };
        }
        game.handle_guess(&state).await?;

        // handle win
        if game.word.chars().all(|c| game.guesses.contains(&c)) {
            let description = format!("The word was {}\n{}", game.word, game.source);
            let guard = state.commands.read().await;
            let commands = guard.get(&guild).unwrap();
            hangman.game_over(&state, guild, commands.write().await, embed(|e| {
                e.color(Color::GOLD);
                e.title("You win!");
                e.description(description);
            })).await?;
        } else if game.wrong == ASCII_ART.len() - 1 {
            let description = format!("The word was {}\n{}", game.word, game.source);
            let guard = state.commands.read().await;
            let commands = guard.get(&guild).unwrap();
            hangman.game_over(&state, guild, commands.write().await, embed(|e| {
                e.color(Color::RED);
                e.title("You lose and the hangman gets to eat");
                e.description(description);
            })).await?;
        }

        Ok(())
    }
}