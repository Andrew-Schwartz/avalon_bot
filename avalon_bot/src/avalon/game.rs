use std::collections::{HashMap, HashSet};

use discorsd::{BotState, UserMarkupExt};
use discorsd::commands::*;
use discorsd::http::channel::{ChannelExt, create_message, embed};
use discorsd::http::ClientResult;
use discorsd::model::ids::*;
use discorsd::model::message::{ChannelMessageId, Color};

use crate::{Bot, create_command, delete_command};
use crate::avalon::board::Board;
use crate::avalon::vote::VoteStatus;
use crate::commands::stop::{StopCommand, StopVoteCommand};

use super::{
    assassinate::Assassinate,
    Avalon,
    AvalonPlayer,
    characters::{Character::{Assassin, Merlin}, Loyalty::Evil},
    lotl::LotlCommand,
    quest::QuestCommand,
    rounds::{Round, Rounds},
    vote::{PartyVote, QuestVote},
};
use tokio::sync::RwLockWriteGuard;

#[derive(Debug, Clone)]
pub struct AvalonGame {
    pub state: AvalonState,
    pub channel: ChannelId,
    pub players: Vec<AvalonPlayer>,
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

    pub fn next_leader(leader: &mut usize, num_players: usize) {
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

    pub fn is_command(command: &dyn SlashCommand<Bot=Bot>) -> bool {
        command.is::<Assassinate>() ||
            command.is::<LotlCommand>() ||
            command.is::<QuestCommand>() ||
            command.is::<StopCommand>() ||
            command.is::<VoteStatus>()
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
        mut commands: RwLockWriteGuard<'_, HashMap<CommandId, Box<dyn SlashCommand<Bot=Bot>>>>,
    ) -> ClientResult<()> {
        let game = self.game_mut();
        let new_state = if game.good_won.iter().filter(|g| **g).count() == 3 {
            if let (Some(assassin), true) = (
                game.players.iter().find(|p| p.role == Assassin),
                game.players.iter().any(|p| p.role == Merlin),
            ) {
                game.channel.send(&state.client, create_message(|m| {
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
                let assassinate = Assassinate(assassin.id());
                create_command(&*state, guild, &mut commands, assassinate).await?;

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
            game.channel.send(&state.client, create_message(|m| {
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
            let lotl = LotlCommand(lotl.id());
            create_command(&*state, guild, &mut commands, lotl).await?;

            AvalonState::Lotl
        } else {
            game.round += 1;
            AvalonGame::next_leader(&mut game.leader, game.players.len());
            game.start_round(state, guild, &mut commands).await?;
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
        commands: &mut HashMap<CommandId, Box<dyn SlashCommand<Bot=Bot>>>,
    ) -> ClientResult<()> {
        let round = self.round();
        delete_command(
            &*state, guild, commands,
            |c| c.is::<VoteStatus>(),
        ).await?;
        let quest = QuestCommand(round.players);
        create_command(&*state, guild, commands, quest).await?;

        self.channel.send(&state.client, create_message(|m| {
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
