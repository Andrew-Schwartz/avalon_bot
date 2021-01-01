use std::collections::HashSet;

use discorsd::http::model::Color;
use discorsd::http::user::UserExt;
use discorsd::UserMarkupExt;

use crate::{create_command, delete_command};
use crate::avalon::characters::Loyalty;
use crate::commands::GameType;
use crate::commands::stop::StopCommand;

use super::*;

#[derive(Clone, Debug)]
pub struct StartCommand(pub HashSet<GameType>);

#[async_trait]
impl SlashCommand for StartCommand {
    fn name(&self) -> &'static str { "start" }

    fn command(&self) -> Command {
        let options = if self.0.len() == 1 {
            TopLevelOption::Empty
        } else {
            TopLevelOption::Data(vec![DataOption::String(CommandDataOption::new_str(
                "game", "Choose the game to start",
            ).required().choices(
                self.0.iter()
                    .map(GameType::name)
                    .map(CommandChoice::new_str)
                    .collect())
            )])
        };
        self.make("Starts the game immediately in this channel", options)
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>> {
        if !data.options.is_empty() {
            todo!("handle starting specific game")
        }
        delete_command(
            &*state, interaction.guild,
            &mut *state.bot.commands.read().await
                .get(&interaction.guild).unwrap()
                .write().await,
            AvalonConfig::is_setup_command,
        ).await?;
        let used = interaction.ack(&state).await?;
        let game = {
            let mut guard = state.bot.games.write().await;
            let game = guard.get_mut(&used.guild).unwrap();
            game.start(used.channel).clone()
        };
        state.client.trigger_typing(game.channel).await?;
        let AvalonGame { channel, players, rounds, lotl, .. } = game;
        let players = Arc::new(players);
        for player in Vec::clone(&*players) {
            let state = Arc::clone(&state);
            let players = Arc::clone(&players);
            tokio::spawn(async move {
                player.send_dm(&*state, embed(|e| {
                    let character = player.role;
                    e.title(character.name());
                    e.description(character.abilities());
                    e.color(character.loyalty().color());
                    let seen_characters = character.sees();
                    if !seen_characters.is_empty() {
                        let sees = seen_characters.iter()
                            .map(|c| c.name())
                            .join("\n");
                        e.add_inline_field("You can see", sees);
                    }
                    let seen_players = players.iter()
                        .filter(|player| seen_characters.contains(&player.role))
                        .cloned()
                        .collect_vec();
                    if !seen_players.is_empty() {
                        e.add_inline_field(
                            "You see",
                            seen_players.iter()
                                .filter(|other| state.bot.config.channel == channel || other.member.id() != player.member.id())
                                .map(|player| player.member.ping_nick())
                                .join("\n"),
                        );
                    }
                    e.image(player.role.image());
                })).await.unwrap();
            });
        }

        channel.send(&state, embed(|e| {
            e.title(format!("Avalon game with {} players", players.len()));
            e.color(Color::GOLD);
            e.add_inline_field(
                "Order of Leaders",
                players.iter()
                    .enumerate()
                    .map(|(i, player)| if i == 0 {
                        format!("{} - goes first", player.ping_nick())
                    } else { player.ping_nick() })
                    .join("\n"),
            );
            e.add_blank_inline_field();
            let (mut good, mut evil): (Vec<_>, _) = players.iter()
                .map(|p| p.role)
                .filter(|c| !matches!(c, LoyalServant | MinionOfMordred))
                .partition(|c| c.loyalty() == Loyalty::Good);
            good.sort_by_key(Character::name);
            evil.sort_by_key(Character::name);
            let (n_good, n_evil) = (good.len(), evil.len());
            let max_evil = max_evil(players.len()).unwrap();
            let max_good = players.len() - max_evil;
            let mut roles = good.into_iter().map(|c| c.name()).join("\n");
            let ls = max_good - n_good;
            if ls != 0 {
                if n_good != 0 { roles.push('\n') }
                roles.push_str(&format!("{}x {}", ls, LoyalServant));
            }
            roles.push('\n');
            roles.push_str(&evil.into_iter().map(|c| c.name()).join("\n"));
            let mom = max_evil - n_evil;
            if mom != 0 {
                if n_evil != 0 { roles.push('\n') }
                roles.push_str(&format!("{}x {}", mom, MinionOfMordred));
            }
            e.add_inline_field("The roles are", roles);
            e.add_inline_field("Rounds", rounds);
            if let Some(idx) = lotl {
                e.add_blank_inline_field();
                e.add_inline_field("Lady of the Lake", players[idx].ping_nick());
            }
        })).await?;

        let commands = state.bot.commands.read().await;
        let mut commands = commands.get(&used.guild).unwrap().write().await;
        let mut guard = state.bot.games.write().await;
        let game = guard.get_mut(&used.guild).unwrap().game_mut();
        create_command(&*state, used.guild, &mut commands, StopCommand(GameType::Avalon)).await?;
        game.start_round(
            &*state,
            used.guild,
            &mut commands,
        ).await?;

        Ok(used)
    }
}