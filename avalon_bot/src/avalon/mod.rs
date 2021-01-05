use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::Arc;

use itertools::Itertools;
use rand::prelude::SliceRandom;

pub use discorsd::{async_trait, BotState, errors::BotError};
use discorsd::http::channel::{ChannelExt, embed, embed_with, RichEmbed};
use discorsd::http::ClientResult;
use discorsd::http::model::{ChannelId, Color, CommandId, GuildId, GuildMember, Id, MessageId, UserId};
pub use discorsd::http::model::interaction::{self, *};
use discorsd::http::user::UserExt;
use discorsd::UserMarkupExt;
use game::{AvalonGame, AvalonState};

pub use crate::{Bot, commands::*};
use crate::avalon::characters::Character::{self, LoyalServant, Merlin, MinionOfMordred};
use crate::avalon::characters::Loyalty::Evil;
use crate::avalon::config::AvalonConfig;

pub mod characters;
pub mod quest;
pub mod start;
pub mod roles;
pub mod rounds;
pub mod config;
pub mod vote;
pub mod assassinate;
pub mod lotl;
pub mod game;
pub mod board;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Avalon {
    Config(AvalonConfig),
    Game(AvalonGame),
}

impl Default for Avalon {
    fn default() -> Self {
        Self::Config(Default::default())
    }
}

impl Avalon {
    pub fn config_mut(&mut self) -> &mut AvalonConfig {
        if let Self::Config(cfg) = self {
            cfg
        } else {
            panic!("Expected Avalon to be in the Config state")
        }
    }

    pub fn game_mut(&mut self) -> &mut AvalonGame {
        if let Self::Game(game) = self {
            game
        } else {
            panic!("Expected Avalon to be in the Config state")
        }
    }

    pub fn game_ref(&self) -> &AvalonGame {
        if let Self::Game(game) = self {
            game
        } else {
            panic!("Expected Avalon to be in the Config state")
        }
    }

    pub fn start(&mut self, channel: ChannelId) -> &mut AvalonGame {
        let config = std::mem::take(self.config_mut());
        let max_evil = config.max_evil().unwrap();
        let AvalonConfig { mut players, mut roles, lotl, .. } = config;
        let num_evil = roles.iter()
            .filter(|c| c.loyalty() == Evil)
            .count();
        let num_good = roles.len() - num_evil;
        let mom = max_evil - num_evil;
        let ls = players.len() - max_evil - num_good;
        roles.extend((0..mom).map(|_| MinionOfMordred));
        roles.extend((0..ls).map(|_| LoyalServant));
        let mut rng = rand::thread_rng();
        roles.shuffle(&mut rng);
        players.shuffle(&mut rng);
        let players = players.into_iter()
            .map(|user| AvalonPlayer { member: user, role: roles.remove(0) })
            .collect_vec();
        let lotl = if lotl { Some(players.len() - 1) } else { None };
        *self = Self::Game(AvalonGame::new(channel, players, lotl));
        self.game_mut()
    }
}

#[derive(Clone, PartialEq)]
pub struct AvalonPlayer {
    pub member: GuildMember,
    pub role: Character,
}

impl Debug for AvalonPlayer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct Member<'a> {
            user: User<'a>,
            nick: &'a Option<String>,
        }
        #[derive(Debug)]
        struct User<'a> {
            id: &'a UserId,
            username: &'a String,
            discriminator: &'a String,
        }
        let member = Member {
            user: User {
                id: &self.member.id(),
                username: &self.member.user.username,
                discriminator: &self.member.user.discriminator,
            },
            nick: &self.member.nick,
        };
        f.debug_struct("AvalonPlayer")
            .field("member", &member)
            .field("role", &self.role)
            .finish()
    }
}

impl Id for AvalonPlayer {
    type Id = UserId;

    fn id(&self) -> Self::Id {
        self.member.id()
    }
}

impl Avalon {
    pub async fn game_over(
        &mut self,
        state: &BotState<Bot>,
        guild: GuildId,
        commands: &mut HashMap<CommandId, Box<dyn SlashCommand>>,
        embed: RichEmbed,
    ) -> ClientResult<()> {
        let game = self.game_ref();
        game.channel.send(&state.client, embed_with(embed, |e| {
            e.fields(
                game.players.iter()
                    .map(|p| (
                        p.member.nick_or_name(),
                        p.role.name(),
                        true
                    ))
            )
        })).await?;
        // todo keep people in the game?
        {
            let mut guard = state.bot.user_games.write().await;
            for player in &game.players {
                guard.entry(player.id())
                    .and_modify(|guilds| { guilds.remove(&guild); });
            }
        }
        for pin in &game.pins {
            let _ = pin.unpin(&state).await;
        }

        *self = Self::default();

        let rcs = state.bot.reaction_commands.write().await;
        state.bot.reset_guild_commands(&*state, commands, rcs, guild).await;
        Ok(())
    }
}

pub fn max_evil(num_players: usize) -> Option<usize> {
    match num_players {
        5..=6 => Some(2),
        7..=9 => Some(3),
        10 => Some(4),
        _ => None,
    }
}
