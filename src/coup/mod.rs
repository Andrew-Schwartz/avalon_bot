use std::mem;

use discorsd::BotState;
use discorsd::commands::{Deferred, InteractionUse, SlashCommandData};
use discorsd::http::channel::{embed, MessageChannelExt};
use discorsd::http::ClientResult;
use discorsd::model::guild::GuildMember;
use discorsd::model::message::Message;
use discorsd::model::user::UserMarkupExt;
use itertools::Itertools;

use crate::Bot;

#[derive(Debug)]
pub enum Coup {
    Config(CoupConfig),
    Game(CoupGame),
}

impl Default for Coup {
    fn default() -> Self {
        Self::Config(Default::default())
    }
}

#[derive(Debug, Default)]
pub struct CoupConfig {
    pub players: Vec<GuildMember>,
    pub settings_display: Option<Message>,
}

impl CoupConfig {
    pub async fn update_settings_embed(
        &mut self,
        state: &BotState<Bot>,
        interaction: &InteractionUse<SlashCommandData, Deferred>,
    ) -> ClientResult<()> {
        let embed = embed(|e| {
            e.title("__Coup Setup__");
            let players_list = self.players.iter()
                .map(UserMarkupExt::ping_nick)
                .join("\n");
            e.add_field(
                format!("Players ({})", self.players.len()),
                if players_list.is_empty() {
                    "None yet".into()
                } else {
                    players_list
                },
            )
        });
        match &mut self.settings_display {
            Some(message) if message.channel == interaction.channel => {
                // not a followup so it doesn't get deleted
                let new = interaction.channel.send(&state, embed).await?;
                let old = mem::replace(message, new);
                old.delete(&state.client).await?;
            }
            Some(_) | None => {
                let new = interaction.channel.send(&state, embed).await?;
                // self.message = Some(new);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CoupGame {
    pub players: Vec<CoupPlayer>,
}

#[derive(Debug)]
pub struct CoupPlayer {
    pub member: GuildMember,
    pub money: usize,
    pub roles: [Card; 2],
}

#[derive(Debug)]
pub enum Card {
    Duke,
    Assassin,
    Ambassador,
    Captain,
    Contessa,
}