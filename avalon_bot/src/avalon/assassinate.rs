use super::*;
use discorsd::model::message::Color;

#[derive(Clone, Debug)]
pub struct Assassinate(pub UserId);

#[async_trait]
impl SlashCommand<Bot> for Assassinate {
    fn name(&self) -> &'static str { "assassinate" }

    fn command(&self) -> Command {
        self.make(
            "Assassinate who you think is Merlin",
            AssassinateData::args()
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let result = if interaction.member.id() == self.0 {
            let target = AssassinateData::from_data(data, interaction.guild)?.target;
            let mut guard = state.bot.games.write().await;
            let avalon = guard.get_mut(&interaction.guild).unwrap();
            let game = avalon.game_mut();
            match game.player_ref(target) {
                None => {
                    interaction.respond(&state.client, message(|m| {
                        m.content(format!("{} is not playing Avalon", target.ping_nick()));
                        m.ephemeral();
                    })).await
                }
                Some(evil) if evil.role.loyalty() == Evil => {
                    interaction.respond(&state.client, message(|m| {
                        m.content(format!("{} is evil, you can't assassinate them!", target.ping_nick()));
                        m.ephemeral();
                    })).await
                }
                Some(guess) => {
                    let interaction = interaction.ack_source(&state.client).await?;
                    let game_over = embed(|e| {
                        if guess.role == Merlin {
                            e.color(Color::RED);
                            e.title(format!("Correct! {} was Merlin! The bad guys win!", guess.member.nick_or_name()));
                        } else {
                            let merlin = game.players.iter().find(|p| p.role == Merlin).unwrap();
                            e.color(Color::BLUE);
                            e.title(format!(
                                "Incorrect! {} was actually {}, and {} was Merlin! The good guys win!",
                                guess.member.nick_or_name(),
                                guess.role,
                                merlin.member.nick_or_name(),
                            ))
                        }
                    });
                    let guard = state.commands.read().await;
                    let mut commands = guard.get(&interaction.guild).unwrap()
                        .write().await;
                    avalon.game_over(&*state, interaction.guild, &mut commands, game_over).await?;
                    Ok(interaction)
                }
            }
        } else {
            interaction.respond(&state.client, message(|m| {
                m.content(format!("Only the assassin ({}) can assassinate someone", self.0.ping_nick()));
                m.ephemeral();
            })).await
        };
        result.map_err(|e| e.into())
    }
}

#[derive(CommandData)]
struct AssassinateData {
    #[command(desc = "Your guess of who is Merlin")]
    target: UserId,
}