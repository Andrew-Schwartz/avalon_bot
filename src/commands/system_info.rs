use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use itertools::Itertools;
use once_cell::sync::Lazy;
use sysinfo::{ComponentExt, Cpu, CpuExt, System, SystemExt};

use command_data_derive::*;
use discorsd::{async_trait, BotState};
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::embed;
use discorsd::model::message::Color;

use crate::Bot;

static SYS_INFO: Lazy<Mutex<System>> = Lazy::new(|| Mutex::new(System::new_all()));

#[derive(Debug, Copy, Clone)]
pub struct SysInfoCommand;

#[async_trait]
impl SlashCommand for SysInfoCommand {
    type Bot = Bot;
    type Data = Data;
    type Use = Used;
    const NAME: &'static str = "system-info";

    fn description(&self) -> Cow<'static, str> {
        "Gets some information about the computer running this bot".into()
    }

    #[allow(clippy::cast_precision_loss)]
    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<SlashCommandData, Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<SlashCommandData, Used>, BotError> {
        let mut embed = embed(|e| {
            e.title("System Usage Information");
            // a nice blue
            e.color(Color::from_rgb(0x2E, 0x8B, 0xC0));
        });

        {
            let mut sys = SYS_INFO.lock().unwrap();
            sys.refresh_all();

            if data.0.iter().any(|it| matches!(it, Choices::Cpu | Choices::All)) {
                let cpus = sys.cpus();
                let value = if cpus.is_empty() {
                    "No CPUs found".to_owned()
                } else {
                    let value = cpus.iter()
                        .map(|cpu| format!("{}: {:.2}%", cpu.name(), cpu.cpu_usage()))
                        .join("\n");
                    if cpus.len() == 1 {
                        value
                    } else {
                        let avg: f32 = cpus.iter()
                            .map(Cpu::cpu_usage)
                            .sum();
                        format!("```{}\nAverage: {:.2}%```", value, avg / cpus.len() as f32)
                    }
                };
                embed.field(("CPU Usage", value))
            }
            if data.0.iter().any(|it| matches!(it, Choices::Memory | Choices::All)) {
                let used = sys.used_memory();
                let total = sys.total_memory();
                let string = format!(
                    "```    {:.2}%\n\
                     Used : {} MB\n\
                     Total: {} MB```",
                    (used as f64) / (total as f64) * 100.0,
                    used / 1024,
                    total / 1024
                );
                embed.field(("Memory Usage", string));
            }
            if data.0.iter().any(|it| matches!(it, Choices::Temperature | Choices::All)) {
                let components = sys.components();
                let value = if components.is_empty() {
                    "No components found".to_owned()
                } else {
                    let value = components.iter()
                        .map(|c| format!("{}: {:.2} °C", c.label(), c.temperature()))
                        .join("\n");
                    if components.len() == 1 {
                        value
                    } else {
                        let avg: f32 = components.iter()
                            .map(ComponentExt::temperature)
                            .sum();
                        format!("```{}\nAverage: {:.2} °C```", value, avg / components.len() as f32)
                    }
                };
                embed.field(("Component Temperature", value));
            }
        }

        interaction.respond(state, embed).await.map_err(|e| e.into())
    }
}

#[derive(CommandData, Debug)]
// todo make required = 0 work with default
pub struct Data(#[command(vararg = "data", va_count = 3, va_req = 1)] HashSet<Choices>);

#[derive(CommandDataChoices, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Choices {
    All,
    Cpu,
    Memory,
    Temperature,
}