use std::collections::HashMap;
use std::mem;

use itertools::Itertools;

use discorsd::{BotState, http, UserMarkupExt};
use discorsd::http::channel::{ChannelExt, embed, RichEmbed};
use discorsd::http::ClientResult;
use discorsd::http::model::{CommandId, GuildId, GuildMember, Message};

use crate::{avalon, Bot};
use crate::avalon::characters::Character;
use crate::avalon::characters::Loyalty::Evil;
use crate::avalon::roles::RolesCommand;
use crate::avalon::SlashCommand;
use crate::avalon::start::StartCommand;
use crate::avalon::toggle_lotl::ToggleLady;
use crate::commands::{AddMeCommand, GameType, InteractionUse, NotUsed, Used};

#[derive(Default, Debug)]
pub struct AvalonConfig {
    // forwarded to Avalon
    pub players: Vec<GuildMember>,
    pub roles: Vec<Character>,
    pub lotl: bool,

    /// the interaction whose message is being edited to show the game settings
    pub message: Option<Message>,
    pub start_id: Option<CommandId>,
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

    fn embed(&self) -> RichEmbed {
        embed(|e| {
            e.title("__Avalon Setup__");
            let players_list = self.players.iter()
                .map(UserMarkupExt::ping_nick)
                .join("\n");
            e.add_inline_field(
                format!("Players ({})", self.players.len()),
                if players_list.is_empty() { "None yet".into() } else { players_list },
            );
            e.add_blank_inline_field();
            let mut roles = self.roles.iter()
                .map(Character::name)
                .join("\n");
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
                    // unreachable to have more than 10 players
                }
            }
            e.add_inline_field("Roles", roles);
            e.add_inline_field("Lady of the Lake", if self.lotl { "enabled" } else { "disabled" });
        })
    }

    pub async fn update_embed(
        &mut self,
        state: &BotState<Bot>,
        interaction: InteractionUse<NotUsed>,
    ) -> http::ClientResult<InteractionUse<Used>> {
        let embed = self.embed();
        match &mut self.message {
            Some(message) if message.channel_id == interaction.channel => {
                let is_last_message = state.cache.channel(interaction.channel).await
                    .and_then(|c| c.last_message_id.map(|id| id == message.id))
                    .unwrap_or(false);
                if is_last_message {
                    message.edit(&state.client, embed).await?;
                } else {
                    let new = interaction.channel.send(&state, embed).await?;
                    let old = mem::replace(message, new);
                    old.delete(&state.client).await?;
                }
            }
            Some(_) | None => {
                let new = interaction.channel.send(&state, embed).await?;
                self.message = Some(new);
            }
        };
        interaction.ack(state).await
    }

    pub async fn start_command(
        &mut self,
        state: &BotState<Bot>,
        commands: &mut HashMap<CommandId, Box<dyn SlashCommand>>,
        enabled: bool,
        guild: GuildId,
    ) -> ClientResult<()> {
        let start = self.start_id
            .and_then(|id| {
                commands.get_mut(&id)
                    .map(|s| s.downcast_mut::<StartCommand>().unwrap())
                    .map(|s| (id, s))
            });
        match (start, enabled) {
            // update list of startable games
            (Some((_id, start)), true) => {
                if !start.0.contains(&GameType::Avalon) {
                    start.0.insert(GameType::Avalon);
                    // todo is this fixed?
                    state.client.edit_guild_command(
                        state.application_id().await,
                        guild,
                        _id,
                        None,
                        None,
                        Some(start.command().options())
                    ).await.unwrap();
                    // state.client.create_guild_command(
                    //     state.application_id().await,
                    //     guild,
                    //     start.command(),
                    // ).await?;
                }
            }
            // disable StartCommand
            (Some((id, _)), false) => {
                state.client.delete_guild_command(
                    state.application_id().await,
                    guild,
                    id,
                ).await?;
                commands.remove(&id);
            }
            // enable StartCommand
            (None, true) => {
                let start = StartCommand(set!(GameType::Avalon));
                let command = state.client.create_guild_command(
                    state.application_id().await,
                    guild,
                    start.command(),
                ).await?;
                self.start_id = Some(command.id);
                commands.insert(command.id, Box::new(start));
            }
            // is (and should be) disabled :)
            (None, false) => {}
        };
        Ok(())
    }

    pub fn max_evil(&self) -> Option<usize> {
        avalon::max_evil(self.players.len())
    }

    pub fn is_setup_command(command: &dyn SlashCommand) -> bool {
        command.is::<StartCommand>() ||
            command.is::<AddMeCommand>() ||
            command.is::<RolesCommand>() ||
            command.is::<ToggleLady>()
    }
}
