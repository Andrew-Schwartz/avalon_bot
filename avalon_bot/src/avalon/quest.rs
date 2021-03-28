use std::borrow::Cow;
use std::collections::HashMap;

use tokio::sync::Mutex;

use discorsd::errors::AvalonError;
use discorsd::http::channel::create_message;
use discorsd::model::message::ChannelMessageId;

use crate::{create_command, delete_command};
use crate::avalon::quest::QuestUserError::{Duplicate, NotPlaying};
use crate::avalon::vote::{PartyVote, VoteStatus};
use crate::utils::IterExt;

use super::*;

#[derive(Clone, Debug)]
pub struct QuestCommand(pub usize);

#[allow(clippy::use_self)]
#[async_trait]
impl SlashCommandData for QuestCommand {
    type Bot = Bot;
    type Data = QuestData;
    const NAME: &'static str = "quest";

    fn description(&self) -> Cow<'static, str> {
        "Choose who will go on the quest! Only the current leader can use this.".into()
    }

    fn options(&self) -> TopLevelOption {
        println!("QuestData::make_args(self) = {:#?}", QuestData::make_args(self));
        let data = ["First", "Second", "Third", "Fourth", "Fifth"].iter()
            .take(self.0)
            .enumerate()
            .map(|(i, s)| CommandDataOption::<UserId>::new(
                format!("player{}", i + 1), *s,
            ))
            .map(CommandDataOption::required)
            .map(DataOption::User)
            .collect();
        TopLevelOption::Data(data)
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: QuestData,
    ) -> Result<InteractionUse<Used>, BotError> {
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
                            e.description(party.iter().list_grammatically(UserId::ping_nick));
                        }),
                    ).await;

                    // I think this we should only move on if this works?
                    if let Ok(interaction) = &result {
                        let guild = interaction.guild().unwrap();
                        let list_party = party.iter().list_grammatically(UserId::ping_nick);
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

                                Result::<_, BotError>::Ok((msg.message, player.id()))
                            });
                            handles.push(handle);
                        }
                        let mut votes = HashMap::new();
                        for res in futures::future::join_all(handles).await {
                            let msg = res.expect("vote tasks do not panic")?;
                            votes.insert(msg, 0);
                        }

                        let guard = state.commands.read().await;
                        let mut commands = guard.get(&guild).unwrap().write().await;
                        // let party_vote = PartyVote {
                        //     guild,
                        //     messages: votes.keys().copied().collect(),
                        // };
                        // state.reaction_commands.write().await
                        //     .push(Box::new(party_vote));
                        create_command(&*state, guild, &mut commands, VoteStatus).await?;
                        delete_command(
                            &*state, guild, &mut commands,
                            |c| c.is::<QuestCommand>(),
                        ).await?;
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
                                        user.ping_nick(),
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
                    m.content(format!("Only the current leader ({}) can choose who goes on the quest", leader.ping_nick()));
                }),
            ).await
        };
        result.map_err(|e| e.into())
    }
}

enum QuestUserError {
    NotPlaying,
    Duplicate,
}

#[derive(CommandData)]
#[command(type = "QuestCommand")]
pub struct QuestData(
    #[command(vararg = "player", va_count = "num_players", ordinals)]
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