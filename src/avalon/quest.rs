use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::{create_message, embed};
use discorsd::http::ClientResult;
use discorsd::http::user::UserExt;
use discorsd::model::ids::{Id, UserId};
use discorsd::model::interaction_response::message;
use discorsd::model::message::ChannelMessageId;
use discorsd::model::user::UserMarkup;
use itertools::Itertools;
use tokio::sync::Mutex;

use crate::avalon::AvalonPlayer;
use crate::avalon::game::AvalonState;
use crate::avalon::quest::QuestUserError::{Duplicate, NotPlaying};
use crate::avalon::vote::PartyVote;
use crate::Bot;
use crate::error::{AvalonError, GameError};
use crate::utils::ListIterGrammatically;

#[derive(Clone, Debug)]
pub struct QuestCommand(pub usize);

#[allow(clippy::use_self)]
#[async_trait]
impl SlashCommand for QuestCommand {
    type Bot = Bot;
    type Data = QuestData;
    type Use = Used;
    const NAME: &'static str = "quest";

    fn description(&self) -> Cow<'static, str> {
        "Choose who will go on the quest! Only the current leader can use this.".into()
    }

    fn default_permissions(&self) -> bool {
        false
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<AppCommandData, Unused>,
                 data: QuestData,
    ) -> Result<InteractionUse<AppCommandData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let mut guard = state.bot.avalon_games.write().await;
        let game = guard.get_mut(&guild).unwrap().game_mut();
        let leader = game.leader();
        let result = if interaction.user().id == leader.member.id() {
            match data.validate(&game.players) {
                Ok(party) => {
                    let result = interaction.respond(
                        &state.client,
                        embed(|e| {
                            e.title(format!("{} has proposed a party to go on this quest", leader.member.nick_or_name()));
                            e.description(party.iter().list_grammatically(UserId::ping, "and"));
                        }),
                    ).await;

                    // I think this we should only move on if this works?
                    if let Ok(interaction) = &result {
                        let guild = interaction.guild().unwrap();
                        let list_party = party.iter().list_grammatically(UserId::ping, "and");
                        let list_party = Arc::new(list_party);
                        let mut handles = Vec::new();
                        let command_idx: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
                        for player in &game.players {
                            let state = Arc::clone(&state);
                            let list_party = Arc::clone(&list_party);
                            let command_idx = Arc::clone(&command_idx);
                            let player = player.clone();
                            let handle = tokio::spawn(async move {
                                let msg = player.send_dm(&state, create_message(|m|
                                    m.content(format!(
                                        "React ✅ to vote to approve the quest, or ❌ to reject it.\
                                            \nThe proposed party is {}",
                                        list_party
                                    ))
                                )).await?;
                                let msg = ChannelMessageId::from(msg);

                                // build the vote command as we go so we don't miss any reactions
                                // (doesn't work perfectly but it helps)
                                {
                                    let mut idx_guard = command_idx.lock().await;
                                    let mut rxn_commands = state.reaction_commands.write().await;
                                    if let Some(idx) = *idx_guard {
                                        let cmd = rxn_commands.get_mut(idx)
                                            .ok_or(AvalonError::Stopped)?;
                                        let pv = cmd.downcast_mut::<PartyVote>().unwrap();
                                        pv.messages.insert((msg.message, player.id()));
                                    } else {
                                        let idx = rxn_commands.len();
                                        let vote = PartyVote { guild, messages: set!((msg.message, player.id())) };
                                        rxn_commands.push(Box::new(vote));
                                        *idx_guard = Some(idx);
                                    }
                                }

                                let state = Arc::clone(&state);
                                tokio::spawn(async move {
                                    msg.react(&state.client, PartyVote::APPROVE).await?;
                                    msg.react(&state.client, PartyVote::REJECT).await?;
                                    ClientResult::Ok(())
                                }).await.expect("reaction task does not panic")?;

                                Result::<_, BotError<GameError>>::Ok((msg.message, player.id()))
                            });
                            handles.push(handle);
                        }
                        let mut votes = HashMap::new();
                        for res in futures::future::join_all(handles).await {
                            let msg = res.expect("vote tasks do not panic")?;
                            votes.insert(msg, 0);
                        }

                        tokio::spawn(crate::avalon::vote::vote_checker(
                            Arc::clone(&state),
                            guild,
                            [PartyVote::APPROVE, PartyVote::REJECT],
                            AvalonState::party_vote_mut,
                            crate::avalon::vote::party_vote_results,
                        ));

                        // state.enable_command::<VoteStatus>(guild).await?;
                        // state.command_id::<QuestCommand>(guild).await
                        //     .disallow_users(&state, guild, &[leader.id()]).await?;
                        game.state = AvalonState::PartyVote(votes, party);
                    }
                    result
                }
                Err(errors) => {
                    interaction.respond(
                        &state.client, message(|m| {
                            m.ephemeral();
                            m.content(format!(
                                "__You must choose {} different people__\n{}",
                                self.0,
                                errors.into_iter()
                                    .map(|(user, error)| format!(
                                        "{} {}.",
                                        user.ping(),
                                        match error {
                                            NotPlaying => "is not playing Avalon",
                                            Duplicate => "was added multiple times",
                                        }
                                    )).join("\n")
                            ));
                        }),
                    ).await
                }
            }
        } else {
            interaction.respond(
                &state.client, message(|m| {
                    m.ephemeral();
                    m.content(format!("Only the current leader ({}) can choose who goes on the quest", leader.ping()));
                }),
            ).await
        };
        result.map_err(Into::into)
    }
}

enum QuestUserError {
    NotPlaying,
    Duplicate,
}

#[derive(CommandData, Debug)]
#[command(command = "QuestCommand")]
pub struct QuestData(
    #[command(va_ordinals, va_count = "num_players")]
    Vec<UserId>
);

fn num_players(command: &QuestCommand) -> usize {
    command.0
}

impl QuestData {
    fn validate(mut self, players: &[AvalonPlayer]) -> Result<Vec<UserId>, HashMap<UserId, QuestUserError>> {
        let mut unique = Vec::new();
        let mut errors = HashMap::new();
        while let Some(user) = self.0.pop() {
            if !players.iter().any(|p| p.id() == user) {
                errors.insert(user, NotPlaying);
            } else if unique.iter().any(|&u| u == user) {
                errors.insert(user, Duplicate);
            } else {
                unique.push(user);
            }
        }
        if errors.is_empty() {
            unique.reverse();
            Ok(unique)
        } else {
            Err(errors)
        }
    }
}