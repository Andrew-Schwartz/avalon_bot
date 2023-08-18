use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;

use itertools::Itertools;
use log::error;
use tokio::sync::Mutex;
use tokio::time::Duration;

use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::{AvalonError, BotError};
use discorsd::http::channel::{embed, MessageChannelExt};
use discorsd::http::ClientResult;
use discorsd::http::user::UserExt;
use discorsd::model::emoji::Emoji;
use discorsd::model::ids::*;
use discorsd::model::message::{ChannelMessageId, Color, EmbedField};
use discorsd::model::user::UserMarkup;
use discorsd::shard::dispatch::{ReactionType::*, ReactionUpdate};

use crate::avalon::Avalon;
use crate::avalon::characters::Loyalty::Evil;
use crate::avalon::game::{AvalonGame, AvalonState};
use crate::Bot;
use crate::utils::ListIterGrammatically;

#[derive(Clone, Debug)]
pub struct PartyVote {
    pub guild: GuildId,
    pub messages: HashSet<(MessageId, UserId)>,
}

impl PartyVote {
    pub const APPROVE: char = '✅';
    pub const REJECT: char = '❌';
}

#[allow(clippy::use_self)]
#[async_trait]
impl ReactionCommand<Bot> for PartyVote {
    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        self.messages.contains(&(reaction.message_id, reaction.user_id))
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<(), BotError> {
        if let Emoji::Unicode { name } = &reaction.emoji {
            let delta = match name.chars().next() {
                Some(Self::APPROVE) => 1,
                Some(Self::REJECT) => -1,
                _ => 0,
            } * match reaction.kind {
                Add => 1,
                Remove => -1,
            };
            let mut guard = state.bot.avalon_games.write().await;
            let guild = self.guild;
            let avalon = guard.get_mut(&guild).unwrap();

            let game = avalon.game_mut();
            // we only show the board here if the quest is rejected
            let AvalonGame { state: avalon_state, .. } = game;
            if let AvalonState::PartyVote(votes, _) = avalon_state {
                let vote = votes.get_mut(&(reaction.message_id, reaction.user_id)).unwrap();
                *vote += delta;

                if votes.iter().filter(|(_, v)| **v == 0).count() == 0 {
                    let owned_avalon = std::mem::take(avalon);
                    match party_vote_results(Arc::clone(&state), guild, owned_avalon).await {
                        Ok(a) => {
                            *avalon = a;
                        }
                        Err((a, e)) => {
                            *avalon = a;
                            return Err(e);
                        }
                    }
                }
            } else {
                unreachable!("state: {:?}", game.state)
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct QuestVote {
    pub guild: GuildId,
    pub messages: HashSet<(MessageId, UserId)>,
}

impl QuestVote {
    pub const SUCCEED: char = '✅';
    pub const FAIL: char = '❌';
}

#[allow(clippy::use_self)]
#[async_trait]
impl ReactionCommand<Bot> for QuestVote {
    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        self.messages.contains(&(reaction.message_id, reaction.user_id))
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<(), BotError> {
        if let Emoji::Unicode { name } = &reaction.emoji {
            let delta = match name.chars().next() {
                Some(Self::SUCCEED) => 1,
                Some(Self::FAIL) => -1,
                _ => 0,
            } * match reaction.kind {
                Add => 1,
                Remove => -1,
            };

            let mut guard = state.bot.avalon_games.write().await;
            let avalon = guard.get_mut(&self.guild).unwrap();
            let game = avalon.game_mut();
            let AvalonGame { state: avalon_state, .. } = game;
            if let AvalonState::Questing(votes) = avalon_state {
                let vote = votes.get_mut(&(reaction.message_id, reaction.user_id)).unwrap();
                *vote += delta;

                if votes.iter().filter(|(_, v)| **v == 0).count() == 0 {
                    let owned_avalon = std::mem::take(avalon);
                    match quest_vote_results(Arc::clone(&state), self.guild, owned_avalon).await {
                        Ok(a) => {
                            *avalon = a;
                        }
                        Err((a, e)) => {
                            *avalon = a;
                            return Err(e);
                        }
                    }
                }
            } else {
                error!("unreachable state: `{:?}`, {}:{}:{}", game.state, file!(), line!(), column!());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct VoteStatus;

#[async_trait]
impl SlashCommand for VoteStatus {
    type Bot = Bot;
    type Data = ();
    type Use = Used;
    const NAME: &'static str = "vote-status";

    fn description(&self) -> Cow<'static, str> {
        "Find out who didn't vote yet".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<AppCommandData, Unused>,
                 _data: (),
    ) -> Result<InteractionUse<AppCommandData, Used>, BotError> {
        let guard = state.bot.avalon_games.read().await;
        let game = guard.get(&interaction.guild().unwrap()).unwrap().game_ref();
        match &game.state {
            AvalonState::PartyVote(votes, _)
            | AvalonState::Questing(votes) => {
                let not_voted = votes.iter()
                    .filter(|(_, vote)| **vote == 0)
                    .collect_vec();
                let list = not_voted.iter()
                    .list_grammatically(|((_, user), _)| user.ping(), "and");
                interaction.respond(&state.client, match not_voted.len() {
                    0 => "no one".to_string(),
                    1 => format!("{} has not voted", list),
                    _ => format!("{} have not voted", list),
                },
                ).await.map_err(Into::into)
            }
            _ => {
                interaction.respond(&state, "Everyone has voted").await.map_err(Into::into)
            }
        }
    }
}

pub async fn vote_checker<G, F, Fut>(
    state: Arc<BotState<Bot>>,
    guild: GuildId,
    yes_no: [char; 2],
    votes_getter: G,
    proceed: F,
) where
    G: Fn(&mut AvalonState) -> Option<&mut HashMap<(MessageId, UserId), i32>> + Send + Sync,
    F: Fn(Arc<BotState<Bot>>, GuildId, Avalon) -> Fut + 'static + Send + Sync,
    Fut: Future<Output=Result<Avalon, (Avalon, BotError)>> + 'static + Send,
{
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        let opt = (|| async {
            let mut game_guard = state.bot.avalon_games.write().await;
            let avalon = game_guard.get_mut(&guild)?;
            let game = avalon.try_game_mut()?;
            let votes = votes_getter(&mut game.state)?;
            let mut all_voted = true;
            for (&(msg, user), vote) in votes {
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                if let Ok(channel) = user.dm(&state).await {
                    let reactions: Result<Vec<_>, _> = state.client.get_all_reactions(
                        channel.id, msg, yes_no,
                    ).await.map(|vec| vec.into_iter()
                        .map(|vec| vec.into_iter()
                            .filter(|u| u.id == user)
                            .count() as i32)
                        .collect());
                    if let Ok(&[yes, no]) = reactions.as_deref() {
                        *vote = yes - no;
                        if *vote == 0 { all_voted = false }
                    }
                }
            }
            if all_voted {
                let owned_avalon = std::mem::take(avalon);
                match proceed(Arc::clone(&state), guild, owned_avalon).await {
                    Ok(a) => {
                        *avalon = a;
                    }
                    Err((a, e)) => {
                        *avalon = a;
                        error!("{}", e.display_error(&state).await);
                    }
                }
                None
            } else {
                Some(())
            }
        })().await;
        if opt.is_none() {
            break;
        }
    }
}

pub async fn party_vote_results(
    state: Arc<BotState<Bot>>,
    guild: GuildId,
    mut avalon: Avalon,
) -> Result<Avalon, (Avalon, BotError)> {
    // `avalon` is write locked, so it still in the game state
    let game = avalon.game_mut();
    let AvalonGame { state: avalon_state, players, .. } = game;
    if let AvalonState::PartyVote(votes, party) = avalon_state {
        let (approver, rejecter) = votes.iter()
            .partition::<Vec<_>, _>(|(_, v)| **v == 1);
        let vote_summary = approver.iter()
            .chain(&rejecter)
            .map(|(&(_, user), &vote)| EmbedField::new_inline(
                players.iter()
                    .find(|p| p.id() == user)
                    .unwrap()
                    .member
                    .nick_or_name(),
                if vote == 1 { "Approved" } else { "Rejected" },
            ))
            .collect_vec();

        let new_state = if rejecter.len() >= approver.len() {
            AvalonGame::advance_leader(&mut game.leader, players.len());
            game.rejected_quests += 1;
            let board = game.board_image();
            match game.rejected_quests {
                5 => {
                    let guard = state.slash_commands.read().await;
                    let commands = guard.get(&guild).unwrap()
                        .write().await;
                    let result = avalon.game_over(&state, guild, commands, embed(|e| {
                        e.color(Color::RED);
                        e.title("With 5 rejected parties in a row, the bad guys win");
                        if let Some(board) = board {
                            e.image(board);
                        }
                    })).await;
                    return match result {
                        Ok(()) => Ok(avalon),
                        Err(e) => Err((avalon, e.into())),
                    };
                }
                rejects => {
                    let result = game.channel.send(&state, embed(|e| {
                        match rejects {
                            1 => e.title("There is now 1 reject"),
                            r => e.title(format!("There are now {} rejects in a row", r)),
                        };
                        e.fields(vote_summary);
                        if let Some(board) = board {
                            e.image(board);
                        }
                    })).await;
                    if let Err(e) = result {
                        return Err((avalon, e.into()));
                    }
                    let guard = state.slash_commands.read().await;
                    let commands = guard.get(&guild).unwrap()
                        .write().await;
                    let result = game.start_round(&state, guild, commands).await;
                    if let Err(e) = result {
                        return Err((avalon, e.into()));
                    }
                    AvalonState::RoundStart
                }
            }
        } else {
            game.rejected_quests = 0;
            let result = game.channel.send(&state, embed(|e| {
                e.title("The party has been accepted!");
                e.fields(vote_summary);
            })).await;
            if let Err(e) = result {
                return Err((avalon, e.into()));
            }

            let mut handles = Vec::new();
            let command_idx: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
            for &user in &*party {
                let loyalty = players.iter()
                    .find(|p| p.id() == user)
                    .unwrap()
                    .role
                    .loyalty();
                let state = Arc::clone(&state);
                let command_idx = Arc::clone(&command_idx);
                let handle = tokio::spawn(async move {
                    let msg = user.send_dm(&*state, format!(
                        "React {} to succeed the quest{}",
                        QuestVote::SUCCEED,
                        if loyalty == Evil {
                            format!(", or {} to fail it", QuestVote::FAIL)
                        } else {
                            String::new()
                        }
                    )).await?;
                    let msg = ChannelMessageId::from(msg);

                    // build the vote command as we go so we don't miss any reactions
                    {
                        let mut idx_guard = command_idx.lock().await;
                        let mut rxn_commands = state.reaction_commands.write().await;
                        if let Some(idx) = *idx_guard {
                            let cmd = rxn_commands.get_mut(idx)
                                .ok_or(AvalonError::Stopped)?;
                            let qv = cmd.downcast_mut::<QuestVote>().unwrap();
                            qv.messages.insert((msg.message, user));
                        } else {
                            let idx = rxn_commands.len();
                            let vote = QuestVote { guild, messages: set!((msg.message, user)) };
                            rxn_commands.push(Box::new(vote));
                            *idx_guard = Some(idx);
                        }
                    }

                    let state = Arc::clone(&state);
                    tokio::spawn(async move {
                        msg.react(&state.client, QuestVote::SUCCEED).await?;
                        if loyalty == Evil {
                            msg.react(&state.client, QuestVote::FAIL).await?;
                        }
                        ClientResult::Ok(())
                    });

                    Result::<_, BotError>::Ok((msg.message, user))
                });
                handles.push(handle);
            }
            let mut votes = HashMap::new();
            for res in futures::future::join_all(handles).await {
                let msg = match res.expect("quote votes tasks do not panic") {
                    Ok(msg) => msg,
                    Err(e) => return Err((avalon, e)),
                };
                votes.insert(msg, 0);
            }

            tokio::spawn(vote_checker(
                Arc::clone(&state),
                guild,
                [QuestVote::SUCCEED, QuestVote::FAIL],
                AvalonState::questing_vote_mut,
                quest_vote_results,
            ));

            let quest_vote = QuestVote {
                guild,
                messages: votes.keys().copied().collect(),
            };
            state.reaction_commands.write().await
                .push(Box::new(quest_vote));

            // if let Err(e) = state.enable_command::<VoteStatus>(guild).await {
            //     return Err((avalon, e.into()));
            // }

            AvalonState::Questing(votes)
        };
        state.reaction_commands.write().await
            .retain(|rc|
                !matches!(
                    rc.downcast_ref::<PartyVote>(),
                    Some(PartyVote { guild: cmd_guild, .. }) if *cmd_guild == guild
                )
            );
        game.state = new_state;
    }
    Ok(avalon)
}

pub async fn quest_vote_results(
    state: Arc<BotState<Bot>>,
    guild: GuildId,
    mut avalon: Avalon,
) -> Result<Avalon, (Avalon, BotError)> {
    // `avalon` is write locked, so it still in the game state
    let game = avalon.game_mut();
    let round = game.round();
    if let AvalonState::Questing(votes) = &mut game.state {
        let fails = votes.iter().filter(|(_, v)| **v == -1).count();
        let questers = votes.keys().map(|(_, u)| u).list_grammatically(UserId::ping, "and");

        if fails >= round.fails {
            game.good_won.push(false);
            let result = game.channel.send(&state, embed(|e| {
                e.color(Color::RED);
                e.title(format!(
                    "There {}", if fails == 1 {
                        "was 1 fail".into()
                    } else {
                        format!("were {} fails", fails)
                    }
                ));
                e.description(format!("Reminder: {} were on this quest", questers));
                if let Some(board) = game.board_image() {
                    e.image(board);
                }
            })).await;
            if let Err(e) = result {
                return Err((avalon, e.into()));
            }
        } else {
            game.good_won.push(true);
            let nvotes = votes.len();
            let result = game.channel.send(&state, embed(|e| {
                e.color(Color::BLUE);
                e.title(if fails == 0 {
                    format!("All {} were successes", nvotes)
                } else {
                    format!("There were {} fails, but {} were required this quest", fails, round.fails)
                });
                e.description(format!("Reminder: {} were on this quest", questers));
                if let Some(board) = game.board_image() {
                    e.image(board);
                }
            })).await;
            if let Err(e) = result {
                return Err((avalon, e.into()));
            }
        };
        let guard = state.slash_commands.read().await;
        let commands = guard.get(&guild).unwrap()
            .write().await;
        let result = avalon.end_round(&state, guild, commands).await;
        if let Err(e) = result {
            return Err((avalon, e.into()));
        }

        state.reaction_commands.write().await
            .retain(|rc|
                !matches!(
                    rc.downcast_ref::<QuestVote>(),
                    Some(QuestVote { guild: cmd_guild, .. }) if *cmd_guild == guild
                )
            );
    }
    Ok(avalon)
}