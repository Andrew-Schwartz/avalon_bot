use discorsd::commands::SlashCommandRaw;

use crate::Bot;

pub mod addme;
pub mod info;
pub mod ping;
pub mod rules;
pub mod stop;
pub mod uptime;
pub mod start;
pub mod system_info;
pub mod ll;
pub mod unpin;
pub mod test;

pub fn commands() -> Vec<Box<dyn SlashCommandRaw<Bot=Bot>>> {
    vec![
        Box::new(addme::AddMeCommand),
        Box::new(start::StartCommand::default()),
        Box::new(stop::StopCommand::default()),
    ]
}