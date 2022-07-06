use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::create_message;
use discorsd::http::guild::CommandPermsExt;
use discorsd::http::user::UserExt;
use discorsd::model::ids::{Id, UserId};
use discorsd::model::user::UserMarkupExt;

use crate::avalon::game::AvalonGame;
use crate::Bot;

#[derive(Clone, Debug)]
pub struct LotlCommand(pub UserId);

#[allow(clippy::use_self)]
#[async_trait]
impl SlashCommand for LotlCommand {
    type Bot = Bot;
    type Data = LadyData;
    type Use = Used;
    const NAME: &'static str = "lotl";

    fn description(&self) -> Cow<'static, str> {
        "Learn a player's true alignment".into()
    }

    fn default_permissions(&self) -> bool {
        false
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 data: LadyData,
    ) -> Result<InteractionUse<SlashCommandData, Used>, BotError> {
        let result = if interaction.user().id == self.0 {
            let guild = interaction.guild().unwrap();
            let target = data.target;
            let mut guard = state.bot.avalon_games.write().await;
            let game = guard.get_mut(&guild).unwrap().game_mut();
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
                        state.command_id::<LotlCommand>(guild).await
                            .disallow_users(&state, guild, &[self.0]).await?;

                        game.lotl = Some(target_idx);
                        game.prev_ladies.push(self.0);
                        game.round += 1;
                        AvalonGame::advance_leader(&mut game.leader, game.players.len());

                        let guard = state.commands.read().await;
                        let commands = guard.get(&guild).unwrap()
                            .write().await;
                        game.start_round(&*state, guild, commands).await?;
                        interaction.delete(&state).await
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

#[derive(CommandData, Debug)]
pub struct LadyData {
    #[command(desc = "The player whose alignment you want to see.")]
    target: UserId,
}

#[derive(Clone, Debug)]
pub struct ToggleLady;

#[async_trait]
impl SlashCommand for ToggleLady {
    type Bot = Bot;
    type Data = ToggleData;
    type Use = Deferred;
    const NAME: &'static str = "lady";

    fn description(&self) -> Cow<'static, str> {
        "Toggle the Lady of the Lake for the next game of Avalon".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 data: ToggleData,
    ) -> Result<InteractionUse<SlashCommandData, Self::Use>, BotError> {
        let interaction = interaction.defer(&state).await?;
        let mut guard = state.bot.avalon_games.write().await;
        let guild = interaction.guild().unwrap();
        let config = guard.get_mut(&guild).unwrap().config_mut();
        config.lotl = if let Some(enabled) = data.enabled {
            enabled
        } else {
            !config.lotl
        };
        config.update_embed(&*state, &interaction).await?;
        Ok(interaction)
    }
}

#[derive(CommandData)]
pub struct ToggleData {
    #[command(desc = "Whether or not the Lady of the Lake will be used")]
    enabled: Option<bool>,
}
