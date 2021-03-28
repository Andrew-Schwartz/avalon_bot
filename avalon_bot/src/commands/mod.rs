use discorsd::commands::SlashCommand;

use crate::Bot;
use crate::commands::info::InfoCommand;
use crate::commands::ping::PingCommand;
use crate::commands::rules::RulesCommand;
use crate::commands::system_info::SysInfoCommand;
use crate::commands::uptime::UptimeCommand;

pub mod addme;
pub mod info;
pub mod ping;
pub mod rules;
pub mod stop;
pub mod uptime;
pub mod start;
pub mod system_info;

// todo reset voting command for avalon
pub const GLOBAL_COMMANDS: [&'static dyn SlashCommand<Bot=Bot>; 5] = [
    &InfoCommand, &PingCommand, &UptimeCommand, &SysInfoCommand, &RulesCommand,
];
