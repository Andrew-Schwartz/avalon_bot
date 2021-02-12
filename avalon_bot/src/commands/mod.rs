use discorsd::commands::SlashCommand;

use crate::Bot;
use crate::commands::info::INFO_COMMAND;
use crate::commands::ping::PING_COMMAND;
use crate::commands::uptime::UPTIME_COMMAND;

pub mod addme;
pub mod info;
pub mod ping;
pub mod rules;
pub mod stop;
pub mod uptime;

pub const GLOBAL_COMMANDS: [&'static dyn SlashCommand<Bot>; 3] = [
    &INFO_COMMAND,
    &PING_COMMAND,
    &UPTIME_COMMAND
];