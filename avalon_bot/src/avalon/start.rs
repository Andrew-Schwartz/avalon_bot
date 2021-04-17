use std::collections::HashSet;
use std::sync::Arc;

use itertools::Itertools;
use log::warn;

use discorsd::{BotState, UserMarkupExt};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::{ChannelExt, embed};
use discorsd::http::ClientResult;
use discorsd::http::user::UserExt;
use discorsd::model::ids::Id;
use discorsd::model::message::ChannelMessageId;
use discorsd::model::message::Color;

use crate::avalon::characters::{Character, Loyalty};
use crate::avalon::characters::Character::{LoyalServant, MinionOfMordred};
use crate::avalon::config::AvalonConfig;
use crate::avalon::game::AvalonGame;
use crate::avalon::max_evil;
use crate::Bot;

pub async fn start(state: &Arc<BotState<Bot>>, interaction: &InteractionUse<Deferred>) -> Result<(), BotError> {
    let guild = interaction.guild().unwrap();
    let mut guard = state.bot.avalon_games.write().await;
    let avalon = guard.get_mut(&guild).unwrap();
    let game = avalon.start(interaction.channel);
    state.client.trigger_typing(game.channel).await?;
    let board = game.board_image();
    let AvalonGame { channel, players, lotl, .. } = game.clone();

    // send all of the players their roles
    let players = Arc::new(players);
    let mut handles = Vec::new();
    for player in Vec::clone(&*players) {
        let state = Arc::clone(state);
        let players = Arc::clone(&players);
        // task should not panic
        let handle = tokio::spawn(async move {
            let message = player.send_dm(&state, embed(|e| {
                let character = player.role;
                e.title(character.name());
                e.description(character.abilities());
                e.color(character.loyalty().color());
                let seen_characters = character.sees();
                if !seen_characters.is_empty() {
                    let sees = seen_characters.iter()
                        .map(|c| c.name())
                        .join("\n");
                    e.add_inline_field("You can see", sees);
                }
                let seen_players = players.iter()
                    .filter(|player| seen_characters.contains(&player.role))
                    .cloned()
                    .collect_vec();
                if !seen_players.is_empty() {
                    e.add_inline_field(
                        "You see",
                        seen_players.iter()
                            .filter(|other| state.bot.config.channel == channel || other.member.id() != player.member.id())
                            .map(|player| player.member.ping_nick())
                            .join("\n"),
                    );
                }
                let image = player.role.image();
                e.image(image);
            })).await?;
            // todo message this user to let them know their pins are full
            if let Err(e) = message.pin(&state).await {
                warn!("Failed to pin character: {}", e.display_error(&state).await);
            }
            Ok(ChannelMessageId::from(message))
        });
        handles.push(handle);
    }
    let pinned = futures::future::join_all(handles).await.into_iter()
        .map(|res| res.expect("character info tasks do not panic"))
        .collect::<ClientResult<HashSet<_>>>()?;
    game.pins.extend(pinned);

    // start info
    channel.send(&state, embed(|e| {
        e.title(format!("Avalon game with {} players", players.len()));
        e.color(Color::GOLD);
        e.add_inline_field(
            "Order of Leaders",
            players.iter()
                .enumerate()
                .map(|(i, player)| if i == 0 {
                    format!("{} - goes first", player.ping_nick())
                } else if lotl.filter(|lotl| *lotl == i).is_some() {
                    format!("{} - has the Lady of the Lake", player.ping_nick())
                } else {
                    player.ping_nick()
                })
                .join("\n"),
        );
        e.add_blank_inline_field();
        let (mut good, mut evil): (Vec<_>, _) = players.iter()
            .map(|p| p.role)
            .filter(|c| !matches!(c, LoyalServant | MinionOfMordred))
            .partition(|c| c.loyalty() == Loyalty::Good);
        good.sort_by_key(|c| c.name());
        evil.sort_by_key(|c| c.name());
        let (n_good, n_evil) = (good.len(), evil.len());
        let max_evil = max_evil(players.len()).unwrap();
        let max_good = players.len() - max_evil;
        let mut roles = good.into_iter().map(Character::name).join("\n");
        let ls = max_good - n_good;
        if ls != 0 {
            if n_good != 0 { roles.push('\n') }
            roles.push_str(&format!("{}x {}", ls, LoyalServant));
        }
        roles.push('\n');
        roles.push_str(&evil.into_iter().map(Character::name).join("\n"));
        let mom = max_evil - n_evil;
        if mom != 0 {
            if n_evil != 0 { roles.push('\n') }
            roles.push_str(&format!("{}x {}", mom, MinionOfMordred));
        }
        e.add_inline_field("The roles are", roles);
        if let Some(idx) = lotl {
            e.footer(
                format!("{} has the Lady of the Lake", players[idx].member.nick_or_name()),
                "images/avalon/lotl.jpg",
            );
        }
        if let Some(board) = board {
            e.image(board)
        }
    })).await?;

    let commands = state.commands.read().await;
    let commands = commands.get(&guild).unwrap().write().await;

    let disabled_commands = commands.iter()
        .map(|(_, c)| c)
        .map(|c| {
            let mut command = c.command();
            if AvalonConfig::is_setup_command(&**c) {
                command.default_permission = false;
            }
            command
        })
        .collect_vec();
    state.client.bulk_overwrite_guild_commands(state.application_id().await, guild, disabled_commands).await?;

    game.start_round(&*state, guild, commands).await?;
    Ok(())
}
