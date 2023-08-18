// use std::collections::HashMap;
//
// use discorsd::BotState;
// use discorsd::commands::{AppCommandData, InteractionUse, Unused, Used};
// use discorsd::errors::BotError;
// use discorsd::http::channel::create_message;
// use discorsd::http::ClientResult;
// use discorsd::model::components::ButtonStyle;
// use discorsd::model::guild::GuildMember;
// use discorsd::model::ids::{Id, UserId};
// use discorsd::model::interaction::Token;
// use discorsd::model::message::{Color, Message};
// use discorsd::model::user::UserMarkup;
//
// use crate::avalon::characters::Character;
// use crate::avalon::characters::Loyalty::Evil;
// use crate::avalon::max_evil;
// use crate::Bot;
//
// pub async fn start_setup(
//     state: &BotState<Bot>,
//     interaction: InteractionUse<AppCommandData, Unused>,
// ) -> Result<InteractionUse<AppCommandData, Used>, BotError> {
//     let guild = interaction.guild().expect("Todo show error message if used in dm");
//
//     let mut game_guard = state.bot.avalon_games.write().await;
//     let coup = game_guard
//         .entry(guild)
//         .or_default();
//     let Avalon::Setup(setup) = coup else {
//         return send_game_error(&state, interaction).await;
//     };
//     let member = interaction.source
//         .guild_ref()
//         .cloned()
//         .expect("Guild Command")
//         .member;
//     setup.players.insert(
//         member.id(),
//         (member, interaction.token.clone()),
//     );
//     setup.update_settings_message(state, interaction).await
//         .map_err(Into::into)
// }
//
// pub struct AvalonSetup {
//     players: HashMap<UserId, (GuildMember, Token)>,
//     roles: Vec<Character>,
//     lotl: bool,
//     settings_display: Option<Token>,
// }
//
// impl AvalonSetup {
//     pub async fn update_settings_message(
//         &mut self,
//         state: &BotState<Bot>,
//         interaction: InteractionUse<AppCommandData, Unused>,
//     ) -> ClientResult<InteractionUse<AppCommandData, Used>> {
//         let message = create_message(|m| {
//             m.embed(|e| {
//                 e.title("Avalon Setup");
//                 e.color(Color::GOLD);
//                 let players_list = self.players.keys()
//                     .map(UserId::ping)
//                     .join("\n");
//                 e.add_field(
//                     format!("Players ({})", self.players.len()),
//                     if players_list.is_empty() {
//                         "None yet".into()
//                     } else {
//                         players_list
//                     },
//                 );
//                 let mut roles = self.roles.iter()
//                     .copied()
//                     .map(Character::name)
//                     .join("\n");
//                 let mut fill = |num_players, max_evil| {
//                     let num_evil = self.roles.iter()
//                         .filter(|c| c.loyalty() == Evil)
//                         .count();
//                     let num_good = self.roles.len() - num_evil;
//                     let mom = max_evil as i32 - num_evil as i32;
//                     let ls = num_players as i32 - max_evil as i32 - num_good as i32;
//                     if ls != 0 {
//                         roles.push_str(&format!("\n{}x Loyal Servant", ls));
//                     }
//                     if mom != 0 {
//                         roles.push_str(&format!("\n{}x Minion of Mordred", mom))
//                     }
//                 };
//                 match max_evil(self.players.len()) {
//                     None if self.players.len() < 5 => {
//                         // assume that there will be 5 players, so treat max_evil as 2
//                         let max_evil = 2;
//                         fill(5, max_evil)
//                     }
//                     Some(max_evil) => {
//                         fill(self.players.len(), max_evil)
//                     }
//                     None => {
//                         // AvalonError::TooManyPlayers(self.players.len())?
//                     }
//                 }
//                 e.add_field(
//                     "Roles",
//                     roles,
//                 );
//             });
//             m.menu(state, StartingCoinsMenu, |m| {
//                 m.min_values(1);
//                 m.default_options(|value| value == self.starting_coins.to_string());
//             });
//             m.buttons(state, [
//                 (Box::new(JoinLeaveButton(true)) as _, make_button(|b| b.label("Join game"))),
//                 (Box::new(JoinLeaveButton(false)) as _, make_button(|b| {
//                     b.label("Leave game");
//                     b.style(ButtonStyle::Danger);
//                 })),
//             ]);
//             m.button(state, StartButton, |b| {
//                 b.label("Start!");
//                 b.style(ButtonStyle::Success);
//                 if !self.can_start() {
//                     b.disable();
//                 }
//             });
//         });
//
//         match &mut self.settings_display {
//             Some(settings) if settings.channel == interaction.channel => {
//                 settings.edit(state, create_message).await?;
//                 interaction.delete(state).await
//             }
//             no_settings => {
//                 let interaction = interaction.respond(state, message).await?;
//                 *no_settings = Some(interaction.token.clone());
//                 Ok(interaction)
//             }
//         }
//     }
// }
//
// pub struct AvalonGame {}
//
// pub enum Avalon {
//     Setup(AvalonSetup),
//     Game(AvalonGame),
// }