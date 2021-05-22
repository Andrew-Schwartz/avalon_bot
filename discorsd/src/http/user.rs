//! Discord API requests involving users.

use async_trait::async_trait;
use serde::Serialize;
use serde_json::json;

use crate::BotState;
use crate::http::{ClientResult, DiscordClient, ImageData};
use crate::http::channel::{CreateMessage, MessageChannelExt};
use crate::http::routes::Route::*;
use crate::model::channel::DmChannel;
use crate::model::guild::PartialGuild;
use crate::model::ids::*;
use crate::model::message::Message;
use crate::model::user::User;

impl DiscordClient {
    /// Returns a user object for a given user ID.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `User`.
    pub async fn get_user(&self, user: UserId) -> ClientResult<User> {
        self.get(GetUser(user)).await
    }

    /// Modify the requester's user account settings.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `User`.
    pub async fn modify_current_user<'u, U, A>(&self, new_username: U, new_avatar: A) -> ClientResult<User>
        where
            U: Into<Option<&'u str>> + Send,
            A: Into<Option<ImageData>> + Send,
    {
        #[derive(Serialize)]
        struct Shim<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            username: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            avatar: Option<String>,
        }
        let username = new_username.into();
        let avatar = new_avatar.into().map(ImageData::into_inner);
        self.patch(ModifyCurrentUser, Shim { username, avatar }).await
    }

    /// Returns a list of partial guild objects the current user is a member of.
    /// Requires the guilds `OAuth2` scope.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Vec<PartialGuild>`.
    pub async fn get_current_user_guilds(&self, query: CurrentGuildQuery) -> ClientResult<Vec<PartialGuild>> {
        self.get_query(GetCurrentUserGuilds, query).await
    }

    /// Create a new DM channel with a user.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `DmChannel`.
    pub async fn create_dm(&self, user: UserId) -> ClientResult<DmChannel> {
        self.post(CreateDm, json!({ "recipient_id": user })).await
    }
}

#[derive(Serialize)]
pub struct CurrentGuildQuery {
    /// Get guilds before this guild ID
    before: Option<GuildId>,
    /// Get guilds after this guild ID
    after: Option<GuildId>,
    /// Max number of guilds to return (1-100), defaults to None
    limit: u32,
}

impl Default for CurrentGuildQuery {
    fn default() -> Self {
        Self { before: None, after: None, limit: 100 }
    }
}

// todo
// impl CurrentGuildQuery {
//     pub fn before(id: GuildId) -> Self {
//
//     }
// }

#[async_trait]
pub trait UserExt: Id<Id=UserId> + Sized {
    async fn dm<B, State>(&self, state: State) -> ClientResult<DmChannel>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
    {
        let state = state.as_ref();
        if let Some(dm) = state.cache.dm_channel(self).await {
            Ok(dm)
        } else {
            let dm = state.client.create_dm(self.id()).await?;
            // todo is this necessary or does it just happen in ChannelCreate anyways?
            let (by_user, by_channel) = &mut *state.cache.dms.write().await;
            by_user.insert(self.id(), dm.id);
            by_channel.insert(dm.clone());
            Ok(dm)
        }
    }

    async fn send_dm<B, State, Msg>(
        &self,
        state: State,
        message: Msg,
    ) -> ClientResult<Message> where
        B: Send + Sync + 'static,
        State: AsRef<BotState<B>> + Send + Sync,
        Msg: Into<CreateMessage> + Send + Sync,
    {
        let dm = self.dm(&state).await?;
        dm.send(state.as_ref(), message).await
    }

    async fn send_dm_unchecked<B, State, Msg>(
        &self,
        state: State,
        message: Msg,
    ) -> ClientResult<Message> where
        B: Send + Sync + 'static,
        State: AsRef<BotState<B>> + Send + Sync,
        Msg: Into<CreateMessage> + Send + Sync,
    {
        let dm = self.dm(&state).await?;
        dm.send_unchecked(state.as_ref(), message).await
    }
}

#[async_trait]
impl<U: Id<Id=UserId>> UserExt for U {}