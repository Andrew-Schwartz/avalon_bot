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

use avalon::roles::RolesCommand;
use discorsd::{BotExt, BotState, shard};
use discorsd::{anyhow, async_trait};
use discorsd::http::channel::{ChannelExt, create_message, CreateMessage, embed};
use discorsd::http::ClientResult;
use discorsd::http::model::*;
use discorsd::shard::dispatch::ReactionUpdate;
use discorsd::shard::model::{Activity, ActivityType, Identify, StatusType, UpdateStatus};

use crate::avalon::{Avalon, toggle_lotl::ToggleLady};
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
        use std::time::Duration;

        loop {
            match File::create("uptime.txt").await {
                Ok(mut file) => match file.write_all(format!("{:?}", Utc::now()).as_bytes()).await {
                    Ok(()) => info!("Updated uptime file"),
                    Err(e) => error!("Error writing uptime file {}", e),
                }
                Err(e) => error!("Error opening uptime file {}", e),
            }

            // write file every 15 mins
            delay_for(Duration::from_secs(60 * 15)).await;
        }
    });

    let config = std::fs::read_to_string("config.json").expect("Could not find config.json");
    let config: Config = serde_json::from_str(&config).expect("Could not read config.json file");
    Bot::new(config).run().await
}

#[async_trait]
impl discorsd::Bot for Bot {
    type Error = anyhow::Error;

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

    async fn ready(&self, state: Arc<BotState<Self>>) -> anyhow::Result<()> {
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

    async fn resumed(&self, state: Arc<BotState<Self>>) -> anyhow::Result<()> {
        state.client.create_message(state.bot.config.channel, create_message(|m| {
            m.embed(|e| {
                e.title("Avalon Bot has resumed");
                e.timestamp_now();
            });
        })).await?;
        Ok(())
    }

    async fn guild_create(&self, guild: Guild, state: Arc<BotState<Self>>) -> anyhow::Result<()> {
        info!("Guild Create: {} ({})", guild.name.as_ref().unwrap(), guild.id);
        self.games.write().await.entry(guild.id).or_default();

        // let general = guild.channels.iter()
        //     .filter_map(|c| match c {
        //         Channel::Text(c) => Some(c),
        //         _ => None,
        //     })
        //     .find(|e| e.name == "general")
        //     .unwrap();
        // let user = state.user().await;
        // general.send(&state, "Commands are now enabled!").await?;
        // general.send(&state, embed(|e| {
        //     e.color(Color::GOLD);
        //     e.title("🎉 Big update for Avalon Bot! 🎉");
        //     e.description(format!("I have a whole new codebase, and am faster and more \
        //         error-proof than ever before! I also now use Discord's slash commands, so you can use \
        //         commands by typing `/roles add ...` (and have roles auto-completed) instead of having to \
        //         type out `!roles merlin morgana assassin percival` yourself like a total loser. Slash \
        //         commands do need separate authorization, so {} or someone else who has permissions \
        //         should click on the title of this message to give me those permissions.",
        //         guild.owner_id.ping_nick()
        //     ));
        //     e.url(format!(
        //         "https://discord.com/oauth2/authorize?scope=applications.commands%20bot&client_id={}&permissions={}&guild_id={}",
        //         user.id,
        //         67497024,
        //         guild.id,
        //     ))
        // })).await?;
        // return Ok(());

        let commands = state.client.get_guild_commands(state.application_id().await, guild.id).await?;
        for command in commands {
            state.client.delete_guild_command(
                state.application_id().await,
                guild.id,
                command.id,
            ).await.unwrap();
        }
        {
            let mut commands = self.commands.write().await;
            let mut commands = commands.entry(guild.id)
                .or_default()
                .write().await;
            let rcs = self.reaction_commands.write().await;
            // create_command(&*state, guild.id, &mut commands, UptimeCommand).await?;
            self.reset_guild_commands(&*state, &mut commands, rcs, guild.id).await;
        }

        if Utc::now().signed_duration_since(*self.first_log_in.get().unwrap()).num_seconds() >= 20 {
            self.config.channel.send(&state, format!(
                "🎉 Joined new guild **{}** (`{}`) 🎉",
                guild.name.unwrap(),
                guild.id,
            )).await?;
        }

        Ok(())
    }

    async fn message_create(&self, message: Message, state: Arc<BotState<Self>>) -> anyhow::Result<()> {
        match message.content.as_ref() {
            "!ping" => {
                let mut resp = state.client.create_message(message.channel_id, CreateMessage::build(|m| {
                    m.embed(|e| {
                        e.title("Pong");
                    });
                })).await?;
                let elapsed = Local::now().signed_duration_since(message.timestamp).to_std()?;
                let embed = resp.embeds.remove(0);
                resp.edit(&state, embed.build(|e| {
                    e.footer_text(format!("Took {:?} to respond", elapsed));
                })).await?;
            }
            "!timestamp" => {
                message.channel_id.send(
                    &state,
                    format!("You created your account at {}", message.author.id.timestamp()),
                ).await?;
            }
            "!lots" => {
                let user = state.user().await;
                message.channel_id.send(&state, embed(|e| {
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
                message.channel_id.send(&state, "logged!").await?;
            }
            "!cache" => {
                info!("{:#?}", state.cache.debug().await);
                message.channel_id.send(&state, "logged!").await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn interaction(&self, interaction: Interaction, state: Arc<BotState<Self>>) -> anyhow::Result<()> {
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

    async fn reaction(&self, reaction: ReactionUpdate, state: Arc<BotState<Self>>) -> anyhow::Result<()> {
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

    async fn error(&self, error: Self::Error) {
        panic!("{}", error)
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