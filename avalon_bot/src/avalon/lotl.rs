use discorsd::http::channel::create_message;

use crate::delete_command;

use super::*;

#[derive(Clone, Debug)]
pub struct LotlCommand(pub UserId);

#[async_trait]
impl SlashCommand<Bot> for LotlCommand {
    fn name(&self) -> &'static str { "lotl" }

    fn command(&self) -> Command {
        self.make(
            "Learn a player's true alignment",
            LadyData::args()
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let result = if interaction.member.id() == self.0 {
            let target = LadyData::from_data(data, interaction.guild)?.target;
            let mut guard = state.bot.games.write().await;
            let game = guard.get_mut(&interaction.guild).unwrap().game_mut();
            match game.player_ref(target).cloned() {
                None => {
                    interaction.respond(&state.client, message(|m| {
                        m.content(format!("{} is not playing Avalon", target.ping_nick()));
                        m.ephemeral();
                    })).await
                }
                Some(target) if target.id() == self.0 => {
                    interaction.respond(&state.client, message(|m| {
                        m.content("You can't use the Lady of the Lake on yourself");
                        m.ephemeral();
                    })).await
                }
                Some(target) => {
                    if let Some(idx) = game.prev_ladies.iter().position(|id| *id == target.id()) {
                        interaction.respond(&state.client, message(|m| {
                            m.content(format!(
                                "You can't use the Lady of the Lake on someone who had the Lady of the \
                                Lake in the past. {} had the Lady of the Lake {}.",
                                target.ping_nick(),
                                match idx {
                                    0 => "first",
                                    1 => "second",
                                    2 => "third? that seems unlikely. plz tell Andrew this happened lol",
                                    _ => unreachable!("harumph"),
                                }
                            ));
                            m.ephemeral();
                        })).await
                    } else {
                        self.0.send_dm(&*state, create_message(|m| {
                            m.content(format!("{} is {}", target.ping_nick(), target.role.loyalty()));
                            m.image(target.role.loyalty().image());
                        })).await?;
                        let target_idx = game.players.iter()
                            .position(|p| p.id() == target.id())
                            .unwrap();
                        let guard = state.commands.read().await;
                        let mut commands = guard.get(&interaction.guild).unwrap()
                            .write().await;
                        delete_command(
                            &*state, interaction.guild, &mut commands,
                            |c| c.is::<LotlCommand>(),
                        ).await?;
                        game.lotl = Some(target_idx);
                        game.prev_ladies.push(self.0);
                        game.round += 1;
                        AvalonGame::next_leader(&mut game.leader, game.players.len());
                        game.start_round(&*state, interaction.guild, &mut commands).await?;
                        interaction.ack_source(&state.client).await
                    }
                }
            }
        } else {
            interaction.respond(&state.client, message(|m| {
                m.content(format!("Only {} can use the Lady of the Lake", self.0.ping_nick()));
                m.ephemeral();
            })).await
        };
        result.map_err(|e| e.into())
    }
}

#[derive(CommandData)]
struct LadyData {
    #[command(desc = "The player whose alignment you want to see.")]
    target: UserId,
}

#[derive(Clone, Debug)]
pub struct ToggleLady;

#[async_trait]
impl SlashCommand<Bot> for ToggleLady {
    fn name(&self) -> &'static str { "lady" }

    fn command(&self) -> Command {
        self.make(
            "Toggle the Lady of the Lake for the next game of Avalon",
            ToggleData::args()
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let mut guard = state.bot.games.write().await;
        let config = guard.get_mut(&interaction.guild).unwrap().config_mut();
        config.lotl = if let Some(enabled) = ToggleData::from_data(data, interaction.guild)?.enabled {
            enabled
        } else {
            !config.lotl
        };
        config.update_embed(&*state, interaction).await.map_err(|e| e.into())
    }
}

#[derive(CommandData)]
struct ToggleData {
    #[command(desc = "Whether or not the Lady of the Lake will be used")]
    enabled: Option<bool>,
}
