use std::fmt::{self, Display};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_tungstenite::tungstenite::http::StatusCode;
use backoff::{ExponentialBackoff, future::FutureOperation};
use log::{error, warn};
use reqwest::{Client, Method, multipart, Response};
use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::{BotState, serde_utils};
use crate::http::rate_limit::{BucketKey, RateLimiter};
use crate::http::routes::Route;
use crate::model::{BotGateway, DiscordError};
use crate::model::Application;
use crate::serde_utils::NiceResponseJson;

mod rate_limit;
pub mod channel;
pub mod interaction;
pub mod user;
pub(crate) mod routes;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("status code `{0}` at {1:?}")]
    Http(reqwest::StatusCode, Route),
    #[error("json error: {0}")]
    Json(#[from] serde_utils::Error),
    /// For endpoints which require uploading a file
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Discord error: {0:?}")]
    Discord(#[from] DiscordError),
}

impl ClientError {
    pub async fn display_error<B: Send + Sync>(self, state: &BotState<B>) -> DisplayClientError {
        match self {
            Self::Request(e) => DisplayClientError::Request(e),
            Self::Http(status, route) => DisplayClientError::Http(format!("`{}` on {}", status, route.debug_with_cache(&state.cache).await)),
            Self::Json(e) => DisplayClientError::Json(e),
            Self::Io(e) => DisplayClientError::Io(e),
            Self::Discord(e) => DisplayClientError::Discord(e),
        }
    }
}

pub enum DisplayClientError {
    Request(reqwest::Error),
    Http(String),
    Json(serde_utils::Error),
    Io(std::io::Error),
    Discord(DiscordError),
}

impl Display for DisplayClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Request(e) => write!(f, "{}", e),
            Self::Http(e) => f.write_str(e),
            Self::Json(e) => write!(f, "{}", e),
            Self::Io(e) => write!(f, "{}", e),
            Self::Discord(e) => write!(f, "{}", e),
        }
    }
}

pub type ClientResult<T> = std::result::Result<T, ClientError>;

#[derive(Debug)]
pub struct DiscordClient {
    pub(crate) token: String,
    client: Client,
    rate_limit: Arc<RwLock<RateLimiter>>,
}

/// General functionality
impl DiscordClient {
    /// Create a new [`DiscordClient`] using the specified bot `token`
    pub fn single(token: String) -> Self {
        Self::shared(token, Default::default())
    }

    pub fn shared(token: String, rate_limit: Arc<RwLock<RateLimiter>>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bot {}", token).parse().expect("Unable to parse token!"));

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("Unable to build client!");

        Self { token, client, rate_limit }
    }

    async fn request<Q, J, F, R, Fut, T>(&self, request: Request<Q, J, F, R, Fut, T>) -> ClientResult<T>
        where Q: Serialize + Send + Sync,
              J: Serialize + Send + Sync,
              F: Fn() -> Option<multipart::Form> + Send + Sync,
              R: Fn(Response) -> Fut + Send + Sync,
              Fut: Future<Output=ClientResult<T>> + Send,
              T: DeserializeOwned,
    {
        let Request { method, route, query, body, multipart, getter } = request;
        let key = BucketKey::from(&route);
        (|| async {
            let mut builder = self.client.request(method.clone(), &route.url());
            if let Some(query) = &query {
                builder = builder.query(query);
            }
            if let Some(json) = &body {
                builder = builder.json(json);
            }
            if let Some(multipart) = multipart() {
                builder = builder.multipart(multipart);
            }
            self.rate_limit.read().await.rate_limit(&key).await;
            let response = builder.send().await.map_err(ClientError::Request)?;
            let headers = response.headers();
            self.rate_limit.write().await.update(key, headers);
            if response.status().is_client_error() || response.status().is_server_error() {
                let status = response.status();
                let err = if status == StatusCode::TOO_MANY_REQUESTS {
                    backoff::Error::Transient(ClientError::Http(status, route.clone()))
                } else {
                    let permanent = if let Ok(error) = response.nice_json().await {
                        ClientError::Discord(error)
                    } else {
                        ClientError::Http(status, route.clone())
                    };
                    backoff::Error::Permanent(permanent)
                };
                Err(err)
            } else {
                Ok(getter(response).await?)
            }
        }).retry_notify(
            // todo make it wait exactly how long ratelimit says if it was ratelimited? is that
            //  possible? is that even a problem, it might only be for emojis and those arent accurate idk
            {
                let mut eb = ExponentialBackoff::default();
                eb.max_elapsed_time = Some(Duration::from_secs(10));
                eb
            },
            |e: ClientError, dur|
                if !matches!(e, ClientError::Http(StatusCode::TOO_MANY_REQUESTS, Route::CreateReaction(_, _, _))) {
                    warn!("Error in request after {:?}: {}", dur, e)
                },
        ).await
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, route: Route) -> ClientResult<T> {
        self.request::<Ser, Ser, _, _, _, _>(Request {
            method: Method::GET,
            route,
            query: None,
            body: None,
            multipart: || None,
            getter: |resp| resp.nice_json(),
        }).await
    }

    pub(crate) async fn post<T, J>(&self, route: Route, json: J) -> ClientResult<T>
        where T: DeserializeOwned,
              J: Serialize + Send + Sync,
    {
        self.request::<Ser, _, _, _, _, _>(Request {
            method: Method::POST,
            route,
            query: None,
            body: Some(json),
            multipart: || None,
            getter: |resp| resp.nice_json(),
        }).await
    }

    pub(crate) async fn post_multipart<T, F>(&self, route: Route, multipart: F) -> ClientResult<T>
        where T: DeserializeOwned,
              F: Fn() -> Option<multipart::Form> + Send + Sync,
    {
        self.request::<Ser, Ser, _, _, _, _>(Request {
            method: Method::POST,
            route,
            query: None,
            body: None,
            multipart,
            getter: |resp| resp.nice_json(),
        }).await
    }

    pub(crate) async fn post_unit<J: Serialize + Send + Sync>(&self, route: Route, json: J) -> ClientResult<()> {
        self.request::<Ser, _, _, _, _, _>(Request {
            method: Method::POST,
            route,
            query: None,
            body: Some(json),
            multipart: || None,
            getter: |_| async { Ok(()) },
        }).await
    }

    pub(crate) async fn patch<T, J: Send + Sync>(&self, route: Route, json: J) -> ClientResult<T>
        where T: DeserializeOwned,
              J: Serialize, {
        self.request::<Ser, _, _, _, _, _>(Request {
            method: Method::PATCH,
            route,
            query: None,
            body: Some(json),
            multipart: || None,
            getter: |resp| resp.nice_json(),
        }).await
    }

    pub(crate) async fn patch_unit<J: Serialize + Send + Sync>(&self, route: Route, json: J) -> ClientResult<()> {
        self.request::<Ser, _, _, _, _, _>(Request {
            method: Method::PATCH,
            route,
            query: None,
            body: Some(json),
            multipart: || None,
            getter: |_| async { Ok(()) },
        }).await
    }

    pub(crate) async fn put_unit(&self, route: Route) -> ClientResult<()> {
        self.request::<Ser, _, _, _, _, _>(Request {
            method: Method::PUT,
            route,
            query: None,
            body: Some(""),
            multipart: || None,
            getter: |_| async { Ok(()) },
        }).await
    }

    pub(crate) async fn delete(&self, route: Route) -> ClientResult<()> {
        self.request::<Ser, Ser, _, _, _, _>(Request {
            method: Method::DELETE,
            route,
            query: None,
            body: None,
            multipart: || None,
            getter: |_| async { Ok(()) },
        }).await
    }
}

pub(crate) struct Request<Q, J, F, R, Fut, T>
    where
    // F: Fn() -> Option<multipart::Form>,
        R: Fn(Response) -> Fut,
        Fut: Future<Output=ClientResult<T>>
{
    method: Method,
    route: Route,
    query: Option<Q>,
    body: Option<J>,
    multipart: F,
    getter: R,
}

/// Never created, just used to tell `request` what type the `None` options are
#[derive(Serialize)]
enum Ser {}

/// general functions
impl DiscordClient {
    /// Gets information about how to connect to the bot's websocket
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `BotGateway`
    pub async fn gateway(&self) -> ClientResult<BotGateway> {
        self.get(Route::GetGateway).await
    }

    /// Gets application information for the bot's application
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Application`
    pub async fn application_information(&self) -> ClientResult<Application> {
        self.get(Route::ApplicationInfo).await
    }
}

impl AsRef<DiscordClient> for DiscordClient {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<B> AsRef<DiscordClient> for BotState<B> {
    fn as_ref(&self) -> &DiscordClient {
        &self.client
    }
}

impl<B> AsRef<DiscordClient> for Arc<BotState<B>> {
    fn as_ref(&self) -> &DiscordClient {
        &self.client
    }
}