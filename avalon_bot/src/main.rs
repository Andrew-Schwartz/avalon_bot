use std::collections::{HashMap, HashSet};
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
use discorsd::{BotExt, BotState, shard};
use discorsd::async_trait;
use discorsd::errors::{BotError, GameError};
use discorsd::http::{ClientError, ClientResult};
use discorsd::http::channel::{ChannelExt, create_message, CreateMessage, embed};
use discorsd::http::model::*;
use discorsd::shard::dispatch::ReactionUpdate;
use discorsd::shard::model::{Activity, ActivityType, Identify, StatusType, UpdateStatus};

use crate::avalon::Avalon;
use crate::avalon::game::AvalonGame;
use crate::commands::{AddMeCommand, InteractionUse, ReactionCommand, SlashCommand};

#[macro_use]
mod macros;
mod commands;
mod avalon;
pub mod utils;

pub type GuildCommands = HashMap<CommandId, Box<dyn SlashCommand>>;

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
    commands: RwLock<HashMap<GuildId, RwLock<GuildCommands>>>,
    reaction_commands: RwLock<Vec<Box<dyn ReactionCommand>>>,
    games: RwLock<HashMap<GuildId, Avalon>>,
    user_games: RwLock<HashMap<UserId, HashSet<GuildId>>>,
    first_log_in: OnceCell<DateTime<Utc>>,
    log_in: RwLock<Option<DateTime<Utc>>>,
}

impl Bot {
    fn new(config: Config) -> Self {
        Self {
            config,
            commands: Default::default(),
            reaction_commands: Default::default(),
            games: Default::default(),
            user_games: Default::default(),
            first_log_in: Default::default(),
            log_in: Default::default(),
        }
    }
}

#[tokio::main]
async fn main() -> shard::Result<()> {
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

    let path = if std::env::args()
        .filter(|arg| arg == "--dev")
        .next()
        .is_some() {
        "config-dev.json"
    } else { "config.json" };

    let config = std::fs::read_to_string(path).expect("Could not find config file");
    let config: Config = serde_json::from_str(&config).expect("Could not read config file");
    Bot::new(config).run().await
}

type Result<T> = std::result::Result<T, BotError>;

#[async_trait]
impl discorsd::Bot for Bot {
    type Error = BotError;

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
        commands::init_global_commands(&state).await;

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
        self.games.write().await.entry(guild.id).or_default();

        self.init_guild_commands(guild.id, &state).await?;

        if Utc::now().signed_duration_since(self.most_recent_login().await.unwrap()).num_seconds() >= 20 {
            self.config.channel.send(&state, format!(
                "🎉 Joined new guild **{}** (`{}`) 🎉",
                guild.name.as_ref().unwrap(),
                guild.id,
            )).await?;
        }

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
        let interaction = commands::run_global_commands(interaction, Arc::clone(&state)).await?;
        if let Err(interaction) = interaction {
            let id = interaction.data.id;
            let guild = interaction.guild_id;
            let command = {
                let guard = self.commands.read().await;
                let commands = guard.get(&guild).unwrap().read().await;
                commands.get(&id).cloned()
            };
            if let Some(command) = command {
                let (interaction, data) = InteractionUse::from(interaction);
                command.run(state, interaction, data).await?;
            }
        }
        Ok(())
    }

    async fn reaction(&self, reaction: ReactionUpdate, state: Arc<BotState<Self>>) -> Result<()> {
        let mut results = Vec::new();
        let commands = state.bot.reaction_commands.read().await.iter()
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
        self.init_guild_commands(guild, &state).await?;
        let channels = state.cache.guild_channels(guild, Channel::text).await;
        let channel = channels.iter().find(|c| c.name == "general")
            .unwrap_or_else(|| channels.iter().next().unwrap());
        channel.send(&state, "Slash Commands are now enabled!").await?;
        Ok(())
    }

    async fn error(&self, error: Self::Error, state: Arc<BotState<Self>>) {
        match error {
            BotError::Client(ce) => match ce {
                ClientError::Request(e) => error!("{}", e),
                ClientError::Http(status, route) => {
                    error!("`{}` on {}", status, route.debug_with_cache(&state.cache).await)
                }
                ClientError::Json(j) => error!("{}", j),
                ClientError::Io(io) => error!("{}", io),
                ClientError::Discord(de) => error!("{}", de),
            },
            BotError::Game(ge) => match ge {
                GameError::Avalon(ae) => error!("{}", ae)
            },
            BotError::CommandParse(cpe) => {
                let guild = if let Some(guild) = state.cache.guild(cpe.guild).await {
                    format!("guild `{}` ({})", guild.name.as_ref().map(|s| s.as_str()).unwrap_or("null"), guild.id)
                } else {
                    format!("unknown guild `{}`", cpe.guild)
                };
                let guard = self.commands.read().await;
                if let Some(guild_lock) = guard.get(&cpe.guild) {
                    let guard = guild_lock.read().await;
                    if let Some(command) = guard.get(&cpe.id).cloned() {
                        error!(
                            "Failed to parse command `{}` ({}) in {}: {:?}",
                            command.name(), cpe.id, guild, cpe.kind
                        );
                    } else {
                        error!(
                            "Failed to parse unknown command `{}` in {}: {:?}",
                            cpe.id, guild, cpe.kind,
                        );
                    }
                } else {
                    error!(
                        "Failed to parse command `{}` in {}, which has no commands: {:?}",
                        cpe.id, guild, cpe.kind,
                    );
                }
            }
            BotError::Chrono => error!("{}", BotError::Chrono)
        }
    }
}


async fn delete_command<F: Fn(&dyn SlashCommand) -> bool>(
    state: &BotState<Bot>,
    guild: GuildId,
    commands: &mut GuildCommands,
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

async fn create_command<C: SlashCommand>(
    state: &BotState<Bot>,
    guild: GuildId,
    commands: &mut GuildCommands,
    command: C,
) -> ClientResult<()> {
    let resp = state.client.create_guild_command(
        state.application_id().await,
        guild,
        command.command(),
    ).await?;
    commands.insert(resp.id, Box::new(command));

    Ok(())
}

impl Bot {
    async fn init_guild_commands(&self, guild: GuildId, state: &BotState<Bot>) -> ClientResult<()> {
        let commands = state.client.get_guild_commands(state.application_id().await, guild).await?;
        for command in commands {
            state.client.delete_guild_command(
                state.application_id().await,
                guild,
                command.id,
            ).await.unwrap();
        }
        let mut commands = self.commands.write().await;
        let mut commands = commands.entry(guild)
            .or_default()
            .write().await;
        let rcs = self.reaction_commands.write().await;
        // create_command(&*state, guild, &mut commands, UptimeCommand).await?;
        self.reset_guild_commands(&state, &mut commands, rcs, guild).await;
        Ok(())
    }

    async fn reset_guild_commands(
        &self,
        state: &BotState<Bot>,
        commands: &mut GuildCommands,
        mut reactions: RwLockWriteGuard<'_, Vec<Box<dyn ReactionCommand>>>,
        guild: GuildId,
    ) {
        reactions.retain(|rc| !AvalonGame::is_reaction_command(rc.as_ref(), guild));
        drop(reactions);
        let application = state.application_id().await;
        let old_commands = state.client.get_guild_commands(
            application,
            guild,
        ).await.unwrap();
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
        let new: Vec<Box<dyn SlashCommand>> = vec![
            Box::new(RolesCommand(vec![])),
            Box::new(AddMeCommand),
            Box::new(ToggleLady),
        ];
        let new = commands::create_guild_commands(&state, guild, new).await;
        commands.extend(new);
    }

    pub async fn most_recent_login(&self) -> Option<DateTime<Utc>> {
        if let Some(time) = *self.log_in.read().await {
            Some(time)
        } else {
            self.first_log_in.get().copied()
        }
    }

    pub async fn debug(&self) -> DebugBot<'_> {
        let Self { config, first_log_in: ready, log_in: resume, commands, reaction_commands, games, user_games } = self;
        let mut commands_read = HashMap::new();
        for (guild, commands) in commands.read().await.iter() {
            commands_read.insert(*guild, commands.read().await.clone());
        }
        #[allow(clippy::eval_order_dependence)]
        DebugBot {
            config,
            commands: commands_read,
            reaction_commands: reaction_commands.read().await,
            games: games.read().await,
            user_games: user_games.read().await,
            ready: ready.get(),
            resume: resume.read().await,
        }
    }
}

#[derive(Debug)]
pub struct DebugBot<'a> {
    config: &'a Config,
    commands: HashMap<GuildId, GuildCommands>,
    reaction_commands: RwLockReadGuard<'a, Vec<Box<dyn ReactionCommand>>>,
    games: RwLockReadGuard<'a, HashMap<GuildId, Avalon>>,
    user_games: RwLockReadGuard<'a, HashMap<UserId, HashSet<GuildId>>>,
    ready: Option<&'a DateTime<Utc>>,
    resume: RwLockReadGuard<'a, Option<DateTime<Utc>>>,
}