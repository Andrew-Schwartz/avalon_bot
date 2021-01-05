use std::fmt::Display;
use std::sync::Arc;

use async_tungstenite::{
    tokio::{connect_async, ConnectStream},
    tungstenite::Message,
    tungstenite::protocol::CloseFrame,
    tungstenite::protocol::frame::coding::CloseCode,
    WebSocketStream,
};
use futures::{SinkExt, TryStreamExt};
use log::{error, info, warn};
use rand::Rng;
use thiserror::Error;
use tokio::time::{Duration, Instant};

use dispatch::DispatchPayload;
use model::{HelloPayload, Payload, Resume};

use crate::Bot;
use crate::bot::BotState;
use crate::cache::Update;
use crate::http::ClientError;
use crate::macros::API_VERSION;
use crate::serde_utils::nice_from_str;
use crate::shard::model::Heartbeat;

pub mod model;
pub mod dispatch;
pub mod intents;

#[derive(Debug, Error)]
pub enum ShardError {
    #[error("http error: {0}")]
    Request(#[from] ClientError),
    #[error("websocket error: {0}")]
    Websocket(#[from] async_tungstenite::tungstenite::Error),
    #[error("json error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ShardError>;
type WsStream = WebSocketStream<ConnectStream>;

pub struct Shard<B: Bot> {
    stream: Option<WsStream>,
    pub shard_info: (u64, u64),
    state: Arc<BotState<B>>,
    session_id: Option<String>,
    seq: Option<u64>,
    heartbeat_interval: Option<Duration>,
    heartbeat: Option<Instant>,
    ack: Option<Instant>,
    strikes: u8,
}

impl<B: Bot + 'static> Shard<B> {
    pub fn new(state: Arc<BotState<B>>) -> Result<Self> {
        // let stream = Shard::connect(&state).await?;
        Ok(Self {
            stream: None,
            shard_info: (0, 0),
            state,
            session_id: None,
            seq: None,
            heartbeat_interval: None,
            heartbeat: None,
            ack: None,
            strikes: 0,
        })
    }

    async fn connect(&mut self) -> Result<()> {
        let ws = format!("{}/?v={}&encoding=json", self.state.client.gateway().await?.url, API_VERSION);
        info!("connecting to {}", &ws);
        let (stream, _): (WsStream, _) = connect_async(ws).await?;
        self.stream = Some(stream);
        // Ok(stream)
        Ok(())
    }

    async fn close(&mut self, close_frame: CloseFrame<'_>, delay: impl Into<Option<Duration>>) -> Result<()> {
        info!("closing: {:?}", close_frame);
        if let Some(mut stream) = self.stream.take() {
            stream.close(Some(close_frame)).await?;
            info!("stream closed");
        } else {
            info!("stream was already closed")
        }
        if let Some(delay) = delay.into() {
            info!("delaying for {:?}", delay);
            tokio::time::delay_for(delay).await;
        }
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            // need to connect
            if self.stream.is_none() {
                info!("connecting");
                self.connect().await?;
            }

            if let (Some(session), &Some(seq)) = (&self.session_id, &self.seq) {
                let resume = Resume {
                    token: self.state.client.token.clone(),
                    session_id: session.clone(),
                    seq,
                };
                self.send(resume).await?;
            }

            'events: loop {
                self.heartbeat().await?;

                let stream = if let Some(stream) = &mut self.stream {
                    stream
                } else {
                    warn!("start of 'events loop with a None stream");
                    break 'events;
                };
                let result = tokio::time::timeout(
                    Duration::from_millis(200),
                    stream.try_next(),
                ).await;
                if let Ok(next) = result {
                    match next {
                        Ok(Some(Message::Text(text))) => {
                            let read = nice_from_str(&text);
                            let payload = match read {
                                Ok(payload) => payload,
                                Err(payload_parse_error) => {
                                    error!("payload_parse_error = {}", payload_parse_error);
                                    println!("{}", text);
                                    continue;
                                }
                            };
                            let need_restart = self.handle_payload(payload).await?;
                            if need_restart {
                                break 'events;
                            }
                        }
                        Ok(Some(Message::Close(close_frame))) => {
                            error!("close frame = {:?}", close_frame);
                            self.reset_connection_state();
                            self.close(close_frame.unwrap_or_else(|| CloseFrame {
                                code: CloseCode::Restart,
                                reason: "Received `Message::Close` (without a CloseFrame)".into(),
                            }), None).await?;
                            break 'events;
                        }
                        Ok(Some(msg)) => warn!("msg = {:?}", msg),
                        Ok(None) => {
                            error!("Websocket closed");
                            self.reset_connection_state();
                            self.close(CloseFrame {
                                code: CloseCode::Restart,
                                reason: "Websocket closed".into(),
                            }, None).await?;
                            break 'events;
                        }
                        Err(ws_error) => {
                            // Protocol("Connection reset without closing handshake")
                            error!("ws_error = {:?}", ws_error);
                            self.reset_connection_state();
                            self.close(CloseFrame {
                                code: CloseCode::Error,
                                reason: "websocket error".into(),
                            }, None).await?;
                            break 'events;
                        }
                    }
                }
            }
        }
    }

    async fn heartbeat(&mut self) -> Result<()> {
        if let (Some(heartbeat), Some(ack)) = (self.heartbeat, self.ack) {
            // if we haven't received a `HeartbeatAck` since the last time we sent a heartbeat,
            // give the connection a strike
            if heartbeat.checked_duration_since(ack).is_some() {
                self.strikes += 1;
                println!("self.strikes = {:?}", self.strikes);
                if self.strikes >= 3 {
                    self.reset_connection_state();
                    self.close(CloseFrame {
                        code: CloseCode::Restart,
                        reason: "ACK not recent enough, closing websocket".into(),
                    }, None).await.unwrap();
                }
            } else {
                self.strikes = 0;
            }
        }

        match (self.heartbeat, self.heartbeat_interval, self.seq) {
            (Some(last_sent), Some(interval), _) if last_sent.elapsed() < interval => {}
            (_, _, Some(seq_num)) => {
                self.send(Heartbeat { seq_num }).await?;
                self.heartbeat = Some(Instant::now());
            }
            _ => {}
        }

        Ok(())
    }

    /// handles `payload`, returns `true` if we need to reconnect
    async fn handle_payload(&mut self, payload: Payload) -> Result<bool> {
        let need_reconnect = match payload {
            Payload::Hello(HelloPayload { heartbeat_interval }) => {
                if self.session_id.is_none() {
                    self.initialize_connection(heartbeat_interval).await?;
                }
                false
            }
            Payload::Dispatch { event, seq_num } => {
                if let Some(curr) = self.seq {
                    if seq_num > curr + 1 {
                        warn!("received seq num {}, expected {} ({} were missed)",
                              seq_num, curr + 1, seq_num - curr - 1
                        )
                    }
                }
                self.seq = Some(seq_num);
                self.handle_dispatch(event).await?;
                false
            }
            Payload::HeartbeatAck => {
                self.ack = Some(Instant::now());
                false
            }
            Payload::Heartbeat(heartbeat) => {
                info!("recv: Heartbeat {}", heartbeat.seq_num);
                false
            }
            Payload::Reconnect => {
                info!("recv: Reconnect");
                self.close(CloseFrame {
                    code: CloseCode::Restart,
                    reason: "Reconnect requested by Discord".into(),
                }, None).await?;
                true
            }
            Payload::InvalidSession(resumable) => {
                info!("recv: Invalid Session");
                if !resumable {
                    self.reset_connection_state();

                    let delay = rand::thread_rng().gen_range(1, 6);
                    self.close(CloseFrame {
                        code: CloseCode::Restart,
                        reason: "(non-resumable) Invalid Session".into(),
                    }, Duration::from_secs(delay)).await?;
                } else {
                    warn!("Resumable Invalid Session: anything special to do here?");
                }
                true
            }
            _ => {
                error!("Should not receive {:?}", payload);
                false
            }
        };
        Ok(need_reconnect)
    }

    async fn initialize_connection(&mut self, heartbeat_interval: u64) -> Result<()> {
        let delay = Duration::from_millis(heartbeat_interval);
        self.heartbeat_interval = Some(delay);

        if self.session_id.is_none() {
            self.send(self.state.bot.identify()).await?;
        }

        Ok(())
    }

    async fn handle_dispatch(&mut self, event: DispatchPayload) -> Result<()> {
        use DispatchPayload::*;
        event.clone().update(&self.state.cache).await;
        if let Ready(ready) = &event {
            // make sure were using the right API version
            assert_eq!(API_VERSION, ready.v);

            // make sure we're the right shard
            let (id, tot) = ready.shard.unwrap_or((0, 0));
            assert_eq!(id, self.shard_info.0);
            assert_eq!(tot, self.shard_info.1);

            self.session_id = Some(ready.session_id.clone());
        }
        let bot = Arc::clone(&self.state);
        // todo panic if this panicked? (make a field in self for handlers, try_join them?)
        let _handle = tokio::spawn(async move {
            let result = match event {
                Ready(_ready) => bot.bot.ready(Arc::clone(&bot)).await,
                Resumed(_resumed) => bot.bot.resumed(Arc::clone(&bot)).await,
                GuildCreate(guild) => bot.bot.guild_create(
                    guild.guild, Arc::clone(&bot),
                ).await,
                MessageCreate(message) => bot.bot.message_create(
                    message.message, Arc::clone(&bot),
                ).await,
                MessageUpdate(update) => bot.bot.message_update(
                    bot.cache.message(update.id).await.unwrap(),
                    Arc::clone(&bot),
                    update,
                ).await,
                InteractionCreate(interaction) => bot.bot.interaction(
                    interaction.interaction, Arc::clone(&bot),
                ).await,
                MessageReactionAdd(add) => bot.bot.reaction(
                    add.into(),
                    Arc::clone(&bot),
                ).await,
                MessageReactionRemove(remove) => bot.bot.reaction(
                    remove.into(),
                    Arc::clone(&bot),
                ).await,
                IntegrationUpdate(integration) => bot.bot.integration_update(
                    integration.guild_id,
                    integration.integration,
                    Arc::clone(&bot),
                ).await,
                _ => Ok(())
            };
            if let Err(error) = result {
                bot.bot.error(error, Arc::clone(&bot)).await;
            }
        });

        Ok(())
    }

    async fn send<P>(&mut self, payload: P) -> Result<()>
        where P: Into<Payload> + Display
    {
        info!("sending {}", payload);
        let message = serde_json::to_string(&payload.into())?;
        self.stream.as_mut()
            .expect("trying to send with a None stream")
            .send(Message::Text(message)).await?;
        Ok(())
    }

    fn reset_connection_state(&mut self) {
        let Self {
            // stream,
            session_id,
            seq,
            heartbeat_interval,
            heartbeat,
            ack,
            strikes,
            // online,
            ..
        } = self;
        // *stream = None;
        *session_id = None;
        *seq = None;
        *heartbeat_interval = None;
        *heartbeat = None;
        *ack = None;
        *strikes = 0;
        // *online = false;
    }
}