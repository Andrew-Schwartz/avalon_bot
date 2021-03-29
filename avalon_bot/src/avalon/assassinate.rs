use std::borrow::Cow;

use discorsd::model::message::Color;

use super::*;

#[derive(Clone, Debug)]
pub struct Assassinate(pub UserId);

#[async_trait]
impl SlashCommandData for Assassinate {
    type Bot = Bot;
    type Data = AssassinateData;
    type Use = Used;
    const NAME: &'static str = "assassinate";

    fn description(&self) -> Cow<'static, str> {
        "Assassinate who you think is Merlin".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: AssassinateData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let result = if interaction.user().id == self.0 {
            let target = data.target;
            let guild = interaction.guild().unwrap();
            let mut guard = state.bot.avalon_games.write().await;
            let avalon = guard.get_mut(&guild).unwrap();
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
                    let interaction = interaction.delete(&state).await?;
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
                    let commands = guard.get(&guild).unwrap()
                        .write().await;
                    avalon.game_over(&*state, guild, commands, game_over).await?;
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
pub struct AssassinateData {
    #[command(desc = "Your guess of who is Merlin")]
    target: UserId,
}