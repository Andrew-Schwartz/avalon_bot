#![warn(clippy::pedantic, clippy::nursery)]
// @formatter:off
#![allow(
    clippy::module_name_repetitions,
    clippy::wildcard_imports,
    clippy::enum_glob_use,
    clippy::empty_enum,
    clippy::too_many_lines,
    clippy::non_ascii_literal,
    clippy::option_if_let_else,
    clippy::option_option,
    clippy::default_trait_access,
    clippy::filter_map,
    clippy::must_use_candidate,
    clippy::similar_names,
    clippy::unit_arg,
    // nursery
    clippy::missing_const_for_fn,
)]
// @formatter:on

use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::fmt::{self, Debug};
use std::io::Write;
use std::prelude::v1::Result::Ok;
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};
use itertools::Itertools;
use log::{error, info};
use log::LevelFilter;
use once_cell::sync::OnceCell;
use serde::Deserialize;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use discorsd::{Bot as _, BotExt, BotState, GuildCommands, IdMap, shard};
use discorsd::async_trait;
use discorsd::commands::*;
use discorsd::errors::BotError;
use discorsd::http::channel::{ChannelExt, create_message, CreateMessage, embed};
use discorsd::http::ClientResult;
use discorsd::http::guild::{CommandPermsExt, GuildCommandPermsExt};
use discorsd::model::channel::Channel;
use discorsd::model::guild::{Guild, Integration};
use discorsd::model::ids::*;
use discorsd::model::interaction::Interaction;
use discorsd::model::message::Message;
use discorsd::model::permissions::{Permissions, Role};
use discorsd::shard::dispatch::ReactionUpdate;
use discorsd::shard::model::{Activity, ActivityType, Identify, StatusType, UpdateStatus};

use crate::avalon::Avalon;
use crate::avalon::game::AvalonGame;
use crate::commands::info::InfoCommand;
use crate::commands::ll::LowLevelCommand;
use crate::commands::ping::PingCommand;
use crate::commands::rules::RulesCommand;
use crate::commands::system_info::SysInfoCommand;
use crate::commands::unpin::UnpinCommand;
use crate::commands::uptime::UptimeCommand;
use crate::hangman::Hangman;
use crate::hangman::random_words::GuildHist;

#[macro_use]
mod macros;
mod commands;
mod avalon;
mod hangman;
pub mod utils;
pub mod games;

#[derive(Deserialize)]
pub struct Config {
    token: String,
    owner: UserId,
    channel: ChannelId,
    guild: GuildId,
}

impl Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("steadfast_id", &self.owner)
            .field("dev_channel", &self.channel)
            .field("guild_id", &self.guild)
            .finish()
    }
}

pub struct Bot {
    config: Config,
    avalon_games: RwLock<HashMap<GuildId, Avalon>>,
    hangman_games: RwLock<HashMap<GuildId, Hangman>>,
    guild_hist_words: RwLock<IdMap<GuildHist>>,
    user_games: RwLock<HashMap<UserId, HashSet<GuildId>>>,
    first_log_in: OnceCell<DateTime<Utc>>,
    log_in: RwLock<Option<DateTime<Utc>>>,
}

impl Bot {
    fn new(config: Config) -> Self {
        Self {
            config,
            avalon_games: Default::default(),
            hangman_games: Default::default(),
            guild_hist_words: Default::default(),
            user_games: Default::default(),
            first_log_in: Default::default(),
            log_in: Default::default(),
        }
    }
}

#[tokio::main]
async fn main() -> shard::ShardResult<()> {
    env_logger::builder()
        .format(|f, record|
            writeln!(f,
                     "{} [{}] {}",
                     Local::now().format("%e %T"),
                     record.level(),
                     record.args(),
            ))
        .filter(None, LevelFilter::Info)
        .init();

    tokio::spawn(async {
        use tokio::{io::AsyncWriteExt, fs::File, time::delay_for};

        loop {
            match File::create("uptime.txt").await {
                Ok(mut file) => match file.write_all(format!("{:?}", Utc::now()).as_bytes()).await {
                    Ok(()) => info!("Updated uptime file"),
                    Err(e) => error!("Error writing uptime file {}", e),
                }
                Err(e) => error!("Error opening uptime file {}", e),
            }

            // write file every 15 mins
            delay_for(std::time::Duration::from_secs(60 * 15)).await;
        }
    });

    let path = if std::env::args().any(|arg| arg == "--dev") {
        "config-dev.json"
    } else {
        "config.json"
    };

    let config = std::fs::read_to_string(path).expect("Could not find config file");
    let config: Config = serde_json::from_str(&config).expect("Could not read config file");
    Bot::new(config).run().await
}

type Result<T> = std::result::Result<T, BotError>;

#[async_trait]
impl discorsd::Bot for Bot {
    fn token(&self) -> &str {
        self.config.token.as_str()
    }

    fn identify(&self) -> Identify {
        Identify::new(self.token().into()).presence(UpdateStatus {
            since: None,
            activities: Some(vec![Activity::for_bot("Avalon - try /addme", ActivityType::Game)]),
            status: StatusType::Online,
            afk: false,
        })
    }

    fn global_commands() -> &'static [&'static dyn SlashCommandRaw<Bot=Self>] {
        &[
            &InfoCommand, &PingCommand, &UptimeCommand, &SysInfoCommand, &RulesCommand, &UnpinCommand
        ]
    }

    fn guild_commands() -> Vec<Box<dyn SlashCommandRaw<Bot=Self>>> {
        let mut vec = commands::commands();
        vec.extend(avalon::commands());
        vec.extend(hangman::commands());
        vec
    }

    async fn ready(&self, state: Arc<BotState<Self>>) -> Result<()> {
        if let Err(now) = self.first_log_in.set(Utc::now()) {
            *self.log_in.write().await = Some(now);
        }

        state.client.create_message(state.bot.config.channel, CreateMessage::build(|m| {
            m.embed(|e| {
                e.title("Avalon Bot is logged on!");
                e.timestamp_now();
                e.url("https://github.com/Andrew-Schwartz/AvalonBot")
            });
        })).await?;

        Ok(())
    }

    async fn resumed(&self, state: Arc<BotState<Self>>) -> Result<()> {
        state.client.create_message(state.bot.config.channel, create_message(|m| {
            m.embed(|e| {
                e.title("Avalon Bot has resumed");
                e.timestamp_now();
            });
        })).await?;
        Ok(())
    }

    async fn guild_create(&self, guild: Guild, state: Arc<BotState<Self>>) -> Result<()> {
        info!("Guild Create: {} ({})", guild.name.as_ref().unwrap(), guild.id);
        self.avalon_games.write().await.entry(guild.id).or_default();

        self.initialize_guild_commands(&guild, &state).await?;

        self.config.channel.send(&state, format!(
            "🎉 Joined new guild **{}** (`{}`) 🎉",
            guild.name.as_ref().unwrap(),
            guild.id,
        )).await?;

        Ok(())
    }

    async fn message_create(&self, message: Message, state: Arc<BotState<Self>>) -> Result<()> {
        match message.content.as_str() {
            "!timestamp" => {
                message.channel.send(
                    &state,
                    format!("You created your account at {}", message.author.id.timestamp()),
                ).await?;
            }
            "!lots" => {
                let user = state.user().await;
                message.channel.send(&state, embed(|e| {
                    e.image("english_channel.jpg");
                    e.thumbnail("av_bot_dev.png");
                    e.authored_by(&user);
                    e.footer("look at my foot", "av_bot_dev.png");
                    for i in 0..6 {
                        match i % 3 {
                            0 => e.add_inline_field("left col", i),
                            1 => e.add_blank_inline_field(),
                            2 => e.field(("right col", format!("i = {}", i), true)),
                            _ => unreachable!(),
                        };
                    }
                }),
                ).await?;
            }
            "!log" => {
                info!("{:#?}", self.debug().await);
                message.channel.send(&state, "logged!").await?;
            }
            "!cache" => {
                info!("{:#?}", state.cache.debug().await);
                message.channel.send(&state, "logged!").await?;
            }
            "!commands" => {
                let commands = state.commands.read().await;
                for (guild, commands) in commands.iter() {
                    let commands = commands.read().await;
                    println!("\nGUILD {}\n------------------------------", guild);
                    for command in commands.iter() {
                        println!("command = {:?}", command);
                    }
                }
                println!("\nEXISTING COMMANDS\n------------------------------");
                let commands = state.client.get_guild_commands(
                    state.application_id().await,
                    message.guild_id.unwrap(),
                ).await?;
                for command in commands {
                    println!("command = {:?}", command);
                }
                message.channel.send(&state, "logged!").await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn interaction(&self, interaction: Interaction, state: Arc<BotState<Self>>) -> Result<()> {
        Self::slash_command(interaction, state).await
    }

    async fn reaction(&self, reaction: ReactionUpdate, state: Arc<BotState<Self>>) -> Result<()> {
        // println!("reaction = {:?}", reaction);
        let mut results = Vec::new();
        let commands = state.reaction_commands.read().await.iter()
            .filter(|rc| rc.applies(&reaction))
            .cloned()
            .collect_vec();
        for command in commands {
            let result = command.run(Arc::clone(&state), reaction.clone()).await;
            results.push(result);
        }
        for res in results {
            res?;
        }

        Ok(())
    }

    async fn integration_update(&self, guild_id: GuildId, integration: Integration, state: Arc<BotState<Self>>) -> Result<()> {
        info!("Guild Integration Update: {:?}", integration);

        let guild = state.cache.guild(guild_id).await.unwrap();
        self.initialize_guild_commands(&guild, &state).await?;

        let channels = state.cache.guild_channels(guild_id, Channel::text).await;
        let channel = channels.iter().find(|c| c.name == "general")
            .unwrap_or_else(|| channels.iter().next().unwrap());
        channel.send(&state, "Slash Commands are now enabled!").await?;
        Ok(())
    }

    // todo should just be one method but have an enum for Create/Update/Delete
    async fn role_create(&self, guild: GuildId, role: Role, state: Arc<BotState<Self>>) -> Result<()> {
        let unpin_command = state.global_command_id::<UnpinCommand>().await;
        unpin_command.edit_permissions(state, guild, vec![CommandPermissions {
            id: role.id.into(),
            permission: unpin_perms(&role),
        }]).await?;
        Ok(())
    }

    async fn role_update(&self, guild: GuildId, role: Role, state: Arc<BotState<Self>>) -> Result<()> {
        let unpin_command = state.global_command_id::<UnpinCommand>().await;
        unpin_command.edit_permissions(state, guild, vec![CommandPermissions {
            id: role.id.into(),
            permission: unpin_perms(&role),
        }]).await?;
        Ok(())
    }

    async fn error(&self, error: BotError, state: Arc<BotState<Self>>) {
        error!("{}", error.display_error(&state).await);
    }
}

impl Bot {
    /// The first time connecting to a guild, run this to delete any commands Discord has saved from
    /// the last time the bot was started
    // todo move to BotExt or smth
    async fn initialize_guild_commands(
        &self,
        guild: &Guild,
        state: &BotState<Self>,
    ) -> ClientResult<()> {
        // this should be only place that writes to first level of `commands`
        let first_time = match state.commands.write().await.entry(guild.id) {
            Entry::Vacant(vacant) => {
                vacant.insert(Default::default());
                true
            }
            Entry::Occupied(_) => false,
        };
        if first_time {
            let commands = state.commands.read().await;
            let mut commands = commands.get(&guild.id).unwrap().write().await;
            let rcs = state.reaction_commands.write().await;

            Self::reset_guild_command_perms(
                state, guild.id, &mut commands, rcs,
            ).await?;

            // set up perms for `/unpin`
            let unpin_command = state.global_command_id::<UnpinCommand>().await;
            let disallow = guild.roles.iter()
                .filter(|r| !unpin_perms(r))
                .map(Role::id);
            unpin_command.disallow_roles(&state, guild.id, disallow).await?;
            unpin_command.allow_users(&state, guild.id, &[guild.owner_id]).await?;

            if guild.id == self.config.guild {
                // `/ll` only in testing server
                let command = state.client.create_guild_command(
                    state.application_id().await,
                    guild.id,
                    LowLevelCommand.command(),
                ).await?;
                commands.insert(command.id, Box::new(LowLevelCommand));
                command.id.allow_users(&state, guild.id, &[self.config.owner]).await?;
            }
        }
        Ok(())
    }

    async fn reset_guild_command_perms(
        state: &BotState<Self>,
        guild: GuildId,
        commands: &mut RwLockWriteGuard<'_, GuildCommands<Self>>,
        mut reaction_commands: RwLockWriteGuard<'_, Vec<Box<dyn ReactionCommand<Self>>>>,
    ) -> ClientResult<()> {
        reaction_commands.retain(|rc| !AvalonGame::is_reaction_command(rc.as_ref(), guild));
        drop(reaction_commands);

        let app = state.application_id().await;
        let guild_commands = Self::guild_commands();
        let guild_commands: GuildCommands<_> = state.client.bulk_overwrite_guild_commands(
            app, guild,
            guild_commands.iter().map(|c| c.command()).collect(),
        ).await
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .zip(guild_commands)
            .collect();
        let command_names = guild_commands.iter()
            .map(|(&id, command)| (command.name(), id))
            .collect();

        **commands = guild_commands;

        *state.command_names.write().await
            .entry(guild)
            .or_default() = RwLock::new(command_names);

        // clear any left over perms
        guild.batch_edit_permissions(state, vec![]).await?;
        Ok(())
    }

    pub async fn most_recent_login(&self) -> Option<DateTime<Utc>> {
        if let Some(time) = *self.log_in.read().await {
            Some(time)
        } else {
            self.first_log_in.get().copied()
        }
    }

    pub async fn debug(&self) -> DebugBot<'_> {
        let Self {
            config,
            hangman_games,
            guild_hist_words,
            first_log_in: ready,
            log_in: resume,
            avalon_games: games,
            user_games
        } = self;
        #[allow(clippy::eval_order_dependence)]
        DebugBot {
            config,
            games: games.read().await,
            hangman_games: hangman_games.read().await,
            guild_hist_words: guild_hist_words.read().await,
            user_games: user_games.read().await,
            ready: ready.get(),
            resume: resume.read().await,
        }
    }
}

#[derive(Debug)]
pub struct DebugBot<'a> {
    config: &'a Config,
    games: RwLockReadGuard<'a, HashMap<GuildId, Avalon>>,
    hangman_games: RwLockReadGuard<'a, HashMap<GuildId, Hangman>>,
    guild_hist_words: RwLockReadGuard<'a, IdMap<GuildHist>>,
    user_games: RwLockReadGuard<'a, HashMap<UserId, HashSet<GuildId>>>,
    ready: Option<&'a DateTime<Utc>>,
    resume: RwLockReadGuard<'a, Option<DateTime<Utc>>>,
}

fn unpin_perms(role: &Role) -> bool {
    role.permissions.intersects(
        Permissions::ADMINISTRATOR |
            Permissions::MANAGE_CHANNELS |
            Permissions::MANAGE_GUILD |
            Permissions::MANAGE_MESSAGES
    )
}
