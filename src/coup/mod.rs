use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::iter::zip;
use std::mem;
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use command_data_derive::{CommandDataChoices, MenuCommand};
use discorsd::{async_trait, BotState};
use discorsd::commands::{AppCommandData, ButtonCommand, InteractionPayload, InteractionUse, MenuCommand, MenuData, Unused, Usability, Used};
use discorsd::errors::BotError;
use discorsd::http::{ClientError, ClientResult, DiscordClient};
use discorsd::http::channel::{create_message, embed, MessageChannelExt, RichEmbed};
use discorsd::http::interaction::{webhook_message, WebhookMessage};
use discorsd::model::components::{ButtonStyle, make_button};
use discorsd::model::guild::GuildMember;
use discorsd::model::ids::{ChannelId, GuildId, Id, MessageId, UserId};
use discorsd::model::interaction::{ButtonPressData, MenuSelectData, Token};
use discorsd::model::interaction_response::{InteractionMessage, message};
use discorsd::model::message::{Color, Message, TextMarkup, TimestampMarkup, TimestampStyle};
use discorsd::model::user::UserMarkup;
use itertools::{Either, Itertools};
use rand::seq::SliceRandom;

use crate::Bot;
use crate::error::GameError;
use crate::utils::ListIterGrammatically;

async fn send_error<S, D, F>(
    state: S,
    interaction: InteractionUse<D, Unused>,
    embed: F,
) -> Result<InteractionUse<D, Used>, BotError<GameError>>
    where S: AsRef<DiscordClient> + Send,
          D: InteractionPayload,
          F: FnOnce(&mut RichEmbed) + Send,
{
    interaction.respond(state, message(|m| {
        m.ephemeral();
        m.embed(embed);
    })).await.map_err(Into::into)
}

async fn send_game_error<D: InteractionPayload, S: AsRef<DiscordClient> + Send>(
    state: S,
    interaction: InteractionUse<D, Unused>,
) -> Result<InteractionUse<D, Used>, BotError<GameError>> {
    send_error(state, interaction, |e| {
        e.title("Coup has already started in this server!");
        e.description("Each server can only have one game of Coup at a time");
        e.color(Color::RED);
    }).await
}

async fn send_config_error<D: InteractionPayload, S: AsRef<DiscordClient> + Send>(
    state: S,
    interaction: InteractionUse<D, Unused>,
) -> Result<InteractionUse<D, Used>, BotError<GameError>> {
    send_error(state, interaction, |e| {
        e.title("Coup has not yet started in this server!");
        e.description("Each server can only have one game of Coup at a time");
        e.color(Color::RED);
    }).await
}

async fn send_non_player_error<D, S, U>(
    state: S,
    interaction: InteractionUse<D, Unused>,
    user: U,
) -> Result<InteractionUse<D, Used>, BotError<GameError>>
    where D: InteractionPayload,
          S: AsRef<DiscordClient> + Send,
          U: Id<Id=UserId> + Send + Sync
{
    send_error(state, interaction, |e| {
        e.title("Invalid target");
        e.description(format!("{} is not in the game!", user.ping()));
        e.color(Color::RED);
    }).await
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, CommandDataChoices, MenuCommand)]
#[menu(skip_display)]
pub enum StartingCoins {
    Zero,
    One,
    #[command(default)]
    Two,
}

pub async fn start_setup(
    state: &BotState<Bot>,
    starting_coins: StartingCoins,
    guild: GuildId,
    interaction: InteractionUse<AppCommandData, Unused>,
) -> Result<InteractionUse<AppCommandData, Used>, BotError<GameError>> {
    let mut game_guard = state.bot.coup_games.write().await;
    let coup = game_guard
        .entry(guild)
        .or_default();
    let Coup::Config(config) = coup else {
        return send_game_error(&state, interaction).await;
    };
    let interaction = interaction.delete(state).await?;
    config.starting_coins = starting_coins;
    let member = interaction.source
        .guild_ref()
        .cloned()
        .expect("Guild Command")
        .member;
    config.players.insert(
        member.id(),
        (member, interaction.token.clone()),
    );
    config.update_settings_message(state, interaction.channel).await?;

    Ok(interaction)
}

#[derive(Debug)]
pub enum Coup {
    Config(CoupConfig),
    // boxed because its a much bigger variant
    Game(Box<CoupGame>),
}

impl Default for Coup {
    fn default() -> Self {
        Self::Config(Default::default())
    }
}

#[derive(Debug, Default)]
pub struct CoupConfig {
    pub players: HashMap<UserId, (GuildMember, Token)>,
    pub starting_coins: StartingCoins,
    pub settings_display: Option<Message>,
}

impl CoupConfig {
    fn can_start(&self) -> bool {
        let n_players = self.players.len();
        (2..=6).contains(&n_players)
    }

    pub async fn update_settings_message(
        &mut self,
        state: &BotState<Bot>,
        channel: ChannelId,
    ) -> ClientResult<()> {
        let message = create_message(|m| {
            m.embed(|e| {
                e.title("__Coup Setup__");
                e.color(Color::GOLD);
                let players_list = self.players.keys()
                    .map(UserId::ping)
                    .join("\n");
                e.add_field(
                    format!("Players ({})", self.players.len()),
                    if players_list.is_empty() {
                        "None yet".into()
                    } else {
                        players_list
                    },
                );
                e.add_field(
                    "Starting Coins",
                    self.starting_coins,
                );
            });
            m.menu(state, StartingCoinsMenu, |m| {
                m.min_values(1);
                m.default_options(|value| value == self.starting_coins.to_string());
            });
            m.buttons(state, [
                (Box::new(JoinLeaveButton(true)) as _, make_button(|b| b.label("Join game"))),
                (Box::new(JoinLeaveButton(false)) as _, make_button(|b| {
                    b.label("Leave game");
                    b.style(ButtonStyle::Danger);
                })),
            ]);
            m.button(state, StartButton, |b| {
                b.label("Start!");
                b.style(ButtonStyle::Success);
                if !self.can_start() {
                    b.disable();
                }
            });
        });
        match &mut self.settings_display {
            Some(settings) if settings.channel == channel => {
                // todo edit message or resend?
                // setting message already exists in this channel, so just update it
                settings.edit(&state, message).await?;
                // let new = settings.channel.send(&state, message).await?;
                // let old = mem::replace(settings, new);
                // old.delete(&state.client).await?;
            }
            no_settings => {
                let new = channel.send(state, message).await?;
                *no_settings = Some(new);
            }
        }
        Ok(())
    }

    async fn start_game(&mut self, state: Arc<BotState<Bot>>) -> ClientResult<CoupGame> {
        let starting_coins = self.starting_coins as usize;
        let mut cards = (0..15).map(|i| Card::from_int(i % 5)).collect_vec();
        {
            let mut rng = rand::thread_rng();
            cards.shuffle(&mut rng);
        }
        let mut cards = cards.chunks(2);
        let mut players = mem::take(&mut self.players)
            .into_iter()
            .map(|(_, (member, interaction_token))| CoupPlayer {
                member,
                token: interaction_token,
                coins: starting_coins,
                cards: cards.next().expect("6 (max players) * 2 < 15 (num_cards)").to_vec(),
                lost_cards: Vec::new(),
                cards_display: None,
                is_exchanging: None,
            })
            .collect_vec();
        {
            let mut rng = rand::thread_rng();
            players.shuffle(&mut rng);
            if players.len() == 2 {
                players[0].coins -= 1;
            }
        }

        let mut handles = Vec::new();
        for mut player in players.clone() {
            let state = Arc::clone(&state);
            let handle = tokio::spawn(async move {
                let msg = player.roles_message(&state).await?;
                let message = player.token.followup(&state, msg).await?;
                Ok(message.id)
            });
            handles.push(handle);
        }
        let messages = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|res| res.expect("awaiting response does not panic"))
            .collect::<ClientResult<Vec<_>>>()?;
        for (player, message) in zip(&mut players, messages) {
            player.cards_display = Some((player.token.clone(), message));
        }
        if let Some(settings) = &mut self.settings_display {
            settings.delete(&state).await?;
        }
        let coins = 50 - players.iter().map(|p| p.coins).sum::<usize>();
        let mut game = CoupGame {
            players,
            starting_coins: self.starting_coins,
            card_pile: cards.flatten().copied().collect_vec(),
            coins,
            idx: 0,
            wait_state: Default::default(),
            wait_idx: 0,
            start_game: None,
            start_turn: None,
            contest: None,
            block: None,
            contest_block: None,
            lose_influence: None,
            lost_influence: None,
            influence_pic: None,
            exchange_menu: None,
            ability_use: None,
        };
        game.get_edit_start_game(&state).await?;
        Ok(game)
    }
}

#[derive(Clone, Debug)]
struct StartingCoinsMenu;

#[async_trait]
impl MenuCommand for StartingCoinsMenu {
    type Bot = Bot;
    type Data = StartingCoins;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        mut data: Vec<Self::Data>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        {
            let guild = interaction.guild().unwrap();
            let mut games_guard = state.bot.coup_games.write().await;
            let coup = games_guard.get_mut(&guild)
                .expect("Coup setup has started");
            let Coup::Config(config) = coup else {
                return send_game_error(&state, interaction).await;
            };
            config.starting_coins = data.remove(0);
            config.update_settings_message(&state, interaction.channel).await?;
        }

        interaction.defer_update(state).await.map_err(Into::into)
    }
}

#[derive(Clone, Debug)]
struct JoinLeaveButton(bool);

#[async_trait]
impl ButtonCommand for JoinLeaveButton {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let mut games_guard = state.bot.coup_games.write().await;
        let coup = games_guard.get_mut(&guild)
            .expect("Coup setup has started");
        let Coup::Config(config) = coup else {
            return send_game_error(&state, interaction).await;
        };
        let member = interaction.source.guild_ref()
            .cloned()
            .expect("This button only exists in guilds")
            .member;
        if self.0 {
            config.players.insert(
                member.id(),
                (member, interaction.token.clone()),
            );
        } else {
            let was_not_in_game = config.players.remove(&member.id()).is_none();
            if was_not_in_game {
                return send_error(&state, interaction, |e| {
                    e.title("You weren't in the game, so you weren't removed");
                    e.color(Color::RED);
                }).await;
            }
        }
        config.update_settings_message(&state, interaction.channel).await?;

        drop(games_guard);
        interaction.defer_update(state).await.map_err(Into::into)
    }
}

#[derive(Clone, Debug)]
struct StartButton;

#[async_trait]
impl ButtonCommand for StartButton {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let mut games_guard = state.bot.coup_games.write().await;
        let coup = games_guard.get_mut(&guild)
            .expect("Game/Config must exist for StartButton to be shown");
        let Coup::Config(config) = coup else {
            return interaction.respond(&state, message(|m| {
                m.ephemeral();
                m.embed(|e| {
                    e.title("Coup has already started!");
                    e.color(Color::RED);
                });
            })).await.map_err(Into::into);
        };
        if !config.can_start() {
            let n_players = config.players.len();
            return interaction.respond(&state, message(|m| {
                m.embed(|e| {
                    e.title(if n_players < 2 { "Not enough players to start" } else { "Too many players to start" });
                    e.color(Color::RED);
                });
            })).await.map_err(Into::into);
        }

        let interaction = interaction.defer(&state).await?;
        let mut game = config.start_game(Arc::clone(&state)).await?;

        game.start_turn(&state).await?;
        *coup = Coup::Game(Box::new(game));

        interaction.delete(&state)
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug)]
pub struct CoupGame {
    players: Vec<CoupPlayer>,
    starting_coins: StartingCoins,
    card_pile: Vec<Card>,
    coins: usize,
    idx: usize,
    wait_state: WaitState,
    wait_idx: usize,
    start_game: Option<(Token, MessageId)>,
    start_turn: Option<(Token, MessageId)>,
    contest: Option<(Token, MessageId)>,
    block: Option<(Token, MessageId)>,
    contest_block: Option<Token>,
    lose_influence: Option<(Token, MessageId)>,
    lost_influence: Option<Token>,
    influence_pic: Option<Token>,
    exchange_menu: Option<(Token, MessageId)>,
    ability_use: Option<(Token, MessageId)>,
}

impl CoupGame {
    fn take_into_setup(&mut self) -> CoupConfig {
        let players = self.players
            .drain(..)
            .map(|p| (p.id(), (p.member, p.token)))
            .collect();
        CoupConfig {
            players,
            starting_coins: self.starting_coins,
            settings_display: None,
        }
    }

    fn current_player(&self) -> &CoupPlayer {
        &self.players[self.idx % self.players.len()]
    }
    fn current_player_mut(&mut self) -> &mut CoupPlayer {
        let len = self.players.len();
        &mut self.players[self.idx % len]
    }

    fn get_player(&self, user: UserId) -> Option<&CoupPlayer> {
        self.players.iter()
            .find(|p| p.id() == user)
    }

    fn get_player_mut(&mut self, user: UserId) -> Option<&mut CoupPlayer> {
        self.players.iter_mut()
            .find(|p| p.id() == user)
    }

    fn wait(&mut self, interactions: Vec<(Token, MessageId, UserId)>) -> usize {
        self.wait_state = WaitState::Waiting(interactions);
        self.wait_idx += 1;
        self.wait_idx
    }

    fn update_token<D, U>(&mut self, interaction: &InteractionUse<D, U>)
        where D: InteractionPayload,
              U: Usability
    {
        let player = self.players.iter_mut()
            .find(|p| p.id() == interaction.user().id);
        match player {
            Some(player) => {
                // println!("setting {} = {}", player.member.user.username, interaction.token);
                player.token = interaction.token.clone();
            }
            None => todo!("send error for user not in game using it")
        }
    }

    async fn delete_message(state: &BotState<Bot>, message: Option<(Token, MessageId)>) -> ClientResult<()> {
        if let Some((token, id)) = message {
            // println!("delete {token}");
            state.client.delete_followup_message(state.application_id(), token, id).await?;
        }
        Ok(())
    }

    async fn get_edit_start_game(&mut self, state: &BotState<Bot>) -> ClientResult<()> {
        let player = self.current_player();
        let embed = embed(|e| {
            e.title("Coup!");
            e.color(Color::GOLD);
            e.add_field(
                "Turn order",
                self.players.iter()
                    .enumerate()
                    .map(|(i, player)| {
                        let field_description = format!(
                            "{}: {}    {} coin{}{}",
                            i + 1,
                            player.ping(),
                            player.coins,
                            if player.coins == 1 { "" } else { "s" },
                            if player.lost_cards.is_empty() {
                                String::new()
                            } else {
                                format!("    Revealed: {}", player.lost_cards.iter().list_grammatically(Card::to_string, "and"))
                            }
                        );
                        if player.cards.is_empty() {
                            field_description.strikethrough()
                        } else {
                            field_description
                        }
                    })
                    .join("\n"),
            );
            e.add_inline_field(
                "Cards in Court Deck",
                self.card_pile.len(),
            );
            e.add_blank_inline_field();
            e.add_inline_field(
                "Coins left",
                self.coins,
            );
            e.description(format!("{}, take your turn!", player.ping()));
        });
        if let Some((token, id)) = &self.start_game {
            // already exists, so edit the message
            state.client.edit_followup_message(
                state.application_id(),
                token.clone(),
                *id,
                embed.into(),
            ).await?;
        } else {
            // first time, so send the message
            // todo handle if someone deletes the message
            let message = player.token
                .followup(&state, embed)
                .await?;
            self.start_game = Some((player.token.clone(), message.id));
        }
        Ok(())
    }

    fn start_turn_message(state: &BotState<Bot>, coins: usize, get_target_for: Option<Ability>) -> InteractionMessage {
        message(|m| {
            m.ephemeral();
            m.content(format!("Take an action! You have {} coins.", coins.to_string().bold()));
            m.menu(state, AbilityMenu, |m| {
                m.placeholder("Pick an ability");
                m.options(Ability::all()
                    .into_iter()
                    .filter(|a| a.needed_coins() <= coins)
                    .map(Ability::into_option)
                    .collect()
                );
                if let Some(ability) = get_target_for {
                    m.default_options(|s| s == ability.to_string());
                }
            });
            if let Some(ability) = get_target_for {
                m.menu(state, AbilityTargetMenu(ability), |m| {
                    m.placeholder("Target of the ability");
                });
            }
        })
    }

    async fn start_turn(&mut self, state: &BotState<Bot>) -> ClientResult<()> {
        let player = self.current_player();
        let message = player.token
            .followup(state, Self::start_turn_message(state, player.coins, None))
            .await?;
        self.start_turn = Some((player.token.clone(), message.id));
        Ok(())
    }

    // todo check if game is over
    async fn next_turn(&mut self, state: &BotState<Bot>) -> ClientResult<()> {
        self.idx += 1;
        while self.current_player().cards.is_empty() {
            self.idx += 1;
        }
        if self.players.iter().filter(|p| !p.cards.is_empty()).count() == 1 {
            // only one player left, game is over!
            let winner = self.current_player();
            winner.token.followup(&state, winner.win_message(state, true)).await?;
        } else {
            Self::delete_message(state, self.start_turn.take()).await?;
            self.get_edit_start_game(state).await?;
            self.start_turn(state).await?;
        }
        Ok(())
    }

    async fn resolve_ability(
        &mut self,
        state: &BotState<Bot>,
        ability: FullAbility,
    ) -> ClientResult<()> {
        Self::delete_message(state, self.ability_use.take()).await?;
        match ability {
            FullAbility::Use(ability) => match ability.ability {
                AbilityTargeted::Income => {
                    if let Some(coins) = self.coins.checked_sub(1) {
                        // only take a coin if there was a coin left
                        self.coins = coins;
                        self.current_player_mut().coins += 1;
                    }
                    let player = self.current_player();
                    let message = player.token.followup(&state, ability.to_string()).await?;
                    self.ability_use = Some((player.token.clone(), message.id));
                    self.next_turn(state).await?;
                    Ok(())
                }
                AbilityTargeted::ForeignAid => {
                    if let Some(coins) = self.coins.checked_sub(2) {
                        // only take 2 coins if there are 2 coins left
                        self.coins = coins;
                        self.current_player_mut().coins += 2;
                    } else {
                        self.current_player_mut().coins += self.coins;
                        self.coins = 0;
                    }
                    let player = self.current_player();
                    let message = player.token.followup(&state, ability.to_string()).await?;
                    self.ability_use = Some((player.token.clone(), message.id));
                    self.next_turn(state).await?;
                    Ok(())
                }
                AbilityTargeted::Tax => {
                    if let Some(coins) = self.coins.checked_sub(3) {
                        // only take 3 coins if there are 3 coins left
                        self.coins = coins;
                        self.current_player_mut().coins += 3;
                    } else {
                        self.current_player_mut().coins += self.coins;
                        self.coins = 0;
                    }
                    let player = self.current_player();
                    let message = player.token.followup(&state, ability.to_string()).await?;
                    self.ability_use = Some((player.token.clone(), message.id));
                    self.next_turn(state).await?;
                    Ok(())
                }
                AbilityTargeted::Coup(target) => {
                    self.current_player_mut().coins -= 7;
                    let target = self.get_player(target).expect("Target in game");
                    let message = LostInfluenceMenu::create(state, target, None).await?;
                    self.lose_influence = Some((target.token.clone(), message.id));
                    let player = self.current_player();
                    let message = player.token.followup(&state, ability.to_string()).await?;
                    self.ability_use = Some((player.token.clone(), message.id));
                    Ok(())
                }
                AbilityTargeted::Assassinate(target) => {
                    self.current_player_mut().coins -= 3;
                    let target = self.get_player(target).expect("Target in game");
                    let message = LostInfluenceMenu::create(state, target, None).await?;
                    self.lose_influence = Some((target.token.clone(), message.id));
                    let player = self.current_player();
                    let message = player.token.followup(&state, ability.to_string()).await?;
                    self.ability_use = Some((player.token.clone(), message.id));
                    Ok(())
                }
                AbilityTargeted::Exchange => {
                    let cards = mem::take(&mut self.current_player_mut().cards)
                        .into_iter()
                        .chain(self.card_pile.drain(..2))
                        .collect_vec();
                    let n_keep = cards.len() - 2;
                    let player = self.current_player_mut();
                    let message = player.token.followup(&state, format!("{} is choosing cards to Exchange...", player.ping()))
                        .await?;
                    player.is_exchanging = Some((player.token.clone(), message.id));
                    let message = player.token
                        .followup(&state, webhook_message(|m| {
                            m.ephemeral();
                            m.content(format!("Choose {n_keep} roles to **keep**:"));
                            cards.iter()
                                .copied()
                                .map(Card::image)
                                .enumerate()
                                // named so that all attachments appear if any cards are the same
                                .for_each(|(i, path)| m.attach((format!("role{i}.png"), path)));
                            let options = cards.iter()
                                .enumerate()
                                .map(|(i, c)| {
                                    let mut option = c.into_option();
                                    option.label = format!("{i}: {}", option.label);
                                    option.value = i.to_string();
                                    option
                                })
                                .collect();
                            m.menu(state, ExchangeMenu(cards), |m| {
                                m.min_max_values(n_keep, n_keep);
                                m.options(options);
                            });
                        }))
                        .await?;
                    self.exchange_menu = Some((player.token.clone(), message.id));
                    Ok(())
                }
                AbilityTargeted::Steal(target) => {
                    let target = self.get_player_mut(target).expect("Target in game");
                    if let Some(coins) = target.coins.checked_sub(2) {
                        // only take 2 coins if target has 2 coins
                        target.coins = coins;
                        self.current_player_mut().coins += 2;
                    } else {
                        let target_coins = target.coins;
                        target.coins = 0;
                        self.current_player_mut().coins += target_coins;
                    }
                    let player = self.current_player();
                    let message = player.token.followup(&state, ability.to_string()).await?;
                    self.ability_use = Some((player.token.clone(), message.id));
                    self.next_turn(state).await?;
                    Ok(())
                }
            }
            FullAbility::Block(_, _, _) => {
                let player = self.current_player();
                let message = player.token.followup(&state, ability.to_string()).await?;
                self.ability_use = Some((player.token.clone(), message.id));
                self.next_turn(state).await?;
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug)]
struct RestartButton;

#[async_trait]
impl ButtonCommand for RestartButton {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();

        let mut game_guard = state.bot.coup_games.write().await;
        let coup = game_guard.get_mut(&guild).unwrap();
        let Coup::Game(game) = coup else {
            return send_config_error(&state, interaction).await;
        };
        // game.update_token(&interaction);2
        let win_message = game.current_player().win_message(&state, false);

        let mut config = game.take_into_setup();
        config.update_settings_message(&state, interaction.channel).await?;
        *coup = Coup::Config(config);

        interaction.update(&state, win_message).await.map_err(Into::into)
    }
}

#[derive(Clone, Debug)]
struct ExchangeMenu(Vec<Card>);

#[async_trait]
impl MenuCommand for ExchangeMenu {
    type Bot = Bot;
    type Data = String;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        data: Vec<Self::Data>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();

        let mut game_guard = state.bot.coup_games.write().await;
        let Coup::Game(game) = game_guard.get_mut(&guild).unwrap() else {
            return send_config_error(&state, interaction).await;
        };

        let (mut keep, mut retrn) = (Vec::new(), Vec::new());
        for (i, &card) in self.0.iter().enumerate() {
            match data.contains(&i.to_string()) {
                true => keep.push(card),
                false => retrn.push(card),
            }
        }
        game.card_pile.extend(retrn);

        let player = game.current_player_mut();
        player.cards = keep;
        player.send_roles(&state).await?;
        CoupGame::delete_message(&state, player.is_exchanging.take()).await?;
        let message = player.token
            .followup(&state, AbilityResolved { user: player.id(), ability: AbilityTargeted::Exchange }.to_string())
            .await?;
        let token = player.token.clone();

        let ability_use = &mut game.ability_use;
        CoupGame::delete_message(&state, ability_use.take()).await?;
        *ability_use = Some((token, message.id));

        CoupGame::delete_message(&state, game.exchange_menu.take()).await?;
        game.next_turn(&state).await?;
        interaction.defer_update(&state).await.map_err(Into::into)
    }
}

#[derive(Debug, Default, Clone)]
enum WaitState {
    /// Not waiting. Starts like this, and when a contest/block happens, gets reset to this
    #[default]
    None,
    /// Counting down, players have a button to pause
    Waiting(Vec<(Token, MessageId, UserId)>),
    /// Someone pressed the pause button
    Paused(Vec<(Token, MessageId, UserId)>),
    /// The countdown is done, and if unpaused, don't have to keep waiting
    PausedDone(Vec<(Token, MessageId, UserId)>),
}

impl WaitState {
    async fn delete_messages(&mut self, state: &BotState<Bot>) -> ClientResult<()> {
        match self {
            Self::None => {}
            Self::Waiting(interactions)
            | Self::Paused(interactions)
            | Self::PausedDone(interactions) => {
                let app = state.application_id();
                for (token, id, _) in interactions.drain(..) {
                    state.client.delete_followup_message(app, token, id).await?;
                }
            }
        };
        *self = Self::None;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct AbilityMenu;

#[derive(MenuCommand, Copy, Clone, Debug, PartialEq, Eq)]
enum Ability {
    #[menu(desc = "Take 1 coin")]
    Income,
    #[menu(label = "Foreign Aid", desc = "Take 2 coins. Can be blocked by Duke")]
    ForeignAid,
    #[menu(desc = "Pay 7 coins. Choose player to lose influence")]
    Coup,
    #[menu(desc = "Take 3 coins")]
    Tax,
    #[menu(desc = "Pay 3 coins. Choose player to lose influence. Can be blocked by Contessa")]
    Assassinate,
    #[menu(desc = "Exchange cards with Court Deck")]
    Exchange,
    #[menu(desc = "Take 2 coins from another player. Can be blocked by Ambassador or Captain")]
    Steal,
}

impl Ability {
    fn needed_coins(self) -> usize {
        match self {
            Self::Income
            | Self::ForeignAid
            | Self::Tax
            | Self::Exchange
            | Self::Steal => 0,
            Self::Assassinate => 3,
            Self::Coup => 7,
        }
    }

    fn target(self, target: Option<UserId>) -> Option<AbilityTargeted> {
        Some(match self {
            Self::Income => AbilityTargeted::Income,
            Self::ForeignAid => AbilityTargeted::ForeignAid,
            Self::Coup => AbilityTargeted::Coup(target?),
            Self::Tax => AbilityTargeted::Tax,
            Self::Assassinate => AbilityTargeted::Assassinate(target?),
            Self::Exchange => AbilityTargeted::Exchange,
            Self::Steal => AbilityTargeted::Steal(target?),
        })
    }

    /// resolve this ability to state that it can then be countered/contested
    async fn get_target(
        &self,
        state: &BotState<Bot>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        coins: usize,
    ) -> ClientResult<InteractionUse<MenuSelectData, Used>> {
        match self {
            &ability @ Self::Coup
            | &ability @ Self::Assassinate
            | &ability @ Self::Steal =>
            // enable target box
                interaction
                    .update(state, CoupGame::start_turn_message(state, coins, Some(ability)))
                    .await,
            Self::Income
            | Self::ForeignAid
            | Self::Tax
            | Self::Exchange => interaction.defer_update(state).await,
        }
    }

    fn contest_block_embed<F: FnOnce(&mut WebhookMessage)>(
        state: &BotState<Bot>,
        ability_desc: &str,
        countdown_or_pauser: Either<DateTime<Utc>, UserId>,
        button: Either<WaitButton, UnpauseButton>,
        send_pause_button: bool,
        make_components: F,
    ) -> WebhookMessage {
        webhook_message(|m| {
            m.ephemeral();
            let content = match countdown_or_pauser {
                Either::Left(expire_time) => format!(
                    "{ability_desc}. The action will go through {}",
                    (expire_time).timestamp_styled(TimestampStyle::Relative)
                ),
                Either::Right(pauser) => format!(
                    "{ability_desc}. {} paused the countdown",
                    pauser.ping()
                )
            };
            m.content(content);
            if send_pause_button {
                // skip sending button to the player what is being blocked/contested/etc
                match button {
                    Either::Left(wait) => m.button(state, wait, |b| {
                        b.label("Wait!!!");
                    }),
                    Either::Right(unpause) => m.button(state, unpause, |b| {
                        b.label("Proceed!");
                    }),
                }
                make_components(m);
            }
        })
    }
}

#[derive(Debug, Copy, Clone)]
enum AbilityTargeted {
    Income,
    ForeignAid,
    Coup(UserId),
    Tax,
    Assassinate(UserId),
    Exchange,
    Steal(UserId),
}

#[derive(Debug, Copy, Clone)]
struct AbilityResolved {
    user: UserId,
    ability: AbilityTargeted,
}

impl AbilityResolved {
    fn counter_roles(self) -> Option<&'static [Card]> {
        match self.ability {
            AbilityTargeted::Income
            | AbilityTargeted::Coup(_)
            | AbilityTargeted::Tax
            | AbilityTargeted::Exchange => None,
            AbilityTargeted::ForeignAid => Some(&[Card::Duke]),
            AbilityTargeted::Assassinate(_) => Some(&[Card::Contessa]),
            AbilityTargeted::Steal(_) => Some(&[Card::Ambassador, Card::Captain]),
        }
    }

    fn needed_card(self) -> Option<Card> {
        match self.ability {
            AbilityTargeted::Income | AbilityTargeted::ForeignAid | AbilityTargeted::Coup(_) => None,
            AbilityTargeted::Tax => Some(Card::Duke),
            AbilityTargeted::Assassinate(_) => Some(Card::Assassin),
            AbilityTargeted::Exchange => Some(Card::Ambassador),
            AbilityTargeted::Steal(_) => Some(Card::Captain),
        }
    }
}

impl Display for AbilityResolved {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let user = self.user.ping();
        match self.ability {
            AbilityTargeted::Income => write!(f, "{user} took Income"),
            AbilityTargeted::ForeignAid => write!(f, "{user} took Foreign Aid"),
            AbilityTargeted::Coup(target) => write!(f, "{user} Couped {}", target.ping()),
            AbilityTargeted::Tax => write!(f, "{user} Taxed"),
            AbilityTargeted::Assassinate(target) => write!(f, "{user} Assassinated {}", target.ping()),
            AbilityTargeted::Exchange => write!(f, "{user} Exchanged with the court deck"),
            AbilityTargeted::Steal(target) => write!(f, "{user} Stole from {}", target.ping()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum FullAbility {
    Use(AbilityResolved),
    Block(AbilityResolved, UserId, Card),
}

impl Display for FullAbility {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Use(ability) => write!(f, "{ability}"),
            Self::Block(ability, user, claim) => {
                write!(f, "{ability}\n{} blocks with {claim}", user.ping())
            }
        }
    }
}

impl FullAbility {
    fn is_use(self) -> bool {
        matches!(self, Self::Use(_))
    }

    fn is_block(self) -> bool {
        matches!(self, Self::Block(_, _, _))
    }

    fn ability(self) -> AbilityResolved {
        match self {
            Self::Use(a) | Self::Block(a, _, _) => a,
        }
    }

    fn user(self) -> UserId {
        match self {
            Self::Use(a) => a.user,
            Self::Block(_, u, _) => u,
        }
    }

    fn counter_roles(self) -> Option<&'static [Card]> {
        match self {
            Self::Use(a) => a.counter_roles(),
            Self::Block(_, _, _) => None,
        }
    }

    fn needed_card(self) -> Option<Card> {
        match self {
            Self::Use(a) => a.needed_card(),
            Self::Block(_, _, c) => Some(c),
        }
    }

    async fn prompt_response(
        self,
        state: &Arc<BotState<Bot>>,
        game: &mut CoupGame,
        interaction: InteractionUse<MenuSelectData, Unused>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        CoupGame::delete_message(state, game.start_turn.take()).await?;
        if let Some(token) = game.influence_pic.take() {
            state.client.delete_interaction_response(state.application_id(), token).await?;
        }

        if self.counter_roles().is_some() || self.needed_card().is_some() {
            let current_player = &game.current_player().member;
            let current_player_id = current_player.id();
            let current_player_name = current_player.nick.clone().unwrap_or_else(|| current_player.user.username.clone());

            // give all players 5 seconds to either block, contest, or click the "considering" button
            let wait_time = Duration::seconds(6);
            let expire_time = Utc::now() + wait_time;
            let mut handles = Vec::new();
            for player in game.players.clone() {
                let state = Arc::clone(state);
                let current_player_name = current_player_name.clone();
                let handle = tokio::spawn(async move {
                    let message = player.token.followup(&state, Ability::contest_block_embed(
                        &state,
                        &self.to_string(),
                        Either::Left(expire_time),
                        Either::Left(WaitButton {
                            ability: self,
                        }),
                        current_player_id != player.id(),
                        |m| {
                            if let Some(counter_roles) = self.counter_roles() {
                                m.menu(
                                    &state,
                                    BlockMenu { ability: self.ability() },
                                    |m| {
                                        m.placeholder("Block with...");
                                        m.options(counter_roles.iter().copied().map(Card::into_option).collect());
                                    },
                                );
                            }
                            if let Some(claim) = self.needed_card() {
                                m.button(&state, ContestButton {
                                    ability: self,
                                    claim,
                                    claimer: current_player_id,
                                }, |b| {
                                    b.label(format!("Contest that {} has {claim}", current_player_name));
                                    b.style(ButtonStyle::Danger);
                                });
                            }
                        },
                    )).await?;
                    Ok((player.token.clone(), message.id, player.id()))
                });
                handles.push(handle);
            }
            let interactions = futures::future::join_all(handles)
                .await
                .into_iter()
                .map(|res| res.expect("awaiting response does not panic"))
                .collect::<ClientResult<Vec<_>>>()?;
            let wait_idx = game.wait(interactions);
            tokio::spawn({
                let state = Arc::clone(state);
                async move {
                    tokio::time::sleep(wait_time.to_std().unwrap()).await;

                    let mut game_guard = state.bot.coup_games.write().await;
                    let Coup::Game(game) = game_guard.get_mut(&guild).unwrap() else {
                        todo!()
                        // send_config_error(&state, interaction).await?;
                    };
                    if game.wait_idx != wait_idx {
                        return Ok(());
                    }
                    match &mut game.wait_state {
                        WaitState::None => {
                            // someone already used something, so don't do anything
                        }
                        wait_state @ WaitState::Waiting(_) => {
                            // if the game is still waiting, we're now done waiting
                            wait_state.delete_messages(&state).await?;
                            CoupGame::delete_message(&state, game.contest.take()).await?;
                            CoupGame::delete_message(&state, game.block.take()).await?;
                            game.resolve_ability(&state, Self::Use(self.ability())).await?;
                        }
                        WaitState::Paused(interactions) => {
                            // its currently paused, so just mark that the countdown is done
                            game.wait_state = WaitState::PausedDone(mem::take(interactions));
                        }
                        WaitState::PausedDone(_) => {
                            unreachable!("Can only be here done was paused before sleep finished")
                        }
                    };
                    Ok::<(), ClientError>(())
                }
            });
        } else {
            game.resolve_ability(state, Self::Use(self.ability())).await?;
        }
        interaction.defer_update(&state).await.map_err(Into::into)
    }
}

#[async_trait]
impl MenuCommand for AbilityMenu {
    type Bot = Bot;
    type Data = Ability;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        mut data: Vec<Self::Data>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let ability = data.remove(0);
        let mut games_guard = state.bot.coup_games.write().await;
        let coup = games_guard.get_mut(&guild).unwrap();
        let Coup::Game(game) = coup else {
            return send_config_error(&state, interaction).await;
        };
        // todo for some reason this token doesn't work
        // game.update_token(&interaction);
        game.wait_state.delete_messages(&state).await?;

        if let Some(ability) = ability.target(None) {
            // retargeted
            let ability = FullAbility::Use(AbilityResolved { user: interaction.user().id, ability });
            ability.prompt_response(&state, game, interaction)
                .await
        } else {
            // get target
            ability.get_target(&state, interaction, game.current_player().coins)
                .await
                .map_err(Into::into)
        }
    }
}

#[derive(Debug, Clone)]
struct WaitButton {
    ability: FullAbility,
}

#[async_trait]
impl ButtonCommand for WaitButton {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let interaction_user = interaction.user().id;

        let mut game_guard = state.bot.coup_games.write().await;
        let Coup::Game(game) = game_guard.get_mut(&guild).unwrap() else {
            return send_config_error(&state, interaction).await;
        };
        game.update_token(&interaction);

        let ability_user_name = game.get_player(self.ability.ability().user).unwrap().name();
        match &mut game.wait_state {
            WaitState::None => {
                println!("WS None!");
                // means a user clicked the button before it was deleted but after we send to req
                // to delete it, aka they were too slow. so don't do anything
            }
            WaitState::Paused(_) => todo!(),
            WaitState::PausedDone(_) => todo!(),
            WaitState::Waiting(interactions) => {
                let mut handles = Vec::new();
                let app = state.application_id();
                for (token, id, receiver) in interactions.clone() {
                    let state = Arc::clone(&state);
                    let ability = self.ability;
                    let ability_user_name = ability_user_name.clone();
                    let handle = tokio::spawn(async move {
                        state.client.edit_followup_message(
                            app,
                            token.clone(),
                            id,
                            Ability::contest_block_embed(
                                &state,
                                &ability.to_string(),
                                Either::Right(interaction_user),
                                Either::Right(UnpauseButton(ability)),
                                receiver != ability.user(),
                                |m| {
                                    if let Some(counter_roles) = ability.counter_roles() {
                                        m.menu(&state, BlockMenu { ability: ability.ability() }, |m| {
                                            m.placeholder("Block with...");
                                            m.options(counter_roles.iter().copied().map(Card::into_option).collect());
                                        });
                                    }
                                    if let Some(claim) = ability.needed_card() {
                                        m.button(&state, ContestButton {
                                            ability,
                                            claim,
                                            claimer: ability.ability().user,
                                        }, |b| {
                                            b.label(format!("Contest that {} has {claim}", ability_user_name));
                                            b.style(ButtonStyle::Danger);
                                        });
                                    }
                                },
                            ),
                        ).await?;
                        Ok(())
                    });
                    handles.push(handle);
                }
                futures::future::join_all(handles)
                    .await
                    .into_iter()
                    .map(|res| res.expect("awaiting response does not panic"))
                    .collect::<ClientResult<Vec<()>>>()?;

                game.wait_state = WaitState::Paused(mem::take(interactions));
            }
        }

        interaction.defer_update(&state)
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
struct UnpauseButton(FullAbility);

#[async_trait]
impl ButtonCommand for UnpauseButton {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();

        let mut game_guard = state.bot.coup_games.write().await;
        let Coup::Game(game) = game_guard.get_mut(&guild).unwrap() else {
            return send_config_error(&state, interaction).await;
        };
        // todo this doesn't work???
        // game.update_token(&interaction);

        match &mut game.wait_state {
            WaitState::None => todo!(),
            WaitState::Waiting(_) => unreachable!("?"),
            WaitState::Paused(interactions) => {
                game.wait_state = WaitState::Waiting(mem::take(interactions));
                interaction.defer_update(&state).await.map_err(Into::into)
            }
            wait_state @ WaitState::PausedDone(_) => {
                wait_state.delete_messages(&state).await?;
                game.resolve_ability(&state, FullAbility::Use(self.0.ability())).await?;
                interaction.defer_update(&state).await.map_err(Into::into)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct AbilityTargetMenu(Ability);

#[async_trait]
impl MenuCommand for AbilityTargetMenu {
    type Bot = Bot;
    type Data = UserId;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        mut data: Vec<Self::Data>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let target = data.remove(0);

        let mut games_guard = state.bot.coup_games.write().await;
        let Coup::Game(game) = games_guard.get_mut(&guild).unwrap() else {
            return send_config_error(&state, interaction).await;
        };
        // game.update_token(&interaction);

        if !game.players.iter().any(|p| p.id() == target) {
            return send_error(&state, interaction, |e| {
                e.color(Color::RED);
                e.title("Choose someone in the game!");
            }).await;
        }

        if target == game.current_player().id() {
            return send_error(&state, interaction, |e| {
                e.color(Color::RED);
                e.title("You can't target yourself!");
            }).await;
        }

        let ability = self.0.target(Some(target)).unwrap();
        let ability = FullAbility::Use(AbilityResolved { user: interaction.user().id, ability });
        ability.prompt_response(&state, game, interaction)
            .await
    }
}

#[derive(Debug, Clone, Copy)]
struct BlockMenu {
    ability: AbilityResolved,
}

#[async_trait]
impl MenuCommand for BlockMenu {
    type Bot = Bot;
    type Data = Card;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        mut data: Vec<Self::Data>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let blocker = interaction.user().id;
        let claim = data.remove(0);
        let mut games_guard = state.bot.coup_games.write().await;
        let Coup::Game(game) = games_guard.get_mut(&guild).unwrap() else {
            return send_config_error(&state, interaction).await;
        };
        // game.update_token(&interaction);
        game.wait_state.delete_messages(&state).await?;

        let Some(blocker) = game.get_player(blocker) else {
            return send_non_player_error(&state, interaction, blocker).await;
        };
        let blocker_id = blocker.id();
        let ability = FullAbility::Block(self.ability, blocker_id, claim);

        // give all players 5 seconds to either block, contest, or click the "considering" button
        let wait_time = Duration::seconds(6);
        let expire_time = Utc::now() + wait_time;
        let mut handles = Vec::new();
        for player in game.players.clone() {
            let state = Arc::clone(&state);
            let blocker_name = blocker.member.nick.clone().unwrap_or_else(|| blocker.member.user.username.clone());
            let player_id = player.id();
            let handle = tokio::spawn(async move {
                let message = player.token.followup(
                    &state,
                    Ability::contest_block_embed(
                        &state,
                        &ability.to_string(),
                        Either::Left(expire_time),
                        Either::Left(WaitButton { ability }),
                        blocker_id != player_id,
                        |m| {
                            m.button(&state, ContestButton {
                                ability,
                                claim,
                                claimer: blocker_id,
                            }, |b| {
                                b.label(format!("Contest that {} has {claim}", blocker_name));
                                b.style(ButtonStyle::Danger);
                            });
                        },
                    ),
                ).await?;
                Ok((player.token.clone(), message.id, player.id()))
            });
            handles.push(handle);
        };
        let interactions = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|res| res.expect("awaiting response does not panic"))
            .collect::<ClientResult<Vec<_>>>()?;
        let wait_idx = game.wait(interactions);
        tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                tokio::time::sleep(wait_time.to_std().unwrap()).await;

                let mut game_guard = state.bot.coup_games.write().await;
                let Coup::Game(game) = game_guard.get_mut(&guild).unwrap() else {
                    todo!()
                    // send_config_error(&state, interaction).await?;
                };
                if game.wait_idx != wait_idx {
                    return Ok(());
                }

                match &mut game.wait_state {
                    WaitState::None => {
                        // someone already used something, so don't do anything
                    }
                    wait_state @ WaitState::Waiting(_) => {
                        // if the game is still waiting, we're now done waiting
                        wait_state.delete_messages(&state).await?;
                        CoupGame::delete_message(&state, game.contest.take()).await?;
                        CoupGame::delete_message(&state, game.block.take()).await?;
                        game.resolve_ability(&state, ability).await?;
                    }
                    WaitState::Paused(interactions) => {
                        // its currently paused, so just mark that the countdown is done
                        game.wait_state = WaitState::PausedDone(mem::take(interactions));
                    }
                    WaitState::PausedDone(_) => {
                        unreachable!("Can only be here done was paused before sleep finished")
                    }
                };
                Ok::<(), ClientError>(())
            }
        });

        CoupGame::delete_message(&state, game.block.take()).await?;
        CoupGame::delete_message(&state, game.contest.take()).await?;
        game.contest_block = Some(interaction.token.clone());
        Ok(interaction.defer_update(&state).await?)
    }
}


#[derive(Debug, Clone)]
struct ContestButton {
    ability: FullAbility,
    claim: Card,
    claimer: UserId,
}

#[async_trait]
impl ButtonCommand for ContestButton {
    type Bot = Bot;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<ButtonPressData, Unused>,
    ) -> Result<InteractionUse<ButtonPressData, Used>, BotError<GameError>> {
        let guild = interaction.guild().unwrap();
        let contester = interaction.user().id;

        let mut game_guard = state.bot.coup_games.write().await;
        let Coup::Game(game) = game_guard.get_mut(&guild).unwrap() else {
            return send_config_error(&state, interaction).await;
        };
        game.update_token(&interaction);
        game.wait_state.delete_messages(&state).await?;

        let Some(contester) = game.get_player(contester) else {
            return send_non_player_error(&state, interaction, &contester).await;
        };
        let contester_token = contester.token.clone();

        let claimer = game.get_player(self.claimer).unwrap();
        let claimer_token = claimer.token.clone();
        let interaction = if claimer.cards.contains(&self.claim) {
            // does have the card, so contester loses an influence
            let content = format!(
                "{c} contested that {} had {}, but they did!\n{c} will now lose an influence.",
                self.claimer.ping(),
                self.claim,
                c = contester.ping(),
            );
            let interaction = interaction.respond(&state, content).await?;
            // give the contester a menu to choose which influence to lose
            // does have the card, so it should resolve if it's a use, not if its a block (?)
            let ability = self.ability.is_use().then_some(self.ability);
            let message = LostInfluenceMenu::create(&state, contester, ability).await?;
            // todo idk if this is right?
            game.lost_influence = Some(interaction.token.clone());
            game.lose_influence = Some((contester_token, message.id));
            {
                // the claimer draws a new influence now
                let claimer_idx = game.players.iter().position(|p| p.id() == self.claimer).unwrap();
                let mut claimer = game.players.remove(claimer_idx);
                let card_idx = claimer.cards.iter().position(|c| *c == self.claim).unwrap();
                let card = claimer.cards.remove(card_idx);
                game.card_pile.push(card);
                {
                    let mut rng = rand::thread_rng();
                    game.card_pile.shuffle(&mut rng);
                }
                let new_card = game.card_pile.swap_remove(0);
                claimer.cards.push(new_card);
                claimer.send_roles(&state).await?;
                game.players.insert(claimer_idx, claimer);
            }
            interaction
        } else {
            // does not have the card, so claimer loses an influence
            let content = format!(
                "{} contested that {c} had {}, and they didn't!\n{c} will now lose an influence.",
                contester.ping(),
                self.claim,
                c = self.claimer.ping(),
            );
            let interaction = interaction.respond(&state, content).await?;
            // give the claimer a menu to choose which influence to lose
            // doesn't have the card, so it should resolve if it's a use, not if its a block (?)
            let ability = self.ability.is_block().then_some(self.ability.ability()).map(FullAbility::Use);
            let message = LostInfluenceMenu::create(&state, claimer, ability).await?;
            // todo idk if this is right?
            game.lost_influence = Some(interaction.token.clone());
            game.lose_influence = Some((claimer_token, message.id));
            interaction
        };
        CoupGame::delete_message(&state, game.block.take()).await?;
        CoupGame::delete_message(&state, game.contest.take()).await?;
        if let Some(token) = game.contest_block.take() {
            state.client.delete_interaction_response(state.application_id(), token).await?;
        }
        game.lost_influence = Some(interaction.token.clone());
        Ok(interaction)
    }
}

#[derive(Debug, Clone)]
struct LostInfluenceMenu(UserId, Option<FullAbility>);

impl LostInfluenceMenu {
    async fn create(state: &BotState<Bot>, player: &CoupPlayer, ability: Option<FullAbility>) -> ClientResult<Message> {
        player.token.followup(&state, webhook_message(|m| {
            m.ephemeral();
            m.content("Choose an influence to lose");
            // todo don't make them choose if they only have one influence left
            m.menu(state, Self(player.id(), ability), |m| {
                m.placeholder("Choose an influence...");
                m.options(player.cards.iter().copied().map(Card::into_option).collect());
            });
        })).await
    }
}

#[async_trait]
impl MenuCommand for LostInfluenceMenu {
    type Bot = Bot;
    type Data = Card;

    async fn run(
        &self,
        state: Arc<BotState<Self::Bot>>,
        interaction: InteractionUse<MenuSelectData, Unused>,
        mut data: Vec<Self::Data>,
    ) -> Result<InteractionUse<MenuSelectData, Used>, BotError<GameError>> {
        let lost = data.remove(0);
        let guild = interaction.guild().unwrap();

        let mut game_guard = state.bot.coup_games.write().await;
        let Coup::Game(game) = game_guard.get_mut(&guild).unwrap() else {
            return send_config_error(&state, interaction).await;
        };
        // game.update_token(&interaction);
        game.wait_state.delete_messages(&state).await?;

        let loser = game.get_player_mut(self.0).unwrap();
        let idx = loser.cards.iter()
            .position(|c| *c == lost)
            .expect("card that is lost is only given the loser's cards");
        let card = loser.cards.remove(idx);
        loser.lost_cards.push(card);

        let interaction = interaction.respond(&state, message(|m| {
            m.content(format!(
                "{} has revealed {card}. {}",
                loser.ping(),
                if loser.cards.is_empty() {
                    "They have no influence and are out of the game!"
                } else {
                    "They have one influence left!"
                }
            ));
            m.attach(card.image());
        })).await?;
        loser.send_roles(&state).await?;
        if let Some(token) = game.lost_influence.take() {
            state.client.delete_interaction_response(state.application_id(), token).await?;
        }
        CoupGame::delete_message(&state, game.lose_influence.take()).await?;
        game.influence_pic = Some(interaction.token.clone());
        if let Some(ability) = self.1 {
            game.resolve_ability(&state, ability).await?;
        } else {
            game.next_turn(&state).await?;
        }
        Ok(interaction)
    }
}

#[derive(Debug, Clone)]
struct CoupPlayer {
    member: GuildMember,
    token: Token,
    coins: usize,
    cards: Vec<Card>,
    lost_cards: Vec<Card>,
    cards_display: Option<(Token, MessageId)>,
    is_exchanging: Option<(Token, MessageId)>,
}

impl CoupPlayer {
    fn name(&self) -> String {
        self.member.nick
            .clone()
            .unwrap_or_else(|| self.member.user.username.clone())
    }

    async fn roles_message(&mut self, state: &BotState<Bot>) -> ClientResult<WebhookMessage> {
        CoupGame::delete_message(state, self.cards_display.take()).await?;
        let message = webhook_message(|m| {
            m.ephemeral();
            self.cards.iter()
                .copied()
                .map(Card::image)
                .enumerate()
                // named so that both 2 attachments appear if both cards are the same
                .for_each(|(i, path)| m.attach((format!("role{i}.png"), path)));
            let roles_str = self.cards.iter()
                // .rev()
                .copied()
                .map(Card::name)
                .list_grammatically(|s| s.bold(), "and");
            let coins = self.coins.to_string().bold();
            m.content(format!("Your influence: {roles_str}. You have {coins} coins."));
        });
        Ok(message)
    }

    async fn send_roles(&mut self, state: &Arc<BotState<Bot>>) -> ClientResult<()> {
        // always delete
        let message = self.roles_message(state).await?;
        if !self.cards.is_empty() {
            let message = self.token.followup(&state, message).await?;
            self.cards_display = Some((self.token.clone(), message.id));
        }
        Ok(())
    }

    fn win_message(&self, state: &BotState<Bot>, restart_enabled: bool) -> InteractionMessage {
        message(|m| {
            m.embed(|e| {
                let name = self.member.nick.clone().unwrap_or_else(|| self.member.user.username.clone());
                e.title(format!("🎉 {name} Wins! 🎉"));
                e.description(format!("They had {} left.", self.cards.iter().list_grammatically(Card::to_string, "and")));
                e.color(Color::GOLD);
                e.authored_by(&self.member.user);
            });
            m.button(state, RestartButton, |b| {
                b.label("Restart");
                if !restart_enabled {
                    b.disable();
                }
            });
        })
    }
}

impl PartialEq for CoupPlayer {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Id for CoupPlayer {
    type Id = UserId;

    fn id(&self) -> Self::Id {
        self.member.id()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, MenuCommand)]
pub enum Card {
    Duke,
    Assassin,
    Ambassador,
    Captain,
    Contessa,
}

impl Card {
    fn from_int(c: usize) -> Self {
        match c {
            0 => Self::Duke,
            1 => Self::Assassin,
            2 => Self::Ambassador,
            3 => Self::Captain,
            4 => Self::Contessa,
            _ => unreachable!(),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Duke => "Duke",
            Self::Assassin => "Assassin",
            Self::Ambassador => "Ambassador",
            Self::Captain => "Captain",
            Self::Contessa => "Contessa",
        }
    }

    fn image(self) -> &'static Path {
        match self {
            Self::Duke => Path::new("images/coup/DukeSmall.jpg"),
            Self::Assassin => Path::new("images/coup/AssassinSmall.jpg"),
            Self::Ambassador => Path::new("images/coup/AmbassadorSmall.jpg"),
            Self::Captain => Path::new("images/coup/CaptainSmall.jpg"),
            Self::Contessa => Path::new("images/coup/ContessaSmall.jpg"),
        }
    }
}