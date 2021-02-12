use async_trait::async_trait;
use serde_json::json;

use crate::BotState;
use crate::http::{ClientResult, DiscordClient};
use crate::http::channel::{ChannelExt, CreateMessage};
use crate::http::routes::Route::*;

pub use crate::model::ids::*;
use crate::model::user::User;
use crate::model::channel::DmChannel;
use crate::model::message::Message;

impl DiscordClient {
    /// Returns a user object for a given user ID
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `User`
    pub async fn get_user(&self, user: UserId) -> ClientResult<User> {
        self.get(GetUser(user)).await
    }

    /// Create a new DM channel with a user
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `DmChannel`
    pub async fn create_dm(&self, user: UserId) -> ClientResult<DmChannel> {
        self.post(CreateDm, json!({ "recipient_id": user })).await
    }
}

#[async_trait]
pub trait UserExt: Id<Id=UserId> + Sized {
    async fn dm<B, State>(&self, state: State) -> ClientResult<DmChannel>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
    {
        let state = state.as_ref();
        let option = {
            let (by_user, by_channel) = &*state.cache.dms.read().await;
            by_user.get(&self.id())
                .and_then(|c| by_channel.get(c))
                .cloned()
        };
        if let Some(dm) = option {
            Ok(dm)
        } else {
            let dm = state.client.create_dm(self.id()).await?;
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
}

#[async_trait]
impl<U: Id<Id=UserId>> UserExt for U {}