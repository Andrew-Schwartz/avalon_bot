use std::sync::Arc;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::commands::SlashCommandExt;
use discorsd::errors::BotError;
use discorsd::http::ClientResult;
use discorsd::model::ids::*;

use crate::Bot;
use crate::games::GameType;

#[derive(Clone, Debug)]
pub struct AddMeCommand;

#[async_trait]
impl SlashCommand<Bot> for AddMeCommand {
    fn name(&self) -> &'static str { "addme" }

    fn command(&self) -> Command {
        self.make(
            "Add yourself to a game",
            AddMeData::args(),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let data = AddMeData::from_data(data, interaction.guild)?;
        let id = data.player.unwrap_or_else(|| interaction.member.id());
        match data.game {
            GameType::Avalon => avalon(&*state, interaction, id).await,
            _ => {
                interaction.respond_source(&state.client, format!(r#""added" to {:?}"#, data.game)).await
            }
        }.map_err(|e| e.into())
    }
}

#[derive(CommandData)]
struct AddMeData {
    #[command(default, choices, desc = "The game to add you to, or Avalon if not specified")]
    game: GameType,
    #[command(desc = "Forcibly add someone else to the game")]
    player: Option<UserId>,
}

async fn avalon(
    state: &BotState<Bot>,
    interaction: InteractionUse<Unused>,
    user: UserId,
) -> ClientResult<InteractionUse<Used>> {
    let guild = interaction.guild;

    let mut games = state.bot.games.write().await;
    let game = games.entry(guild).or_default();
    let config = game.config_mut();

    {
        let mut users = state.bot.user_games.write().await;
        let guilds = users.entry(user).or_default();

        if config.players.iter().any(|m| m.id() == user) {
            // remove player
            config.players.retain(|m| m.id() != user);
            guilds.remove(&guild);
        } else {
            // add player
            if config.players.len() == 10 {
                return interaction.respond(&state.client, message(|m| {
                    m.content("There can be a maximum of 10 people playing Avalon");
                    m.ephemeral();
                })).await;
            } else {
                if interaction.channel == state.bot.config.channel && user == state.bot.config.owner {
                    for _ in 0..(5_usize.saturating_sub(config.players.len())) {
                        config.players.push(interaction.member.clone());
                    };
                } else {
                    config.players.push(interaction.member.clone());
                }
                guilds.insert(guild);
            }
        }
    }

    let guard = state.commands.read().await;
    let mut commands = guard.get(&interaction.guild).unwrap().write().await;
    config.start_command(state, &mut commands, config.startable(), guild).await?;
    config.update_embed(state, interaction).await
}