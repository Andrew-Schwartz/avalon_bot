use std::sync::Arc;

use discorsd::{anyhow::Result, async_trait, BotState};
use discorsd::http::ClientResult;
use discorsd::http::model::Id;
use discorsd::http::model::interaction::{self, *};

use crate::Bot;
use crate::commands::{InteractionUse, NotUsed, SlashCommand, SlashCommandExt, Used};

use super::GameType;

#[derive(Clone, Debug)]
pub struct AddMeCommand;

#[async_trait]
impl SlashCommand for AddMeCommand {
    fn name(&self) -> &'static str { "addme" }

    fn command(&self) -> Command {
        self.make(
            "Add yourself to a game",
            TopLevelOption::Data(vec![
                DataOption::String(CommandDataOption::new_str(
                    "game",
                    "The game to add you to, or Avalon if not specified",
                ).choices(vec![
                    CommandChoice::new_str("Avalon"),
                    CommandChoice::new_str("Hangman"),
                    CommandChoice::new("Exploding Kittens", "Kittens"),
                ]))
            ]),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>> {
        // doesn't work if its just GameType, have to figure out better way to debug
        let game = GameType::from(data);
        match game {
            GameType::Avalon => avalon(&*state, interaction).await,
            _ => {
                interaction.respond(
                    &state.client,
                    interaction::message(|m|
                        m.content(format!(r#""added" to {:?}"#, game))
                    ).with_source(),
                ).await
            }
        }.map_err(|e| e.into())
    }
}

async fn avalon(
    state: &BotState<Bot>,
    interaction: InteractionUse<NotUsed>,
) -> ClientResult<InteractionUse<Used>> {
    let guild = interaction.guild;
    let user = interaction.member.id();

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
                return interaction.respond(&state.client, interaction::message(|m| {
                    m.content("There can be a maximum of 10 people playing Avalon");
                    m.ephemeral();
                }).without_source()).await;
            } else {
                if interaction.channel == state.bot.config.channel && user == state.bot.config.owner {
                    for _ in 0..(5usize.saturating_sub(config.players.len())) {
                        config.players.push(interaction.member.clone());
                    };
                } else {
                    config.players.push(interaction.member.clone());
                }
                guilds.insert(guild);
            }
        }
    }

    let guard = state.bot.commands.read().await;
    let mut commands = guard.get(&interaction.guild).unwrap().write().await;
    config.start_command(state, &mut commands, config.startable(), guild).await?;
    config.update_embed(state, interaction).await
}