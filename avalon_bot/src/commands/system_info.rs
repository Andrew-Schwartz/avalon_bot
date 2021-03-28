use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use itertools::Itertools;
use once_cell::sync::Lazy;
use sysinfo::{ComponentExt, ProcessorExt, System, SystemExt};

use command_data_derive::*;
use discorsd::async_trait;
use discorsd::commands::*;
use discorsd::http::channel::embed;
use discorsd::model::message::Color;

use crate::avalon::{BotError, BotState};
use crate::Bot;

static SYS_INFO: Lazy<Mutex<System>> = Lazy::new(|| Mutex::new(System::new_all()));

#[derive(Debug, Copy, Clone)]
pub struct SysInfoCommand;

#[async_trait]
impl SlashCommandData for SysInfoCommand {
    type Bot = Bot;
    type Data = Data;
    const NAME: &'static str = "system-info";

    fn description(&self) -> Cow<'static, str> {
        "Gets some information about the computer running this bot".into()
    }

    #[allow(clippy::cast_precision_loss)]
    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<Used>, BotError> {
        let mut embed = embed(|e| {
            e.title("System Usage Information");
            // a nice blue
            e.color(Color::from_rgb(0x2E, 0x8B, 0xC0));
        });

        {
            let mut sys = SYS_INFO.lock().unwrap();
            sys.refresh_all();

            if data.0.iter().any(|it| matches!(it, Choices::Cpu | Choices::All)) {
                let processors = sys.get_processors();
                let value = if processors.is_empty() {
                    "No CPUs found".to_owned()
                } else {
                    let value = processors.iter()
                        .map(|p| format!("{}: {:.2}%", p.get_name(), p.get_cpu_usage()))
                        .join("\n");
                    let avg: f32 = processors.iter()
                        .map(ProcessorExt::get_cpu_usage)
                        .sum();
                    format!("{}\n**Average**: {:.2}%", value, avg / processors.len() as f32)
                };
                embed.field(("CPU Usage", value))
            }
            if data.0.iter().any(|it| matches!(it, Choices::Memory | Choices::All)) {
                let used = sys.get_available_memory();
                let total = sys.get_total_memory();
                let string = format!(
                    "{:.2}% used\n\
                     Used: {} MB\n\
                     Total: {} MB",
                    (used as f64) / (total as f64) * 100.0,
                    used / 1024,
                    total / 1024
                );
                embed.field(("Memory Usage", string));
            }
            if data.0.iter().any(|it| matches!(it, Choices::Temperature | Choices::All)) {
                let components = sys.get_components();
                let value = if components.is_empty() {
                    "No components found".to_owned()
                } else {
                    let value = components.iter()
                        .map(|c| format!("{}: {:.2} °C", c.get_label(), c.get_temperature()))
                        .join("\n");
                    let avg: f32 = components.iter()
                        .map(ComponentExt::get_temperature)
                        .sum();
                    format!("{}\n**Average**: {:.2} °C", value, avg / components.len() as f32)
                };
                embed.field(("Component Temperature", value));
            }
        }

        interaction.respond(state, embed).await.map_err(|e| e.into())
    }
}

#[derive(CommandData, Debug)]
pub struct Data(#[command(vararg = "data", va_count = 3, required = 1)] HashSet<Choices>);

#[derive(CommandDataOption, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Choices {
    Cpu,
    Memory,
    Temperature,
    All,
}