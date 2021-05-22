//! Functionality for making http requests to Discord's API.

use std::ffi::OsStr;
use std::fmt::{self, Display};
use std::future::Future;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_tungstenite::tungstenite::http::StatusCode;
use backoff::ExponentialBackoff;
use log::{error, warn};
use reqwest::{Client, Method, multipart, Response};
use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{BotState, serde_utils};
use crate::http::rate_limit::{BucketKey, RateLimiter};
use crate::http::routes::Route;
use crate::model::{BotGateway, DiscordError};
use crate::model::Application;
use crate::model::permissions::Permissions;
use crate::serde_utils::NiceResponseJson;

mod rate_limit;
pub(crate) mod routes;

pub mod channel;
pub mod guild;
pub mod interaction;
pub mod user;

/// An error that happened while making a request to Discord's API.
///
/// If you have a `ClientError` in an async context, you likely want to display
/// [ClientError::display_error](ClientError::display_error) instead of just this enum to get more
/// context.
///
/// ```rust
/// # use discorsd::http::ClientError;
/// # use discorsd::BotState;
/// # use std::sync::Arc;
/// async fn handle_client_error<B: Send+ Sync>(error: ClientError, state: Arc<BotState<B>>) {
///     // do this
///     eprintln!("look at this error: {}", error.display_error(&state).await);
///
///     // not this
///     eprintln!("look at this error: {}", error);
/// }
/// ```
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("error making request: {0}")]
    Request(#[from] reqwest::Error),
    #[error("status code `{0}` at {1:?}")]
    Http(reqwest::StatusCode, Route),
    #[error("bad json: {0}")]
    Json(#[from] serde_utils::Error),
    /// For endpoints which require uploading a file
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Discord error: {0:?}")]
    Discord(#[from] DiscordError),
    #[error("lacking permissions {0:?}")]
    Perms(Permissions),
}

impl ClientError {
    /// Get cached information to make the error message have more context.
    pub async fn display_error<B: Send + Sync>(&self, state: &BotState<B>) -> DisplayClientError<'_> {
        match self {
            Self::Request(e) => DisplayClientError::Request(e),
            Self::Http(status, route) => DisplayClientError::Http(format!("status code `{}` at {:?}", status, route.debug_with_cache(&state.cache).await)),
            Self::Json(e) => DisplayClientError::Json(e),
            Self::Io(e) => DisplayClientError::Io(e),
            Self::Discord(e) => DisplayClientError::Discord(e),
            Self::Perms(p) => DisplayClientError::Perms(*p),
        }
    }
}

/// Display a [`ClientError`] with more context.
pub enum DisplayClientError<'a> {
    Request(&'a reqwest::Error),
    Http(String),
    Json(&'a serde_utils::Error),
    Io(&'a std::io::Error),
    Discord(&'a DiscordError),
    Perms(Permissions),
}

impl Display for DisplayClientError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Request(e) => write!(f, "error making request: {}", e),
            Self::Http(e) => f.write_str(e),
            Self::Json(e) => write!(f, "bad json: {}", e),
            Self::Io(e) => write!(f, "io error: {}", e),
            Self::Discord(e) => write!(f, "Discord error: {:?}", e),
            Self::Perms(p) => write!(f, "lacking permissions {:?}", p)
        }
    }
}

/// Result where the error type is [`ClientError`].
pub type ClientResult<T> = Result<T, ClientError>;

/// Handles performing requests to Discord's api, managing your Bot's token and Discord's rate
/// limits.
///
/// Wraps [Reqwest's Client](https://docs.rs/reqwest/*/reqwest/struct.Client.html).
#[derive(Debug)]
pub struct DiscordClient {
    pub(crate) token: String,
    client: Client,
    rate_limit: Arc<Mutex<RateLimiter>>,
}

/// General functionality
impl DiscordClient {
    /// Create a new [`DiscordClient`] using the specified bot `token`
    pub(crate) fn single(token: String) -> Self {
        Self::shared(token, Default::default())
    }

    /// Create a
    pub(crate) fn shared(token: String, rate_limit: Arc<Mutex<RateLimiter>>) -> Self {
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
        let async_operation = || async {
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
            self.rate_limit.lock().await.rate_limit(&key).await;
            let response = builder.send().await.map_err(ClientError::Request)?;
            let headers = response.headers();
            self.rate_limit.lock().await.update(key, headers);
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
        };
        backoff::future::retry_notify(
            ExponentialBackoff {
                max_elapsed_time: Some(Duration::from_secs(10)),
                ..Default::default()
            },
            async_operation,
            |e: ClientError, dur|
                if !matches!(e, ClientError::Http(StatusCode::TOO_MANY_REQUESTS, Route::CreateReaction(_, _, _))) {
                    warn!("Error in request after {:?}: {}", dur, e)
                },
        ).await
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, route: Route) -> ClientResult<T> {
        self.request(Request::new(
            Method::GET,
            route,
            || None,
            NiceResponseJson::nice_json,
        )).await
    }

    pub(crate) async fn get_query<T, Q>(&self, route: Route, query: Q) -> ClientResult<T>
        where T: DeserializeOwned,
              Q: Serialize + Send + Sync,
    {
        self.request(Request::with_query(
            Method::GET,
            route,
            query,
            || None,
            NiceResponseJson::nice_json,
        )).await
    }

    pub(crate) async fn post<T, J>(&self, route: Route, json: J) -> ClientResult<T>
        where T: DeserializeOwned,
              J: Serialize + Send + Sync,
    {
        self.request(Request::with_body(
            Method::POST,
            route,
            json,
            || None,
            NiceResponseJson::nice_json,
        )).await
    }

    pub(crate) async fn post_multipart<T, F>(&self, route: Route, multipart: F) -> ClientResult<T>
        where T: DeserializeOwned,
              F: Fn() -> Option<multipart::Form> + Send + Sync,
    {
        self.request(Request::new(
            Method::POST,
            route,
            multipart,
            NiceResponseJson::nice_json,
        )).await
    }

    pub(crate) async fn post_unit<J: Serialize + Send + Sync>(&self, route: Route, json: J) -> ClientResult<()> {
        self.request(Request::with_body(
            Method::POST,
            route,
            json,
            || None,
            |_| async { Ok(()) },
        )).await
    }

    pub(crate) async fn patch<T, J>(&self, route: Route, json: J) -> ClientResult<T>
        where T: DeserializeOwned,
              J: Serialize + Send + Sync,
    {
        self.request(Request::with_body(
            Method::PATCH,
            route,
            json,
            || None,
            NiceResponseJson::nice_json,
        )).await
    }

    pub(crate) async fn patch_unit<J: Serialize + Send + Sync>(&self, route: Route, json: J) -> ClientResult<()> {
        self.request(Request::with_body(
            Method::PATCH,
            route,
            json,
            || None,
            |_| async { Ok(()) },
        )).await
    }

    pub(crate) async fn put<T, J>(&self, route: Route, json: J) -> ClientResult<T>
        where T: DeserializeOwned,
              J: Serialize + Send + Sync,
    {
        self.request(Request::with_body(
            Method::PUT,
            route,
            json,
            || None,
            NiceResponseJson::nice_json,
        )).await
    }

    pub(crate) async fn put_unit<J>(&self, route: Route, json: J) -> ClientResult<()>
        where J: Serialize + Send + Sync,
    {
        self.request(Request::with_body(
            Method::PUT,
            route,
            json,
            || None,
            |_| async { Ok(()) },
        )).await
    }

    pub(crate) async fn delete(&self, route: Route) -> ClientResult<()> {
        self.request(Request::new(
            Method::DELETE,
            route,
            || None,
            |_| async { Ok(()) },
        )).await
    }
}

pub(crate) struct Request<Q, J, F, R, Fut, T>
    where
        F: Fn() -> Option<multipart::Form>,
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

impl<F, R, Fut, T> Request<SerializeNever, SerializeNever, F, R, Fut, T> where
    F: Fn() -> Option<multipart::Form>,
    R: Fn(Response) -> Fut,
    Fut: Future<Output=ClientResult<T>>
{
    fn new(method: Method, route: Route, multipart: F, getter: R) -> Self {
        Self {
            method,
            route,
            query: None,
            body: None,
            multipart,
            getter,
        }
    }
}

impl<J, F, R, Fut, T> Request<SerializeNever, J, F, R, Fut, T> where
    F: Fn() -> Option<multipart::Form>,
    R: Fn(Response) -> Fut,
    Fut: Future<Output=ClientResult<T>>
{
    fn with_body(method: Method, route: Route, body: J, multipart: F, getter: R) -> Self {
        Self {
            method,
            route,
            query: None,
            body: Some(body),
            multipart,
            getter,
        }
    }
}

impl<Q, F, R, Fut, T> Request<Q, SerializeNever, F, R, Fut, T> where
    F: Fn() -> Option<multipart::Form>,
    R: Fn(Response) -> Fut,
    Fut: Future<Output=ClientResult<T>>
{
    fn with_query(method: Method, route: Route, query: Q, multipart: F, getter: R) -> Self {
        Self {
            method,
            route,
            query: Some(query),
            body: None,
            multipart,
            getter,
        }
    }
}

/// Never created, just used to tell `Request` what type the `None` options are
#[derive(Serialize)]
enum SerializeNever {}

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

impl AsRef<Self> for DiscordClient {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<B: Send + Sync> AsRef<DiscordClient> for BotState<B> {
    fn as_ref(&self) -> &DiscordClient {
        &self.client
    }
}

impl<B: Send + Sync> AsRef<DiscordClient> for Arc<BotState<B>> {
    fn as_ref(&self) -> &DiscordClient {
        &self.client
    }
}

/// Base 64 encoding of an image, with some additional metadata.
///
/// See [Image Data](https://discord.com/developers/docs/reference#image-data).
pub struct ImageData(String);

/// An error that occurs while hashing a profile image.
#[derive(Debug, Error)]
pub enum ImageHashError {
    #[error("Unsupported error type. Only `png`, `jpeg`, and `gif` are supported by Discord.")]
    FileType,
    #[error("IO error {0}")]
    Io(io::Error),
}

impl ImageData {
    // todo fix this, `base64::encode` is way too long
    //  actually I was trying to put this where `icon_url` should be, have to test on some POST/PATCH or
    //  some method
    /// Encode an image in base 64 and format it so that it can be uploaded to Discord, ie in the form
    /// `data:image/jpeg;base64,BASE64_ENCODED_JPEG_IMAGE_DATA`.
    ///
    /// # Errors
    ///
    /// Errors if the the file at `path` isn't one of the supported image types (which are png, jpg, and
    /// gif), or if [`std::fs::read`](std::fs::read) fails
    pub async fn hash_image<P: AsRef<Path> + Send>(path: P) -> Result<Self, ImageHashError> {
        let path = path.as_ref();
        let image = path.extension()
            .and_then(OsStr::to_str)
            .and_then(|ext| match ext {
                "jpg" | "jpeg" => Some("jpeg"),
                "png" => Some("png"),
                "gif" => Some("gif"),
                _ => None,
            });
        if let Some(image) = image {
            match tokio::fs::read(path).await {
                Ok(file) => {
                    Ok(Self(format!("data:image/{};base64,{}", image, base64::encode(&file))))
                }
                Err(e) => Err(ImageHashError::Io(e)),
            }
        } else {
            Err(ImageHashError::FileType)
        }
    }

    fn into_inner(self) -> String {
        self.0
    }
}