use std::collections::BTreeSet;
use std::fmt::{self, Display};
use std::mem;

use itertools::Itertools;
use tokio::sync::RwLockWriteGuard;

use discorsd::{BotState, GuildCommands, http, IdMap};
use discorsd::commands::*;
use discorsd::http::channel::{embed, MessageChannelExt, RichEmbed};
use discorsd::http::ClientResult;
use discorsd::model::ids::{ChannelId, GuildId, UserId};
use discorsd::model::message::Message;
use discorsd::model::user::UserMarkupExt;

use crate::Bot;
use crate::hangman::random_words::{GuildHist, Wordnik};

pub mod start;
pub mod hangman_command;
pub mod random_words;
mod guess;

pub fn commands() -> Vec<Box<dyn SlashCommandRaw<Bot=Bot>>> {
    vec![
        Box::new(hangman_command::HangmanCommand),
    ]
}

#[derive(Debug)]
pub enum RandomWord {
    Guild(GuildHist),
    Wordnik(Wordnik),
}

impl RandomWord {
    async fn word(&mut self) -> (String, String) {
        match self {
            Self::Guild(guild) => guild.word().await,
            Self::Wordnik(wordnik) => wordnik.word().await,
        }
    }
}

impl Display for RandomWord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // words from
        f.write_str(match self {
            Self::Guild(_) => "Messages in this server",
            Self::Wordnik(_) => "An online service",
        })
    }
}

impl Default for RandomWord {
    fn default() -> Self {
        Self::Wordnik(Wordnik::default())
    }
}

#[derive(Debug)]
pub enum Hangman {
    Config(HangmanConfig),
    Game(HangmanGame),
}

impl Default for Hangman {
    fn default() -> Self {
        Self::Config(Default::default())
    }
}

impl Hangman {
    pub fn config_mut(&mut self) -> &mut HangmanConfig {
        if let Self::Config(cfg) = self {
            cfg
        } else {
            panic!("Expected Hangman to be in the Config state")
        }
    }

    pub fn game_mut(&mut self) -> &mut HangmanGame {
        if let Self::Game(game) = self {
            game
        } else {
            panic!("Expected Hangman to be in the Game state")
        }
    }

    pub fn game_ref(&self) -> &HangmanGame {
        if let Self::Game(game) = self {
            game
        } else {
            panic!("Expected Hangman to be in the Game state")
        }
    }

    pub async fn start(&mut self, channel: ChannelId, mut ghw: RwLockWriteGuard<'_, IdMap<GuildHist>>) -> &mut HangmanGame {
        let mut config = std::mem::take(self.config_mut());
        let (word, source) = config.source.word().await;

        if let RandomWord::Guild(source) = config.source {
            ghw.insert(source);
        }
        // drop(ghw);

        *self = Self::Game(HangmanGame::new(channel, word, source, config.players));
        self.game_mut()
    }

    pub async fn game_over(
        &mut self,
        state: &BotState<Bot>,
        guild: GuildId,
        mut commands: RwLockWriteGuard<'_, GuildCommands<Bot>>,
        embed: RichEmbed,
    ) -> ClientResult<()> {
        let game = self.game_ref();
        game.channel.send(state, embed).await?;

        {
            let mut guard = state.bot.user_games.write().await;
            match &game.players {
                HangmanPlayers::Whitelist(players) => {
                    for player in players {
                        guard.entry(*player)
                            .and_modify(|guilds| { guilds.remove(&guild); });
                    }
                }
                HangmanPlayers::Anyone => {}
            }
        }
        *self = Self::default();

        let rcs = state.reaction_commands.write().await;
        Bot::reset_guild_command_perms(state, guild, &mut commands, rcs).await?;
        Ok(())
    }
}

#[derive(Default, Debug)]
pub struct HangmanConfig {
    pub players: Vec<UserId>,
    pub source: RandomWord,

    // the message being edited to show settings
    pub message: Option<Message>,
}

impl HangmanConfig {
    fn embed(&self) -> RichEmbed {
        embed(|e| {
            e.title("__Hangman Setup__");
            let players_list = self.players.iter()
                .map(UserMarkupExt::ping_nick)
                .join("\n");
            e.add_inline_field(
                format!("Players ({})", self.players.len()),
                if players_list.is_empty() { "None yet".into() } else { players_list },
            );
            e.add_blank_inline_field();
            e.add_inline_field("With words from", &self.source);
        })
    }

    pub async fn update_embed(
        &mut self,
        state: &BotState<Bot>,
        interaction: &InteractionUse<Deferred>,
    ) -> http::ClientResult<()> {
        let embed = self.embed();
        match &mut self.message {
            Some(message) if message.channel == interaction.channel => {
                // not a followup so it doesn't get deleted
                let new = interaction.channel.send(&state, embed).await?;
                let old = mem::replace(message, new);
                old.delete(&state.client).await?;
            }
            Some(_) | None => {
                let new = interaction.channel.send(&state, embed).await?;
                self.message = Some(new);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum HangmanPlayers {
    Anyone,
    Whitelist(BTreeSet<UserId>),
}

impl HangmanPlayers {
    pub fn matches(&self, player: UserId) -> bool {
        match self {
            Self::Anyone => true,
            Self::Whitelist(players) => players.contains(&player),
        }
    }
}

#[derive(Debug)]
pub struct HangmanGame {
    word: String,
    source: String,
    channel: ChannelId,
    players: HangmanPlayers,
    message: Option<Message>,
    guesses: BTreeSet<char>,
    wrong: usize,
    feedback: String,
}

impl HangmanGame {
    fn new(channel: ChannelId, word: String, source: String, players: Vec<UserId>) -> Self {
        let players = if players.is_empty() {
            HangmanPlayers::Anyone
        } else {
            HangmanPlayers::Whitelist(players.into_iter().collect())
        };

        Self { word, source, channel, players, message: None, guesses: Default::default(), wrong: 0, feedback: String::new() }
    }

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

    pub async fn handle_guess(&mut self, state: &BotState<Bot>) -> ClientResult<()> {
        let embed = self.embed();
        self.message.as_mut().unwrap().edit(state, embed).await?;

        Ok(())
    }
}

const ASCII_ART: [&str; 6] = [
    r#"+-------------+
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
|                           |"#,
    r#"+-------------+
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
|                           |"#,
    r#"+-------------+
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
|                           |"#,
    r#"+-------------+
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
|                           |"#,
    r#"+-------------+
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
|                           |"#,
    r#"+-------------+
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
|                           |"#,
];