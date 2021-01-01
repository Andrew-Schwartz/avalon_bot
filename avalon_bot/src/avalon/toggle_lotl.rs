use super::*;

#[derive(Clone, Debug)]
pub struct ToggleLady;

#[async_trait]
impl SlashCommand for ToggleLady {
    fn name(&self) -> &'static str { "lady" }

    fn command(&self) -> Command {
        self.make(
            "Toggle the Lady of the Lake for the next game of Avalon",
            TopLevelOption::Data(vec![DataOption::Boolean(
                CommandDataOption::new("enabled", "Whether or not the Lady of the Lake will be used")
            )]),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 mut data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>> {
        let mut guard = state.bot.games.write().await;
        let config = guard.get_mut(&interaction.guild).unwrap().config_mut();
        config.lotl = if data.options.is_empty() {
            !config.lotl
        } else {
            data.options.remove(0).value.unwrap().unwrap_bool()
        };
        config.update_embed(&*state, interaction).await.map_err(|e| e.into())
    }
}