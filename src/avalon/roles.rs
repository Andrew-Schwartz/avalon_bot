use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use itertools::Itertools;
use strum::EnumCount;

use command_data_derive::CommandData;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;

use crate::avalon::characters::Character;
use crate::avalon::characters::Character::{LoyalServant, MinionOfMordred};
use crate::Bot;

#[derive(Clone, Debug)]
pub struct RolesCommand(pub Vec<Character>);

#[async_trait]
impl SlashCommand for RolesCommand {
    type Bot = Bot;
    type Data = RoleData;
    type Use = Deferred;
    const NAME: &'static str = "roles";

    fn description(&self) -> Cow<'static, str> {
        "Pick which roles will be available in the next game of Avalon.".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 data: RoleData,
    ) -> Result<InteractionUse<SlashCommandData, Self::Use>, BotError> {
        let interaction = interaction.defer(&state).await?;
        let guild = interaction.guild().unwrap();
        let mut guard = state.bot.avalon_games.write().await;
        let game = guard.get_mut(&guild).unwrap();
        let config = game.config_mut();
        let roles = &mut config.roles;
        let changed = match data {
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
            let mut commands = guard.get(&guild).unwrap().write().await;
            let roles_cmd = commands.get_mut(&interaction.data.command)
                .unwrap()
                .downcast_mut::<Self>()
                .unwrap();
            roles_cmd.0 = roles.clone();
            roles_cmd.edit_command(&state, guild, interaction.data.command).await?;
            config.start_command(&state, commands, config.startable(), guild).await?;
        }
        config.update_embed(&state, &interaction).await?;
        Ok(interaction)
    }
}

#[derive(CommandData)]
#[command(command = "RolesCommand")]
pub enum RoleData {
    #[command(desc = "Choose roles to add", enable_if = "add_roles")]
    Add(
        #[command(va_ordinals, va_count = "add_count", va_req = 1, retain = "add_choice")]
        HashSet<Character>
    ),
    #[command(desc = "Choose roles to remove", enable_if = "remove_roles")]
    Remove(
        #[command(va_ordinals, va_count = "remove_count", va_req = 1, retain = "remove_choice")]
        HashSet<Character>
    ),
    #[command(desc = "Clear all roles", enable_if = "remove_roles")]
    Clear,
}

fn add_count(command: &RolesCommand) -> usize {
    Character::COUNT - 2 - command.0.len()
}

fn add_choice(command: &RolesCommand, choice: Character) -> bool {
    choice != LoyalServant &&
        choice != MinionOfMordred &&
        !command.0.iter().any(|&c| choice == c)
}

fn remove_count(command: &RolesCommand) -> usize {
    command.0.len()
}

fn remove_choice(command: &RolesCommand, choice: Character) -> bool {
    command.0.iter().any(|&c| choice == c)
}

fn add_roles(command: &RolesCommand) -> bool {
    command.0.len() < Character::COUNT - 2
}

fn remove_roles(command: &RolesCommand) -> bool {
    !command.0.is_empty()
}