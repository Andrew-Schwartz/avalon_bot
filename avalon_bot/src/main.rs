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
use std::hint::unreachable_unchecked;
use std::io::Write;
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};
use itertools::Itertools;
use log::{error, info};
use log::LevelFilter;
use once_cell::sync::OnceCell;
use serde::Deserialize;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use avalon::lotl::ToggleLady;
use avalon::roles::RolesCommand;
use discorsd::{BotExt, BotState, GuildCommands, IdMap, shard};
use discorsd::async_trait;
use discorsd::commands::{ReactionCommand, SlashCommand};
use discorsd::errors::BotError;
use discorsd::http::channel::{ChannelExt, create_message, CreateMessage, embed};
use discorsd::http::ClientResult;
use discorsd::model::channel::Channel;
use discorsd::model::guild::{Guild, Integration};
use discorsd::model::ids::*;
use discorsd::model::interaction::Interaction;
use discorsd::model::message::Message;
use discorsd::shard::dispatch::ReactionUpdate;
use discorsd::shard::model::{Activity, ActivityType, Identify, StatusType, UpdateStatus};

use crate::avalon::Avalon;
use crate::avalon::game::AvalonGame;
pub use crate::commands::{addme::AddMeCommand};
use crate::commands::start::StartCommand;
use crate::games::GameType;
use crate::hangman::Hangman;
use crate::hangman::hangman_command::HangmanCommand;
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
    start: RwLock<HashMap<GuildId, CommandId>>,
    first_log_in: OnceCell<DateTime<Utc>>,
    log_in: RwLock<Option<DateTime<Utc>>>,
}

impl Bot {
    fn new(config: Config) -> Self {
        Self {
            config,
            // commands: Default::default(),
            // reaction_commands: Default::default(),
            avalon_games: Default::default(),
            hangman_games: Default::default(),
            guild_hist_words: Default::default(),
            user_games: Default::default(),
            start: Default::default(),
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

    fn global_commands() -> &'static [&'static dyn SlashCommand<Bot=Self>] {
        &commands::GLOBAL_COMMANDS
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

        {
            // deletes any commands Discord has saved from the last time this bot was run
            Self::clear_old_commands(guild.id, &state).await.unwrap();

            let mut commands = state.commands.write().await;
            let commands = commands.entry(guild.id).or_default().write().await;
            let rcs = state.reaction_commands.write().await;
            Self::reset_guild_commands(guild.id, &state, commands, rcs).await;
        }

        self.config.channel.send(&state, format!(
            "🎉 Joined new guild **{}** (`{}`) 🎉",
            guild.name.as_ref().unwrap(),
            guild.id,
        )).await?;

        Ok(())
    }

    async fn message_create(&self, message: Message, state: Arc<BotState<Self>>) -> Result<()> {
        match message.content.as_ref() {
            "!ping" => {
                let mut resp = state.client.create_message(message.channel, CreateMessage::build(|m| {
                    m.embed(|e| {
                        e.title("Pong");
                    });
                })).await?;
                #[allow(clippy::map_err_ignore)]
                    let elapsed = Utc::now()
                    .signed_duration_since(message.timestamp)
                    .to_std()
                    .map_err(|_| BotError::Chrono)?;
                let embed = resp.embeds.remove(0);
                resp.edit(&state, embed.build(|e| {
                    e.footer_text(format!("Took {:?} to respond", elapsed));
                })).await?;
            }
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
                            _ => unsafe { unreachable_unchecked() }
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

    async fn integration_update(&self, guild: GuildId, integration: Integration, state: Arc<BotState<Self>>) -> Result<()> {
        info!("Guild Integration Update: {:?}", integration);

        {
            let mut commands = state.commands.write().await;
            let commands = commands.entry(guild.id()).or_default().write().await;
            let rcs = state.reaction_commands.write().await;
            Self::reset_guild_commands(guild.id(), &state, commands, rcs).await;
        }

        let channels = state.cache.guild_channels(guild, Channel::text).await;
        let channel = channels.iter().find(|c| c.name == "general")
            .unwrap_or_else(|| channels.iter().next().unwrap());
        channel.send(&state, "Slash Commands are now enabled!").await?;
        Ok(())
    }

    async fn error(&self, error: BotError, state: Arc<BotState<Self>>) {
        error!("{}", error.display_error(&state).await);
    }
}

async fn delete_command<F: Fn(&dyn SlashCommand<Bot=Bot>) -> bool + Send>(
    state: &BotState<Bot>,
    guild: GuildId,
    commands: &mut GuildCommands<Bot>,
    pred: F,
) -> ClientResult<()> {
    let ids = commands.iter()
        .filter(|(_, c)| pred(c.as_ref()))
        .map(|(id, _)| *id)
        .collect_vec();
    for id in ids {
        state.client.delete_guild_command(
            state.application_id().await, guild, id,
        ).await?;
        commands.remove(&id);
    }
    Ok(())
}

async fn create_command<C: SlashCommand<Bot=Bot>>(
    state: &BotState<Bot>,
    guild: GuildId,
    commands: &mut GuildCommands<Bot>,
    command: C,
) -> ClientResult<CommandId> {
    let resp = state.client.create_guild_command(
        state.application_id().await,
        guild,
        command.command(),
    ).await?;
    commands.insert(resp.id, Box::new(command));

    Ok(resp.id)
}

impl Bot {
    // async fn init_guild_commands(&self, guild: GuildId, state: &BotState<Bot>) -> ClientResult<()> {
    //     let app = state.application_id().await;
    //     let commands = state.client.get_guild_commands(app, guild).await?;
    //     for command in commands {
    //         state.client
    //             .delete_guild_command(app, guild, command.id)
    //             .await.unwrap();
    //     }
    //     let mut commands = state.commands.write().await;
    //     let mut commands = commands.entry(guild)
    //         .or_default()
    //         .write().await;
    //     // create_command(&*state, guild, &mut commands, UptimeCommand).await?;
    //     let rcs = state.reaction_commands.write().await;
    //     self.reset_guild_commands(&state, &mut commands, rcs, guild).await;
    //     Ok(())
    // }

    async fn reset_guild_commands(
        guild: GuildId,
        state: &BotState<Self>,
        mut commands: RwLockWriteGuard<'_, GuildCommands<Self>>,
        mut reactions: RwLockWriteGuard<'_, Vec<Box<dyn ReactionCommand<Self>>>>,
    ) {
        reactions.retain(|rc| !AvalonGame::is_reaction_command(rc.as_ref(), guild));
        drop(reactions);
        let application = state.application_id().await;
        let old_commands = state.client
            .get_guild_commands(application, guild)
            .await.unwrap();
        for command in old_commands {
            if let Some(slash_command) = commands.get(&command.id) {
                if AvalonGame::is_command(slash_command.as_ref()) {
                    let delete = state.client.delete_guild_command(application, guild, command.id).await;
                    if let Err(e) = delete {
                        error!("Failed to delete {} ({}) in guild {}, {:?}", command.name, command.id, guild, e);
                    }
                    commands.remove(&command.id);
                }
            }
        }
        let new: Vec<Box<dyn SlashCommand<Bot=Self>>> = vec![
            Box::new(RolesCommand(vec![])),
            Box::new(AddMeCommand),
            Box::new(ToggleLady),
            Box::new(HangmanCommand),
            // todo write the logic for taking this off of here when hangman starts etc (also ^)
        ];
        let new = discorsd::commands::create_guild_commands(&state, guild, new).await;
        commands.extend(new);
        let start_id = create_command(
            state, guild, &mut commands, StartCommand(set!(GameType::Hangman)),
        ).await.unwrap();
        // todo when `Entry.insert` is stabilized just use that
        match state.bot.start.write().await.entry(guild) {
            Entry::Occupied(mut o) => { o.insert(start_id); },
            Entry::Vacant(v) => { v.insert(start_id); },
        };
    }

    pub async fn most_recent_login(&self) -> Option<DateTime<Utc>> {
        if let Some(time) = *self.log_in.read().await {
            Some(time)
        } else {
            self.first_log_in.get().copied()
        }
    }

    pub async fn debug(&self) -> DebugBot<'_> {
        let Self { config, hangman_games, guild_hist_words, start, first_log_in: ready, log_in: resume, avalon_games: games, user_games } = self;
        #[allow(clippy::eval_order_dependence)]
        DebugBot {
            config,
            games: games.read().await,
            hangman_games: hangman_games.read().await,
            guild_hist_words: guild_hist_words.read().await,
            user_games: user_games.read().await,
            start: start.read().await,
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
    start: RwLockReadGuard<'a, HashMap<GuildId, CommandId>>,
    ready: Option<&'a DateTime<Utc>>,
    resume: RwLockReadGuard<'a, Option<DateTime<Utc>>>,
}