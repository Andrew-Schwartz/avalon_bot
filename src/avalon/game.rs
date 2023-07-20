use std::collections::{HashMap, HashSet};

use tokio::sync::RwLockWriteGuard;

use discorsd::{BotState, GuildCommands};
use discorsd::commands::*;
use discorsd::http::channel::{create_message, embed, MessageChannelExt};
use discorsd::http::ClientResult;
use discorsd::model::ids::*;
use discorsd::model::message::{ChannelMessageId, Color};
use discorsd::model::user::UserMarkupExt;

use crate::avalon::board::Board;
use crate::avalon::vote::VoteStatus;
use crate::Bot;
use crate::commands::stop::StopVoteCommand;

use super::{
    assassinate::AssassinateCommand,
    Avalon,
    AvalonPlayer,
    characters::{Character::{Assassin, Merlin}, Loyalty::Evil},
    lotl::LotlCommand,
    quest::QuestCommand,
    rounds::{Round, Rounds},
    vote::{PartyVote, QuestVote},
};

#[derive(Debug, Clone)]
pub struct AvalonGame {
    pub state: AvalonState,
    pub channel: ChannelId,
    pub players: Vec<AvalonPlayer>,
    // pub roles: AvalonRoles,
    pub rounds: Rounds,
    pub board: Board,
    pub lotl: Option<usize>,
    pub leader: usize,
    pub round: usize,
    pub good_won: Vec<bool>,
    pub rejected_quests: usize,
    pub prev_ladies: Vec<UserId>,
    pub pins: HashSet<ChannelMessageId>,
    pub stop_votes: (i8, i8),
}

impl AvalonGame {
    pub fn new(channel: ChannelId,
               players: Vec<AvalonPlayer>,
               lotl: Option<usize>,
    ) -> Self {
        let rounds = Rounds(players.len());
        let board = Board::new(players.len());
        Self {
            state: AvalonState::GameStart,
            channel,
            players,
            rounds,
            board,
            lotl,
            leader: 0,
            round: 1,
            good_won: Vec::new(),
            rejected_quests: 0,
            prev_ladies: Vec::new(),
            pins: Default::default(),
            stop_votes: (0, 0),
        }
    }

    // has to be an associated fn because of &mut rules
    pub fn advance_leader(leader: &mut usize, num_players: usize) {
        *leader += 1;
        *leader %= num_players;
    }

    pub fn leader(&self) -> &AvalonPlayer {
        &self.players[self.leader]
    }

    pub fn lotl(&self) -> Option<&AvalonPlayer> {
        self.lotl.map(|l| &self.players[l])
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn player_ref<I: Id<Id=UserId>>(&self, id: I) -> Option<&AvalonPlayer> {
        let id = id.id();
        self.players.iter().find(|p| p.id() == id)
    }

    pub fn round(&self) -> Round {
        self.rounds[self.round]
    }

    pub fn board_image(&self) -> Option<(&'static str, Vec<u8>)> {
        self.board.image(&self.good_won, self.rejected_quests)
            .map(|image| ("board.jpg", image))
    }

    pub fn is_reaction_command(command: &dyn ReactionCommand<Bot>, guild: GuildId) -> bool {
        matches!(command.downcast_ref::<StopVoteCommand>(), Some(svc) if svc.1 == guild) ||
            matches!(command.downcast_ref::<PartyVote>(), Some(pv) if pv.guild == guild) ||
            matches!(command.downcast_ref::<QuestVote>(), Some(qv) if qv.guild == guild)
    }
}

impl Avalon {
    pub async fn end_round(
        &mut self,
        state: &BotState<Bot>,
        guild: GuildId,
        mut commands: RwLockWriteGuard<'_, GuildCommands<Bot>>,
    ) -> ClientResult<()> {
        let game = self.game_mut();
        let new_state = if game.good_won.iter().filter(|g| **g).count() == 3 {
            if let (Some(assassin), true) = (
                game.players.iter().find(|p| p.role == Assassin),
                game.players.iter().any(|p| p.role == Merlin),
            ) {
                game.channel.send(&state, create_message(|m| {
                    m.content(assassin.ping_nick());
                    m.embed(|e| {
                        e.title("The good guys have succeeded three quests, but the Assassin can still try to kill Merlin");
                        e.description("Use `/assassinate` to assassinate who you think is Merlin");
                        e.fields(
                            game.players
                                .iter()
                                .filter(|p| p.role.loyalty() == Evil)
                                .map(|p| (
                                    p.member.nick_or_name(),
                                    p.role,
                                    true
                                ))
                        );
                    });
                })).await?;
                let (assassinate_id, assassinate) = state.get_command_mut::<AssassinateCommand>(guild, &mut commands).await;
                // assassinate_id.allow_users(&state, guild, &[assassin.id()]).await?;
                assassinate.0 = assassin.id();
                assassinate.edit_command(&state, guild, assassinate_id).await?;

                AvalonState::Assassinate
            } else {
                return self.game_over(state, guild, commands, embed(|e| {
                    e.color(Color::BLUE);
                    e.title("The good guys win!");
                })).await;
            }
        } else if game.good_won.iter().filter(|g| !**g).count() == 3 {
            return self.game_over(state, guild, commands, embed(|e| {
                e.color(Color::RED);
                e.title("The bad guys win!");
            })).await;
        } else if let (Some(lotl), 2..=4) = (game.lotl(), game.round) {
            game.channel.send(&state, create_message(|m| {
                m.content(lotl.ping_nick());
                m.embed(|e| {
                    e.title(format!(
                        "Now {} will use the Lady of the Lake to find someone's alignment",
                        lotl.member.nick_or_name()
                    ));
                    e.description(
                        "Use `/lotl` to find someone's alignment. You can't use this on \
                                        someone who has already had the Lady of the Lake."
                    );
                });
            })).await?;
            let (lotl_id, lotl_command) = state.get_command_mut::<LotlCommand>(guild, &mut commands).await;
            lotl_command.0 = lotl.id();
            lotl_command.edit_command(&state, guild, lotl_id).await?;
            // lotl_id.allow_users(&state, guild, &[lotl.id()]).await?;

            AvalonState::Lotl
        } else {
            game.round += 1;
            AvalonGame::advance_leader(&mut game.leader, game.players.len());
            game.start_round(state, guild, commands).await?;
            AvalonState::RoundStart
        };
        game.state = new_state;
        Ok(())
    }
}

impl AvalonGame {
    pub async fn start_round(
        &mut self,
        state: &BotState<Bot>,
        guild: GuildId,
        mut commands: RwLockWriteGuard<'_, GuildCommands<Bot>>,
    ) -> ClientResult<()> {
        let round = self.round();
        // state.disable_command::<VoteStatus>(guild).await?;
        let (quest_id, quest) = state.get_command_mut::<QuestCommand>(guild, &mut commands).await;
        quest.0 = round.players;
        quest.edit_command(&state, guild, quest_id).await?;
        // quest_id.allow_users(&state, guild, &[self.leader().id()]).await?;

        self.channel.send(&state, create_message(|m| {
            m.content(self.leader().ping_nick());
            m.embed(|e| {
                e.color(Color::GOLD);
                e.title(format!("Quest {}: The leader is {}", self.round, self.leader().member.nick_or_name()));
                if let Some(lotl) = self.lotl() {
                    if self.round != 5 {
                        e.description(format!("{} has the Lady of the Lake", lotl.member.nick_or_name()));
                    }
                }
                e.add_field(
                    "Use `/quest` to choose who to send on the quest",
                    format!("Send {} players on the quest{}", round.players, if round.fails == 1 {
                        ".".into()
                    } else {
                        format!(", {} fails are needed for this quest to fail.", round.fails)
                    }),
                );
            });
        })).await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum AvalonState {
    GameStart,
    RoundStart,
    PartyVote(HashMap<(MessageId, UserId), i32>, Vec<UserId>),
    Questing(HashMap<(MessageId, UserId), i32>),
    Assassinate,
    Lotl,
}

impl AvalonState {
    pub fn party_vote_mut(&mut self) -> Option<&mut HashMap<(MessageId, UserId), i32>> {
        match self {
            Self::PartyVote(votes, _) => Some(votes),
            _ => None,
        }
    }

    pub fn questing_vote_mut(&mut self) -> Option<&mut HashMap<(MessageId, UserId), i32>> {
        match self {
            Self::Questing(votes) => Some(votes),
            _ => None,
        }
    }
}