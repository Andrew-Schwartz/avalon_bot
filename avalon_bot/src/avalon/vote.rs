use std::borrow::Cow;
use std::collections::HashSet;

use tokio::sync::Mutex;

use discorsd::errors::AvalonError;
use discorsd::model::emoji::Emoji;
use discorsd::model::ids::*;
use discorsd::model::message::{ChannelMessageId, Color, EmbedField};
use discorsd::shard::dispatch::{ReactionType::*, ReactionUpdate};
use discorsd::UserMarkupExt;

use crate::create_command;
use crate::utils::IterExt;

use super::*;

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
            let AvalonGame { state: avalon_state, players, .. } = game;
            if let AvalonState::PartyVote(votes, party) = avalon_state {
                let vote = votes.get_mut(&(reaction.message_id, reaction.user_id)).unwrap();
                *vote += delta;

                if votes.iter().filter(|(_, v)| **v == 0).count() == 0 {
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
                        AvalonGame::next_leader(&mut game.leader, players.len());
                        game.rejected_quests += 1;
                        let board = game.board_image();
                        match game.rejected_quests {
                            5 => {
                                let guard = state.commands.read().await;
                                let commands = guard.get(&guild).unwrap()
                                    .write().await;
                                return avalon.game_over(&state, guild, commands, embed(|e| {
                                    e.color(Color::RED);
                                    e.title("With 5 rejected parties in a row, the bad guys win");
                                    e.image(board);
                                })).await.map_err(|e| e.into());
                            }
                            rejects => {
                                game.channel.send(&state, embed(|e| {
                                    match rejects {
                                        1 => e.title("There is now 1 reject"),
                                        r => e.title(format!("There are now {} rejects in a row", r)),
                                    };
                                    e.fields(vote_summary);
                                    e.image(board);
                                })).await?;
                                let guard = state.commands.read().await;
                                let mut commands = guard.get(&guild).unwrap()
                                    .write().await;
                                game.start_round(&state, guild, &mut commands).await?;
                                AvalonState::RoundStart
                            }
                        }
                    } else {
                        game.rejected_quests = 0;
                        game.channel.send(&state, embed(|e| {
                            e.title("The party has been accepted!");
                            e.fields(vote_summary);
                        })).await?;
                        // let mut votes = HashMap::new();

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
                                        "".into()
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

                                // votes.insert((msg.id, *user), 0);

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
                            let msg = res.expect("quote votes tasks do not panic")?;
                            votes.insert(msg, 0);
                        }

                        // let quest_vote = QuestVote {
                        //     guild: self.guild,
                        //     messages: votes.keys().copied().collect(),
                        // };
                        // state.reaction_commands.write().await
                        //     .push(Box::new(quest_vote));
                        let guard = state.commands.read().await;
                        let mut commands = guard.get(&guild).unwrap()
                            .write().await;
                        create_command(&*state, guild, &mut commands, VoteStatus).await?;
                        AvalonState::Questing(votes)
                    };
                    state.reaction_commands.write().await
                        .retain(|rc|
                            !matches!(
                                rc.downcast_ref::<PartyVote>(),
                                Some(PartyVote { guild: cmd_guild, .. }) if *cmd_guild == self.guild
                            )
                        );
                    game.state = new_state;
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
            let round = game.round();
            let AvalonGame { state: avalon_state, .. } = game;
            if let AvalonState::Questing(votes) = avalon_state {
                let vote = votes.get_mut(&(reaction.message_id, reaction.user_id)).unwrap();
                *vote += delta;

                if votes.iter().filter(|(_, v)| **v == 0).count() == 0 {
                    let fails = votes.iter().filter(|(_, v)| **v == -1).count();
                    let questers = votes.keys().map(|(_, u)| u).list_grammatically(UserId::ping_nick);

                    if fails >= round.fails {
                        game.good_won.push(false);
                        game.channel.send(&state.client, embed(|e| {
                            e.color(Color::RED);
                            e.title(format!(
                                "There {}", if fails == 1 {
                                    "was 1 fail".into()
                                } else {
                                    format!("were {} fails", fails)
                                }
                            ));
                            e.description(format!("Reminder: {} were on this quest", questers));
                            e.image(game.board_image());
                        })).await?
                    } else {
                        game.good_won.push(true);
                        let nvotes = votes.len();
                        game.channel.send(&state.client, embed(|e| {
                            e.color(Color::BLUE);
                            e.title(if fails == 0 {
                                format!("All {} were successes", nvotes)
                            } else {
                                format!("There were {} fails, but {} were required this quest", fails, round.fails)
                            });
                            e.description(format!("Reminder: {} were on this quest", questers));
                            e.image(game.board_image());
                        })).await?
                    };
                    // let _ = game.pin(&state, msg).await;
                    let guard = state.commands.read().await;
                    let commands = guard.get(&self.guild).unwrap()
                        .write().await;
                    avalon.end_round(&state, self.guild, commands).await?;

                    state.reaction_commands.write().await
                        .retain(|rc|
                            !matches!(
                                rc.downcast_ref::<QuestVote>(),
                                Some(QuestVote { guild: cmd_guild, .. }) if *cmd_guild == self.guild
                            )
                        );
                }
            } else {
                unreachable!("state: {:?}", game.state)
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct VoteStatus;

#[async_trait]
impl SlashCommandData for VoteStatus {
    type Bot = Bot;
    type Data = ();
    type Use = Used;
    const NAME: &'static str = "vote-status";

    fn description(&self) -> Cow<'static, str> {
        "Find out who didn't vote yet".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 _data: (),
    ) -> Result<InteractionUse<Used>, BotError> {
        let guard = state.bot.avalon_games.read().await;
        let game = guard.get(&interaction.guild().unwrap()).unwrap().game_ref();
        match &game.state {
            AvalonState::PartyVote(votes, _)
            | AvalonState::Questing(votes) => {
                let not_voted = votes.iter()
                    .filter(|(_, vote)| **vote == 0)
                    .collect_vec();
                let list = not_voted.iter()
                    .list_grammatically(|((_, user), _)| user.ping_nick());
                interaction.respond(&state.client, match not_voted.len() {
                    0 => "no one".to_string(),
                    1 => format!("{} has not voted", list),
                    _ => format!("{} have not voted", list),
                },
                ).await.map_err(|e| e.into())
            }
            _ => {
                unreachable!("state: {:?}", game.state)
            }
        }
    }
}