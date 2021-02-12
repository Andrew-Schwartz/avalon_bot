use std::collections::HashSet;

use itertools::Itertools;
use strum::{EnumCount, IntoEnumIterator};

use crate::avalon::characters::Character;
use crate::Bot;

use super::*;

#[derive(Clone, Debug)]
pub struct RolesCommand(pub Vec<Character>);

#[async_trait]
impl SlashCommand<Bot> for RolesCommand {
    fn name(&self) -> &'static str { "roles" }

    fn command(&self) -> Command {
        let roles: HashSet<Character> = self.0.iter().cloned().collect();
        let make_opts = |first, addl, already_present| {
            let choices = Character::iter()
                .skip(2)
                .filter(|c| roles.contains(c) == already_present)
                .map(Character::name)
                .map(CommandChoice::new_str)
                .collect_vec();
            let num_choices = if already_present {
                roles.len()
            } else {
                Character::COUNT - 2 - roles.len()
            };
            (0..num_choices).map(|i| {
                match i {
                    0 => CommandDataOption::new_str("role1", first).required(),
                    1 => CommandDataOption::new_str("role2", addl),
                    2 => CommandDataOption::new_str("role3", addl),
                    3 => CommandDataOption::new_str("role4", addl),
                    4 => CommandDataOption::new_str("role5", addl),
                    5 => CommandDataOption::new_str("role6", addl),
                    _ => unreachable!("harumph"),
                }
            })
                .map(|opt| opt.choices(choices.clone()))
                .map(DataOption::String)
                .collect()
        };
        let mut commands = Vec::new();
        if roles.len() < Character::COUNT - 2 {
            commands.push(SubCommand {
                name: "add",
                description: "Choose roles to add",
                options: make_opts("The role to add", "Additional role to add", false),
            });
        }
        if !roles.is_empty() {
            commands.push(SubCommand {
                name: "remove",
                description: "Choose roles to remove",
                options: make_opts("The role to remove", "Additional role to remove", true),
            });
            commands.push(SubCommand {
                name: "clear",
                description: "Clear all roles",
                options: Vec::new(),
            });
        }
        self.make(
            "Pick which roles will be available in the next game of Avalon.",
            TopLevelOption::Commands(commands),
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>, BotError> {
        let mut guard = state.bot.games.write().await;
        let game = guard.get_mut(&interaction.guild).unwrap();
        let config = game.config_mut();
        let roles = &mut config.roles;
        let changed = match RoleData::from_data(data, interaction.guild)? {
            RoleData::Add(add) => {
                let new = add.into_iter()
                    .filter(|c| !roles.contains(c))
                    .collect_vec();
                let added = !new.is_empty();
                roles.extend(&new);
                added
            }
            RoleData::Remove(rem) => {
                let mut removed = false;
                roles.retain(|char| {
                    let retain = !rem.contains(char);
                    if !retain { removed = true }
                    retain
                });
                removed
            }
            RoleData::Clear => {
                roles.clear();
                true
            }
        };
        if changed {
            let guard = state.commands.read().await;
            let mut commands = guard.get(&interaction.guild).unwrap().write().await;
            let roles_cmd = commands.get_mut(&interaction.command)
                .unwrap()
                .downcast_mut::<Self>()
                .unwrap();
            roles_cmd.0 = roles.clone();
            state.client.create_guild_command(
                state.application_id().await,
                interaction.guild,
                roles_cmd.command(),
            ).await?;
            config.start_command(&*state, &mut commands, config.startable(), interaction.guild).await?;
        }
        config.update_embed(&*state, interaction).await.map_err(|e| e.into())
    }
}

#[derive(CommandData)]
enum RoleData {
    Add(#[command(vararg = "role", default = "HashSet::new")] HashSet<Character>),
    Remove(#[command(vararg = "role", default = "HashSet::new")] HashSet<Character>),
    Clear,
}