use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use sysinfo::{ComponentExt, ProcessorExt, System, SystemExt};

use command_data_derive::*;
use discorsd::async_trait;
use discorsd::commands::*;

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

    async fn run(&self,
                 state: Arc<BotState<Bot>>,
                 interaction: InteractionUse<Unused>,
                 data: Self::Data,
    ) -> Result<InteractionUse<Used>, BotError> {
        println!("data = {:?}", data);
        {
            let mut sys = SYS_INFO.lock().unwrap();
            sys.refresh_all();
            // println!("sys.get_global_processor_info() = {:?}", sys.get_global_processor_info());
            for processor in sys.get_processors() {
                println!("processor.get_cpu_usage()% = {:?}", processor.get_cpu_usage());
            }
            for component in sys.get_components() {
                println!("component.get_temperature() °C = {:?}", component.get_temperature());
            }
            println!("sys.get_total_memory() = {:?}", sys.get_total_memory());
            println!("sys.get_used_memory()  = {:?}", sys.get_used_memory());
        }
        interaction.respond(state, "hi").await.map_err(|e| e.into())
    }
}

#[derive(CommandData, Debug)]
pub struct Data(#[command(vararg = "data", va_count = 3)] HashSet<Choices>);

#[derive(CommandDataOption, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Choices {
    Memory,
    Cpu,
    Temperature,
}