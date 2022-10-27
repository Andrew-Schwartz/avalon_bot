use std::borrow::Cow;
use std::sync::Arc;

use command_data_derive::MenuCommand;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::{create_message, MessageChannelExt};

use crate::avalon::characters::Character;
use crate::avalon::config::AvalonConfig;
use crate::Bot;

#[derive(Debug, Clone, Copy)]
pub struct SetupCommand;

// fn message(config: &AvalonConfig) -> CreateMessage {
//     config.embed().into()
//     // create_message(|m| {
//     //     let players_list = if config.players.is_empty() {
//     //         "None".to_string()
//     //     } else {
//     //         config.players.iter().list_grammatically(|u| u.ping_nick(), "and")
//     //     };
//     //     // todo: list number of MoM/LS
//     //     let roles_list = if config.roles.is_empty() {
//     //         "None".to_string()
//     //     } else {
//     //         config.roles.iter().list_grammatically(|c| c.name().to_string(), "and")
//     //     };
//     //     let content = format!(
//     //         "**Avalon Setup**\n\
//     //               Players: {}\n\
//     //               Roles: {}",
//     //         players_list,
//     //         roles_list,
//     //     );
//     //     m.content(content);
//     // })
// }

#[async_trait]
impl SlashCommand for SetupCommand {
    type Bot = Bot;
    type Data = ();
    type Use = Deferred;
    const NAME: &'static str = "setup";

    fn description(&self) -> Cow<'static, str> {
        "setup a game of Avalon".into()
    }

    async fn run(&self,
                 state: Arc<BotState<<Self as SlashCommand>::Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 (): Self::Data,
    ) -> Result<InteractionUse<SlashCommandData, Self::Use>, BotError> {
        interaction.channel.send(&state, create_message(|m| {
            m.content("config");
            m.button(&state, JoinButton::default());
            m.menu(&state, RolesMenu::default());
        })).await?;
        let interaction = interaction.defer(&state).await?;
        let mut config = AvalonConfig::default();
        config.update_embed(&state, &interaction).await?;
        Ok(interaction)
    }
}

#[derive(Clone, Default)]
struct JoinButton(AvalonConfig);

#[async_trait]
impl ButtonCommand for JoinButton {
    type Bot = Bot;

    fn label(&self) -> String {
        "join/leave game".into()
    }

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError> {
        if let InteractionSource::Guild(GuildSource { id: _id, member }) = &interaction.source {
            {
                let mut guard = state.buttons.write().unwrap();
                let config = &mut guard
                    .get_mut(&interaction.data.custom_id)
                    .unwrap()
                    .downcast_mut::<Self>()
                    .unwrap()
                    .0;
                config.players.push(member.clone());
                // config.update_embed(&state, )
                todo!()
            }
            interaction.defer_update(&state).await.map_err(|e| e.into())
        } else {
            unreachable!("should not have /setup in dms")
        }
    }
}

#[derive(Clone, Default)]
struct RolesMenu(AvalonConfig);

#[derive(MenuCommand, Debug, Copy, Clone)]
enum Role {
    Assassin,
    Merlin,
    Mordred,
    Morgana,
    Oberon,
    Percival,
}

impl From<&'_ Role> for Character {
    fn from(role: &'_ Role) -> Self {
        match role {
            Role::Assassin => Self::Assassin,
            Role::Merlin => Self::Merlin,
            Role::Mordred => Self::Mordred,
            Role::Morgana => Self::Morgana,
            Role::Oberon => Self::Oberon,
            Role::Percival => Self::Percival,
        }
    }
}

#[async_trait]
impl MenuCommand for RolesMenu {
    type Bot = Bot;
    type Data = Role;

    fn num_values(&self) -> (Option<u8>, Option<u8>) {
        (None, Some(6))
    }

    async fn run(&self,
                 state: Arc<BotState<Self::Bot>>,
                 interaction: InteractionUse<MenuSelectData<Self::Data>, Unused>,
    ) -> Result<InteractionUse<MenuSelectData<Self::Data>, Used>, BotError> {
        let embed = {
            let mut guard = state.menus.write().unwrap();
            let config = &mut guard
                .get_mut(&interaction.data.custom_id)
                .unwrap()
                .downcast_mut::<Self>()
                .unwrap()
                .0;
            config.roles = interaction.data.values.iter()
                .map(Character::from)
                .collect();
            config.embed()
        };
        interaction.update(&state, embed).await
            .map_err(|e| e.into())
    }
}