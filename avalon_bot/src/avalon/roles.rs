use std::collections::HashSet;

use itertools::Itertools;
use strum::{EnumCount, IntoEnumIterator};

use discorsd::http::model::interaction::*;

use crate::Bot;
use crate::avalon::characters::Character;
use crate::commands::{InteractionUse, NotUsed, SlashCommand, Used};

use super::*;

#[derive(Clone, Debug)]
pub struct RolesCommand(pub Vec<Character>);

#[async_trait]
impl SlashCommand for RolesCommand {
    fn name(&self) -> &'static str { "roles" }

    fn command(&self) -> Command {
        let roles: HashSet<Character> = self.0.iter().cloned().collect();
        let make_opts = |first, addl, already_present| {
            let choices = Character::iter()
                .skip(2)
                .filter(|c| roles.contains(c) == already_present)
                .map(|c| c.name())
                .map(|name| CommandChoice::new_str(name))
                .collect_vec();
            let num_choices = if already_present {
                roles.len()
            } else {
                Character::COUNT - 2 - roles.len()
            };
            (0..num_choices).map(|i| {
                match i {
                    0 => CommandDataOption::new_str("role", first).required(),
                    1 => CommandDataOption::new_str("role1", addl),
                    2 => CommandDataOption::new_str("role2", addl),
                    3 => CommandDataOption::new_str("role3", addl),
                    4 => CommandDataOption::new_str("role4", addl),
                    5 => CommandDataOption::new_str("role5", addl),
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
            TopLevelOption::Commands(commands)
        )
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<NotUsed>,
                 data: ApplicationCommandInteractionData,
    ) -> Result<InteractionUse<Used>> {
        // let InteractionUse { command, guild, .. } = interaction;
        let mut guard = state.bot.games.write().await;
        let game = guard.get_mut(&interaction.guild).unwrap();
        let config = game.config_mut();
        let roles = &mut config.roles;
        let changed = match RoleData::from(data) {
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
            let guard = state.bot.commands.read().await;
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

#[derive(Debug)]
enum RoleData {
    Add(HashSet<Character>),
    Remove(HashSet<Character>),
    Clear,
}

impl From<ApplicationCommandInteractionData> for RoleData {
    fn from(mut data: ApplicationCommandInteractionData) -> Self {
        let ApplicationCommandInteractionDataOption {
            name,
            value: _value,
            options
        } = data.options.remove(0);
        let get_roles = || options.into_iter()
            .map(|data| data.value
                .unwrap()
                .unwrap_string()
                .parse()
                .unwrap()
            ).collect();
        match name.as_str() {
            "clear" => Self::Clear,
            "add" => Self::Add(get_roles()),
            "remove" => Self::Remove(get_roles()),
            bad => unreachable!("should not receive {}", bad)
        }
    }
}