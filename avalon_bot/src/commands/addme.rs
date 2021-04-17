use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::ClientResult;
use discorsd::model::ids::*;

use crate::Bot;
use crate::games::GameType;

#[derive(Clone, Debug)]
pub struct AddMeCommand;

#[async_trait]
impl SlashCommand for AddMeCommand {
    type Bot = Bot;
    type Data = AddMeData;
    type Use = Used;
    const NAME: &'static str = "addme";

    fn description(&self) -> Cow<'static, str> {
        "Add yourself to a game".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: AddMeData,
    ) -> Result<InteractionUse<Self::Use>, BotError> {
        let id = data.player.unwrap_or_else(|| interaction.user().id());
        match data.game {
            GameType::Avalon => avalon(&*state, interaction, id).await,
            GameType::Hangman => hangman(&state, interaction, id).await,
            GameType::Kittens => {
                interaction.respond(&state.client, format!(r#""added" to {:?}"#, data.game)).await
            }
        }.map_err(|e| e.into())
    }
}

// fixme: doesn't work if just `player` is given
#[derive(CommandData)]
pub struct AddMeData {
    #[command(default, desc = "The game to add you to, or Avalon if not specified")]
    game: GameType,
    #[command(desc = "Forcibly add someone else to the game")]
    player: Option<UserId>,
}

async fn avalon(
    state: &BotState<Bot>,
    interaction: InteractionUse<Unused>,
    user: UserId,
) -> ClientResult<InteractionUse<Used>> {
    let guild = interaction.guild().unwrap();

    let mut games = state.bot.avalon_games.write().await;
    let game = games.entry(guild).or_default();
    let config = game.config_mut();

    // track which guilds this user is in a game in
    let deferred = {
        let mut users = state.bot.user_games.write().await;
        let guilds = users.entry(user).or_default();

        if config.players.iter().any(|m| m.id() == user) {
            // remove player
            config.players.retain(|m| m.id() != user);
            guilds.remove(&guild);
            interaction.defer(&state).await?
        } else {
            // add player
            if config.players.len() == 10 {
                return interaction.respond(&state.client, message(|m| {
                    m.content("There can be a maximum of 10 people playing Avalon");
                    m.ephemeral();
                })).await;
            }
            if interaction.channel == state.bot.config.channel && user == state.bot.config.owner {
                for _ in 0..(5_usize.saturating_sub(config.players.len())) {
                    config.players.push(interaction.member().unwrap().clone());
                };
            } else if let Some(member) = state.cache.member(guild, user).await {
                config.players.push(member);
            } else if let Ok(member) = state.cache_guild_member(guild, user).await {
                config.players.push(member);
            } else {
                return interaction.respond(&state, message(|m| {
                    m.content("Could not find that user in this guild!");
                    m.ephemeral();
                })).await;
            }
            guilds.insert(guild);
            interaction.defer(&state).await?
        }
    };

    let guard = state.commands.read().await;
    let commands = guard.get(&guild).unwrap().write().await;
    config.start_command(state, commands, config.startable(), guild).await?;
    config.update_embed(state, &deferred).await?;
    deferred.delete(&state).await
}

async fn hangman(
    state: &BotState<Bot>,
    interaction: InteractionUse<Unused>,
    user: UserId,
) -> ClientResult<InteractionUse<Used>> {
    let guild = interaction.guild().unwrap();

    let mut games = state.bot.hangman_games.write().await;
    let game = games.entry(guild).or_default();
    let config = game.config_mut();

    // track which guilds this user is in a game in
    {
        let mut users = state.bot.user_games.write().await;
        let guilds = users.entry(user).or_default();

        if config.players.iter().any(|u| u == &user) {
            // remove player
            config.players.retain(|&u| u != user);
            guilds.remove(&guild);
        } else {
            // add player
            config.players.push(user);
            guilds.insert(guild);
        }
    }

    config.update_embed(state, interaction).await
}