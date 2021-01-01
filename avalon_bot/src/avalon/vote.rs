use std::collections::HashSet;

use discorsd::http::model::{Color, Emoji, GuildId};
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

#[async_trait]
impl ReactionCommand for PartyVote {
    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        self.messages.contains(&(reaction.message_id, reaction.user_id))
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<()> {
        if let Emoji::Unicode { name } = &reaction.emoji {
            let delta = match name.chars().next() {
                Some(Self::APPROVE) => 1,
                Some(Self::REJECT) => -1,
                _ => 0,
            } * match reaction.kind {
                Add => 1,
                Remove => -1,
            };
            let mut guard = state.bot.games.write().await;
            let avalon = guard.get_mut(&self.guild).unwrap();
            let game = avalon.game_mut();
            let AvalonGame { state: avalon_state, players, .. } = game;
            if let AvalonState::PartyVote(votes, party) = avalon_state {
                let vote = votes.get_mut(&(reaction.message_id, reaction.user_id)).unwrap();
                *vote += delta;

                if votes.iter().filter(|(_, v)| **v == 0).count() == 0 {
                    let (approver, rejecter) = votes.iter()
                        .partition::<Vec<_>, _>(|(_, v)| **v == 1);
                    let vote_summary = approver.iter()
                        .chain(&rejecter)
                        .map(|(&(_, user), &vote)| (
                            players.iter()
                                .find(|p| p.id() == user)
                                .unwrap()
                                .member
                                .nick_or_name(),
                            if vote == 1 { "Approved" } else { "Rejected" },
                            /*inline*/ true,
                        ));

                    let new_state = if rejecter.len() >= approver.len() {
                        game.leader += 1;
                        game.rejected_quests += 1;
                        match game.rejected_quests {
                            5 => {
                                let guard = state.bot.commands.read().await;
                                let mut commands = guard.get(&self.guild).unwrap()
                                    .write().await;
                                return avalon.game_over(&state, self.guild, &mut commands, embed(|e| {
                                    e.color(Color::RED);
                                    e.title("With 5 rejected parties in a row, the bad guys win")
                                })).await.map_err(|e| e.into());
                            }
                            rejects => {
                                game.channel.send(&state, embed(|e| {
                                    match rejects {
                                        1 => e.title("There is now 1 reject"),
                                        r => e.title(format!("There are now {} rejects in a row", r)),
                                    };
                                    e.fields(vote_summary);
                                })).await?;
                                let guard = state.bot.commands.read().await;
                                let mut commands = guard.get(&self.guild).unwrap()
                                    .write().await;
                                game.start_round(&state, self.guild, &mut commands).await?;
                                AvalonState::RoundStart
                            }
                        }
                    } else {
                        game.rejected_quests = 0;
                        game.channel.send(&state, embed(|e| {
                            e.title("The party has been accepted!");
                            e.fields(vote_summary);
                        })).await?;
                        let mut votes = HashMap::new();
                        for user in &*party {
                            let loyalty = players.iter()
                                .find(|p| p.id() == *user)
                                .unwrap()
                                .role
                                .loyalty();
                            // let loyalty = game.player(user).unwrap().role.loyalty();
                            let msg = user.send_dm(&*state, format!(
                                "React {} to succeed the quest{}",
                                QuestVote::SUCCEED,
                                if loyalty == Evil {
                                    format!(", or {} to fail it", QuestVote::FAIL)
                                } else {
                                    "".into()
                                }
                            )).await?;
                            votes.insert((msg.id, *user), 0);
                            let state = Arc::clone(&state);
                            tokio::spawn(async move {
                                msg.react(&state.client, QuestVote::SUCCEED).await?;
                                if loyalty == Evil {
                                    msg.react(&state.client, QuestVote::FAIL).await?;
                                }
                                ClientResult::Ok(())
                            });
                        }
                        let quest_vote = QuestVote {
                            guild: self.guild,
                            messages: votes.keys().copied().collect(),
                        };
                        state.bot.reaction_commands.write().await
                            .push(Box::new(quest_vote));
                        let guard = state.bot.commands.read().await;
                        let mut commands = guard.get(&self.guild).unwrap()
                            .write().await;
                        create_command(&*state, self.guild, &mut commands, VoteStatus).await?;
                        AvalonState::Questing(votes)
                    };
                    state.bot.reaction_commands.write().await
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

#[async_trait]
impl ReactionCommand for QuestVote {
    fn applies(&self, reaction: &ReactionUpdate) -> bool {
        self.messages.contains(&(reaction.message_id, reaction.user_id))
    }

    async fn run(&self, state: Arc<BotState<Bot>>, reaction: ReactionUpdate) -> Result<()> {
        if let Emoji::Unicode { name } = &reaction.emoji {
            let delta = match name.chars().next() {
                Some(Self::SUCCEED) => 1,
                Some(Self::FAIL) => -1,
                _ => 0,
            } * match reaction.kind {
                Add => 1,
                Remove => -1,
            };
            let mut guard = state.bot.games.write().await;
            let avalon = guard.get_mut(&self.guild).unwrap();
            let game = avalon.game_mut();
            let round = game.round();
            let AvalonGame { state: avalon_state, .. } = game;
            if let AvalonState::Questing(votes) = avalon_state {
                let vote = votes.get_mut(&(reaction.message_id, reaction.user_id)).unwrap();
                *vote += delta;

                if votes.iter().filter(|(_, v)| **v == 0).count() == 0 {
                    let fails = votes.iter().filter(|(_, v)| **v == -1).count();

                    let msg = if fails >= round.fails {
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
                            e.description(format!(
                                "Reminder: {} were on this quest",
                                votes.keys().map(|(_, u)| u).list_grammatically(UserId::ping_nick)
                            ))
                        })).await?
                    } else {
                        game.good_won.push(true);
                        game.channel.send(&state.client, embed(|e| {
                            e.color(Color::BLUE);
                            e.title(if fails == 0 {
                                format!("All {} were successes", votes.len())
                            } else {
                                format!("There were {} fails, but {} were required this round", fails, round.fails)
                            });
                            e.description(format!(
                                "Reminder: {} were on this quest",
                                votes.keys().map(|(_, u)| u).list_grammatically(UserId::ping_nick)
                            ))
                        })).await?
                    };
                    let _ = game.pin(&state, msg).await;
                    let guard = state.bot.commands.read().await;
                    let mut commands = guard.get(&self.guild).unwrap()
                        .write().await;
                    avalon.end_round(&state, self.guild, &mut commands).await?;

                    state.bot.reaction_commands.write().await
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
impl SlashCommand for VoteStatus {
    fn name(&self) -> &'static str { "vote_status" }

    fn command(&self) -> Command {
        self.make("Find out who didn't vote yet", TopLevelOption::Empty)
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 _data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>> {
        let guard = state.bot.games.read().await;
        let game = guard.get(&interaction.guild).unwrap().game_ref();
        match &game.state {
            AvalonState::PartyVote(votes, _)
            | AvalonState::Questing(votes) => {
                let not_voted = votes.iter()
                    .filter(|(_, vote)| **vote == 0)
                    .collect_vec();
                let list = not_voted.iter()
                    .list_grammatically(|((_, user), _)| user.ping_nick());
                interaction.respond(&state.client, interaction::message(|m|
                    match not_voted.len() {
                        0 => m.content("no one"),
                        1 => m.content(format!("{} has not voted", list)),
                        _ => m.content(format!("{} have not voted", list)),
                    }
                ).with_source()).await.map_err(|e| e.into())
            }
            _ => {
                unreachable!("state: {:?}", game.state)
            }
        }
    }
}