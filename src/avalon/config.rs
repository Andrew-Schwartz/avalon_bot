use std::mem;

use itertools::Itertools;
use tokio::sync::RwLockWriteGuard;

use discorsd::{BotState, GuildCommands, http};
use discorsd::http::channel::{embed, MessageChannelExt, RichEmbed};
use discorsd::http::ClientResult;
use discorsd::model::commands::*;
use discorsd::model::guild::GuildMember;
use discorsd::model::ids::*;
use discorsd::model::message::Message;
use discorsd::model::user::UserMarkup;

use crate::{avalon, Bot};
use crate::avalon::characters::Character;
use crate::avalon::characters::Loyalty::Evil;
use crate::avalon::lotl::ToggleLady;
use crate::avalon::roles::RolesCommand;
use crate::avalon::SlashCommandRaw;
use crate::commands::addme::AddMeCommand;
use crate::commands::start::StartCommand;

#[derive(Default, Debug, Clone)]
pub struct AvalonConfig {
    // forwarded to Avalon
    pub players: Vec<GuildMember>,
    pub roles: Vec<Character>,
    pub lotl: bool,

    /// the interaction whose message is being edited to show the game settings
    pub message: Option<Message>,
}

impl AvalonConfig {
    pub fn startable(&self) -> bool {
        let max_evil = self.max_evil();
        let num_evil = self.roles.iter()
            .filter(|r| r.loyalty() == Evil)
            .count();
        self.players.len() >= self.roles.len() &&
            match max_evil {
                None => false,
                Some(max_evil) if num_evil > max_evil => false,
                Some(_) => true,
            }
    }

    pub fn embed(&self) -> RichEmbed {
        embed(|e| {
            e.title("__Avalon Setup__");
            let players_list = self.players.iter()
                .map(UserMarkup::ping)
                .join("\n");
            e.add_inline_field(
                format!("Players ({})", self.players.len()),
                if players_list.is_empty() { "None yet".into() } else { players_list },
            );
            e.add_blank_inline_field();
            let mut roles = self.roles.iter()
                .copied()
                .map(Character::name)
                .join("\n");
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let mut fill = |num_players, max_evil| {
                let num_evil = self.roles.iter()
                    .filter(|c| c.loyalty() == Evil)
                    .count();
                let num_good = self.roles.len() - num_evil;
                let mom = max_evil as i32 - num_evil as i32;
                let ls = num_players as i32 - max_evil as i32 - num_good as i32;
                if ls != 0 {
                    roles.push_str(&format!("\n{}x Loyal Servant", ls));
                }
                if mom != 0 {
                    roles.push_str(&format!("\n{}x Minion of Mordred", mom))
                }
            };
            match self.max_evil() {
                None if self.players.len() < 5 => {
                    // assume that there will be 5 players, so treat max_evil as 2
                    let max_evil = 2;
                    fill(5, max_evil)
                }
                Some(max_evil) => {
                    fill(self.players.len(), max_evil)
                }
                None => {
                    // AvalonError::TooManyPlayers(self.players.len())?
                }
            }
            e.add_inline_field("Roles", roles);
            e.add_inline_field("Lady of the Lake", if self.lotl { "enabled" } else { "disabled" });
        })
    }

    pub async fn update_embed(
        &mut self,
        state: &BotState<Bot>,
        interaction: &InteractionUse<AppCommandData, Deferred>,
    ) -> http::ClientResult<()> {
        let embed = self.embed();
        match &mut self.message {
            Some(message) if message.channel == interaction.channel => {
                // not a followup so it doesn't get deleted
                let new = interaction.channel.send(&state, embed).await?;
                let mut old = mem::replace(message, new);
                old.delete(&state.client).await?;
            }
            Some(_) | None => {
                let new = interaction.channel.send(&state, embed).await?;
                self.message = Some(new);
            }
        };
        Ok(())
    }

    /// Determine if Avalon can be started, and if it can be, include it in the list of games
    /// available in the start command.
    pub async fn start_command(
        &mut self,
        state: &BotState<Bot>,
        mut commands: RwLockWriteGuard<'_, GuildCommands<Bot>>,
        enable: bool,
        guild: GuildId,
    ) -> ClientResult<()> {
        // let (start_id, start_command) = state.get_command_mut::<StartCommand>(guild, &mut commands).await;
        // let edit = if enable {
        //     matches!(start_command.insert(GameType::Avalon), None)
        // } else {
        //     matches!(start_command.remove(GameType::Avalon), Some(_))
        // };
        // if edit {
        //     start_command.edit_command(&state, guild, start_id).await?;
        // }
        Ok(())
    }

    pub fn max_evil(&self) -> Option<usize> {
        avalon::max_evil(self.players.len())
    }

    pub fn is_setup_command(command: &dyn SlashCommandRaw<Bot=Bot>) -> bool {
        command.is::<StartCommand>() ||
            command.is::<AddMeCommand>() ||
            command.is::<RolesCommand>() ||
            command.is::<ToggleLady>()
    }
}
