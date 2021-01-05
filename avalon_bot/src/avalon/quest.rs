use std::collections::HashMap;

use tokio::sync::Mutex;

use discorsd::errors::AvalonError;
use discorsd::http::channel::create_message;
use discorsd::UserMarkupExt;

use crate::{create_command, delete_command};
use crate::avalon::quest::QuestUserError::*;
use crate::avalon::vote::{PartyVote, VoteStatus};
use crate::utils::IterExt;

use super::*;
use discorsd::http::model::ChannelMessageId;

#[derive(Clone, Debug)]
pub struct QuestCommand(pub usize);

#[async_trait]
impl SlashCommand for QuestCommand {
    fn name(&self) -> &'static str { "quest" }

    fn command(&self) -> Command {
        let options = (1..=self.0).map(|i| {
            CommandDataOption::<UserId>::new(
                format!("player{}", i),
                format!("{} player", match i {
                    1 => "First",
                    2 => "Second",
                    3 => "Third",
                    4 => "Fourth",
                    5 => "Fifth",
                    6 => "Sixth",
                    _ => unreachable!(),
                }),
            ).required()
        }).map(DataOption::User).collect();
        self.make(
            "Choose who will go on the quest! Only the current leader can use this.",
            TopLevelOption::Data(options),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let data = QuestData::from(data);
        let mut guard = state.bot.games.write().await;
        let game = guard.get_mut(&interaction.guild).unwrap().game_mut();
        let leader = game.leader();
        let result = if interaction.member.id() != leader.member.id() {
            interaction.respond(
                &state.client,
                interaction::message(|m| {
                    m.ephemeral();
                    m.content(format!("Only the current leader ({}) can choose who goes on the quest", leader.ping_nick()));
                }).without_source(),
            ).await
        } else {
            match data.validate(&game.players) {
                Ok(party) => {
                    let result = interaction.respond(
                        &state.client,
                        interaction::message(|m| m.embed(|e| {
                            e.title(format!("{} has proposed a party to go on this quest", leader.member.nick_or_name()));
                            e.description(party.iter().list_grammatically(|u| u.ping_nick()));
                        })).with_source(),
                    ).await;

                    // I think this we should only move on if this works?
                    if let Ok(interaction) = &result {
                        let guild = interaction.guild;
                        let list_party = party.iter().list_grammatically(|u| u.ping_nick());
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
                                    let mut rxn_commands = state.bot.reaction_commands.write().await;
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

                        let guard = state.bot.commands.read().await;
                        let mut commands = guard.get(&guild).unwrap().write().await;
                        // let party_vote = PartyVote {
                        //     guild,
                        //     messages: votes.keys().copied().collect(),
                        // };
                        // state.bot.reaction_commands.write().await
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
                        &state.client,
                        interaction::message(|m| {
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
                        }).without_source(),
                    ).await
                }
            }
        };
        result.map_err(|e| e.into())
    }
}

enum QuestUserError {
    NotPlaying,
    Duplicate,
}

#[derive(Debug)]
struct QuestData(Vec<UserId>);

impl QuestData {
    fn validate(mut self, players: &[AvalonPlayer]) -> Result<Vec<UserId>, HashMap<UserId, QuestUserError>> {
        let mut unique = Vec::new();
        let mut errors = HashMap::new();
        for i in (0..self.0.len()).rev() {
            let user = self.0.remove(i);
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

impl From<ApplicationCommandInteractionData> for QuestData {
    fn from(data: ApplicationCommandInteractionData) -> Self {
        let data = data.options.into_iter()
            .map(|opt| opt.value.unwrap().unwrap_user())
            .collect();
        Self(data)
    }
}