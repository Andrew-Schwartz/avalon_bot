use std::collections::HashSet;
use std::time::Instant;

use discorsd::errors::BotError;
use discorsd::http::user::UserExt;
use discorsd::model::message::{ChannelMessageId, Color};
use discorsd::UserMarkupExt;

use crate::{create_command, delete_command};
use crate::avalon::characters::Loyalty;
use crate::commands::stop::StopCommand;
use crate::games::GameType;

use super::*;

#[derive(Clone, Debug)]
pub struct StartCommand(pub HashSet<GameType>);

#[async_trait]
impl SlashCommand<Bot> for StartCommand {
    fn name(&self) -> &'static str { "start" }

    fn command(&self) -> Command {
        let options = if self.0.len() == 1 {
            TopLevelOption::Empty
        } else {
            StartData::args()
        };
        self.make("Starts the game immediately in this channel", options)
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        if !data.options.is_empty() {
            let game = StartData::from_data(data, interaction.guild)?.game;
            todo!("start specific game {:?}", game)
        }
        let used = interaction.ack(&state).await?;
        let mut guard = state.bot.games.write().await;
        let avalon = guard.get_mut(&used.guild).unwrap();
        let game = avalon.start(used.channel);
        state.client.trigger_typing(game.channel).await?;
        let board = game.board_image();
        let AvalonGame { channel, players, lotl, .. } = game.clone();
        let players = Arc::new(players);
        let mut handles = Vec::new();
        for player in Vec::clone(&*players) {
            let state = Arc::clone(&state);
            let players = Arc::clone(&players);
            // task should not panic
            let handle = tokio::spawn(async move {
                let message = player.send_dm(&state, embed(|e| {
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
                    let image = player.role.image();
                    e.image(image);
                })).await?;
                if let Err(e) = message.pin(&state).await {
                    warn!("Failed to pin character: {}", e.display_error(&state).await);
                }
                Ok(ChannelMessageId::from(message))
            });
            handles.push(handle);
        }
        let pinned = futures::future::join_all(handles).await.into_iter()
            .map(|res| res.expect("character info tasks do not panic"))
            .collect::<ClientResult<HashSet<_>>>()?;
        game.pins.extend(pinned);

        let start = Instant::now();
        channel.send(&state, embed(|e| {
            e.title(format!("Avalon game with {} players", players.len()));
            e.color(Color::GOLD);
            e.add_inline_field(
                "Order of Leaders",
                players.iter()
                    .enumerate()
                    .map(|(i, player)| if i == 0 {
                        format!("{} - goes first", player.ping_nick())
                    } else if lotl.filter(|lotl| *lotl == i).is_some() {
                        format!("{} - has the Lady of the Lake", player.ping_nick())
                    } else {
                        player.ping_nick()
                    })
                    .join("\n"),
            );
            e.add_blank_inline_field();
            let (mut good, mut evil): (Vec<_>, _) = players.iter()
                .map(|p| p.role)
                .filter(|c| !matches!(c, LoyalServant | MinionOfMordred))
                .partition(|c| c.loyalty() == Loyalty::Good);
            good.sort_by_key(|c| c.name());
            evil.sort_by_key(|c| c.name());
            let (n_good, n_evil) = (good.len(), evil.len());
            let max_evil = max_evil(players.len()).unwrap();
            let max_good = players.len() - max_evil;
            let mut roles = good.into_iter().map(Character::name).join("\n");
            let ls = max_good - n_good;
            if ls != 0 {
                if n_good != 0 { roles.push('\n') }
                roles.push_str(&format!("{}x {}", ls, LoyalServant));
            }
            roles.push('\n');
            roles.push_str(&evil.into_iter().map(Character::name).join("\n"));
            let mom = max_evil - n_evil;
            if mom != 0 {
                if n_evil != 0 { roles.push('\n') }
                roles.push_str(&format!("{}x {}", mom, MinionOfMordred));
            }
            e.add_inline_field("The roles are", roles);
            if let Some(idx) = lotl {
                e.footer(
                    format!("{} has the Lady of the Lake", players[idx].member.nick_or_name()),
                    "images/avalon/lotl.jpg",
                );
            }
            e.image(board);
        })).await?;
        println!("message = {:?}", start.elapsed());

        let commands = state.commands.read().await;
        let mut commands = commands.get(&used.guild).unwrap().write().await;
        create_command(&*state, used.guild, &mut commands, StopCommand(GameType::Avalon)).await?;
        delete_command(
            &*state, used.guild,
            &mut commands,
            AvalonConfig::is_setup_command,
        ).await?;
        game.start_round(
            &*state,
            used.guild,
            &mut commands,
        ).await?;

        Ok(used)
    }
}

#[derive(CommandData)]
struct StartData {
    #[command(choices, desc = "Choose the game to start")]
    game: GameType,
}