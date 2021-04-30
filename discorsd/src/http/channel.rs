//! Discord API requests involving channels.
//!
//! Use these [impl DiscordClient](../struct.DiscordClient.html#impl) methods for the low level api
//! for channel related requests, and the [MessageChannelExt] extension trait for higher level api
//! access to some of the requests.

use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use reqwest::{IntoUrl, Url};
use reqwest::multipart::{Form, Part};
use serde::Serialize;

use crate::BotState;
use crate::http::{ClientError, DiscordClient};
use crate::http::ClientResult;
use crate::http::interaction::WebhookMessage;
use crate::http::routes::Route::*;
use crate::http::routes::Route;
use crate::model::channel::{Channel, DmChannel, GroupDmChannel, NewsChannel, TextChannel};
use crate::model::emoji::Emoji;
use crate::model::ids::*;
use crate::model::message::*;
use crate::model::permissions::Permissions;
use crate::model::user::User;

/// Channel related http requests
impl DiscordClient {
    /// Get a channel by ID. Returns a [`Channel`](Channel) object.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Channel`
    pub async fn get_channel(&self, id: ChannelId) -> ClientResult<Channel> {
        self.get(GetChannel(id)).await
    }

    // todo
    // /// Update a channel's settings. Requires the `MANAGE_CHANNELS` permission for the guild. Fires
    // /// a [ChannelUpdate](crate::shard::dispatch::DispatchEvent::ChannelUpdate) event.
    // /// If modifying a category, individual [ChannelUpdate](crate::shard::dispatch::DispatchEvent::ChannelUpdate)
    // /// events will fire for each child channel that also changes.
    // pub async fn modify_channel(&self, id: ChannelId, channel: ) -> Result<Channel> {
    //     self.patch(api!("/channels/{}", id), json).await
    // }

    /// Returns a specific message in the channel. If operating on a guild channel, this endpoint
    /// requires the
    /// [`READ_MESSAGE_HISTORY`](crate::model::permissions::Permissions::READ_MESSAGE_HISTORY)
    /// permission to be present on the current user.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Message`.
    pub async fn get_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<Message> {
        self.get(GetMessage(channel, message)).await
    }

    /// Post a message in the specified channel
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Message`.
    pub async fn create_message(&self, channel: ChannelId, message: CreateMessage) -> ClientResult<Message> {
        self.send_message_with_files(PostMessage(channel), message).await
    }

    /// Edits the specified message according to `edit`.
    ///
    /// Only [MessageFlags::SUPPRESS_EMBEDS] can be set/unset, but trying to send other flags is not
    /// an error.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Message`.
    pub async fn edit_message(&self, channel: ChannelId, message: MessageId, edit: EditMessage) -> ClientResult<Message> {
        // not an error to send other flags
        // let flags = flags & MessageFlags::SUPPRESS_EMBEDS;
        self.patch(EditMessage(channel, message), edit).await
    }

    /// Delete a message. If operating on a guild channel and trying to delete a message that was
    /// not sent by the current user, this endpoint requires the `MANAGE_MESSAGES` permission.
    ///
    /// Fires a [`MessageDelete`](crate::shard::dispatch::MessageDelete) event.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<()> {
        self.delete(DeleteMessage(channel, message)).await
    }

    /// Create a reaction for the message. This endpoint requires the `READ_MESSAGE_HISTORY`
    /// permission to be present on the current user. Additionally, if nobody else has reacted to
    /// the message using this emoji, this endpoint requires the `ADD_REACTIONS` permission to be
    /// present on the current user.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn create_reaction<E: Into<Emoji> + Send>(
        &self,
        channel: ChannelId,
        message: MessageId,
        emoji: E,
    ) -> ClientResult<()> {
        self.put_unit(CreateReaction(channel, message, emoji.into()), "").await
    }

    /// Delete a reaction the current user has made for the message.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_own_reaction<E: Into<Emoji> + Send>(
        &self,
        channel: ChannelId,
        message: MessageId,
        emoji: E,
    ) -> ClientResult<()> {
        self.delete(DeleteOwnReaction(channel, message, emoji.into())).await
    }

    /// Deletes another user's reaction. This endpoint requires the `MANAGE_MESSAGES` permission to
    /// be present on the current user.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_user_reaction<E: Into<Emoji> + Send>(
        &self,
        channel: ChannelId,
        message: MessageId,
        user: UserId,
        emoji: E,
    ) -> ClientResult<()> {
        self.delete(DeleteUserReaction(channel, message, emoji.into(), user)).await
    }

    /// Get a list of users that reacted with this emoji.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Vec<User>`
    pub async fn get_reactions<E: Into<Emoji> + Send>(
        &self,
        channel: ChannelId,
        message: MessageId,
        emoji: E,
    ) -> ClientResult<Vec<User>> {
        // todo query params
        self.get(GetReactions(channel, message, emoji.into())).await
    }

    /// Post a typing indicator for the specified channel. Generally bots should not implement this
    /// route. However, if a bot is responding to a command and expects the computation to take a
    /// few seconds, this endpoint may be called to let the user know that the bot is processing
    /// their message. Returns a 204 empty response on success. Fires a
    /// [`TypingStart`](crate::shard::dispatch::TypingStart) event.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn trigger_typing(&self, channel: ChannelId) -> ClientResult<()> {
        self.post_unit(TriggerTyping(channel), "").await
    }

    /// Returns all pinned messages in the channel
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Vec<Message>`
    pub async fn get_pinned_messages(&self, channel: ChannelId) -> ClientResult<Vec<Message>> {
        self.get(GetPinnedMessages(channel)).await
    }

    /// Pin a message in a channel. Requires the `MANAGE_MESSAGES` permission.
    ///
    /// The max pinned messages is 50.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn add_pinned_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<()> {
        self.put_unit(PinMessage(channel, message), "").await
    }

    /// Delete a pinned message in a channel. Requires the `MANAGE_MESSAGES` permission.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_pinned_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<()> {
        self.delete(UnpinMessage(channel, message)).await
    }
}

/// A set of methods on channels that make interacting with messages in that channel easier. Most
/// importantly, the [send](MessageChannelExt::send) method allows for easily sending messages to
/// channels.
///
/// All of the methods this trait provides take a `Client: AsRef<DiscordClient>` parameter in order
/// to make the actual request. See the module level documentation on [bot](../../bot/index.html)
/// for the specific types that implement `AsRef<DiscordClient>`.
#[async_trait]
pub trait MessageChannelExt: Id<Id=ChannelId> {
    /// Send a message to this channel, checking that the bot has all the necessary permissions:
    /// [SEND_MESSAGES](Permissions::SEND_MESSAGES),
    /// [SEND_TTS_MESSAGES](Permissions::SEND_TTS_MESSAGES) if it is a tts message, and
    /// [READ_MESSAGE_HISTORY](Permissions::READ_MESSAGE_HISTORY) if it is a reply. Otherwise,
    /// behaves the same as [MessageChannelExt::send].
    ///
    /// See the documentation for [CreateMessage] to see all of the types that can be used for the
    /// `message` parameter.
    ///
    /// ```rust
    /// # use discorsd::model::message::Message;
    /// # use discorsd::BotState;
    /// # use std::sync::Arc;
    /// # use discorsd::model::ids::ChannelId;
    /// # use discorsd::http::channel::MessageChannelExt;
    /// async fn respond_to_message<B: Send + Sync>(message: Message, state: Arc<BotState<B>>) {
    ///     let channel: ChannelId = message.channel;
    ///     channel.send(
    ///         // also would work: `&state`, `&state.client`
    ///         state,
    ///         // also would work: a `String`, a `RichEmbed`, or an entire `CreateMessage`
    ///         "Hello world!",
    ///     ).await
    ///      .unwrap();
    /// }
    /// ```
    ///
    /// Returns the [Message] object sent in Discord.
    ///
    /// # Errors
    ///
    /// Returns [ClientError::Perms] if the bot is missing any of the necessary permissions, or
    /// an other variant if the request fails for another reason.
    async fn send<State, Msg, B>(&self, state: State, message: Msg) -> ClientResult<Message>
        where State: AsRef<BotState<B>> + Send + Sync,
              B: Send + Sync + 'static,
              Msg: Into<CreateMessage> + Send + Sync,
    {
        let state = state.as_ref();
        let message = message.into();
        let channel = state.cache.channel(self.id()).await.unwrap();
        let perms = Permissions::get_own(&state.cache, &channel).await;
        let check_perms = |perm: Permissions|
            (perms.contains(perm))
                .then(|| ())
                .ok_or(ClientError::Perms(perm));
        check_perms(Permissions::SEND_MESSAGES)?;
        if message.tts {
            check_perms(Permissions::SEND_TTS_MESSAGES)?
        }
        if message.message_reference.is_some() {
            check_perms(Permissions::READ_MESSAGE_HISTORY)?
        }

        state.client.create_message(self.id(), message).await
    }

    /// Try to send a message to this channel, regardless of whether this bot has the necessary
    /// permissions.
    ///
    /// See [Self::send] for documentation.
    async fn send_unchecked<Client, Msg>(&self, client: Client, message: Msg) -> ClientResult<Message>
        where Client: AsRef<DiscordClient> + Send + Sync,
              Msg: Into<CreateMessage> + Send + Sync,
    {
        client.as_ref().create_message(self.id(), message.into()).await
    }

    /// Get all pinned messages in this channel.
    async fn get_pinned_messages<Client>(&self, client: Client) -> ClientResult<Vec<Message>>
        where Client: AsRef<DiscordClient> + Sync + Send,
    {
        let client = client.as_ref();
        client.get_pinned_messages(self.id()).await
    }
}
macro_rules! impl_message_channel_ext {
    ($($types:ty),* $(,)?) => { $(
        impl MessageChannelExt for $types {}
    )* };
}
impl_message_channel_ext!(ChannelId, Channel, TextChannel, DmChannel, GroupDmChannel, NewsChannel);

impl ChannelMessageId {
    /// Edit this message
    ///
    /// # Errors
    ///
    /// See [`DiscordClient::edit_message`](DiscordClient)
    pub async fn edit<Client, Msg>(&self, client: Client, edit: Msg) -> ClientResult<Message>
        where Client: AsRef<DiscordClient> + Send,
              Msg: Into<EditMessage> + Send,
    {
        let client = client.as_ref();
        client.edit_message(self.channel, self.message, edit.into()).await
    }

    /// Delete this message.
    ///
    /// # Errors
    ///
    /// See [`DiscordClient::delete_message`](crate::http::DiscordClient)
    pub async fn delete<Client: AsRef<DiscordClient> + Send>(&self, client: Client) -> ClientResult<()> {
        let client = client.as_ref();
        client.delete_message(self.channel, self.message).await
    }

    /// React to this message
    ///
    /// # Errors
    ///
    /// See [`DiscordClient::create_reaction`](crate::http::DiscordClient)
    pub async fn react<E, Client>(&self, client: Client, emoji: E) -> ClientResult<()>
        where E: Into<Emoji> + Send,
              Client: AsRef<DiscordClient> + Send,
    {
        let client = client.as_ref();
        client.create_reaction(self.channel, self.message, emoji).await
    }

    /// Pin this message
    ///
    /// # Errors
    ///
    /// See [`DiscordClient::add_pinned_message`](crate::http::DiscordClient)
    pub async fn pin<Client: AsRef<DiscordClient> + Send>(&self, client: Client) -> ClientResult<()> {
        let client = client.as_ref();
        client.add_pinned_message(self.channel, self.message).await
    }

    /// Unpin this message
    ///
    /// # Errors
    ///
    /// See [`DiscordClient::delete_pinned_message`](crate::http::DiscordClient)
    pub async fn unpin<Client: AsRef<DiscordClient> + Send>(&self, client: Client) -> ClientResult<()> {
        let client = client.as_ref();
        client.delete_pinned_message(self.channel, self.message).await
    }
}

impl Message {
    /// Edit this message. The value of `self` is updated to the new message as shown in Discord.
    ///
    /// # Errors
    ///
    /// See [`ChannelMessageId::edit`](ChannelMessageId)
    pub async fn edit<Client, Msg>(&mut self, client: Client, edit: Msg) -> ClientResult<()>
        where Client: AsRef<DiscordClient> + Send,
              Msg: Into<EditMessage> + Send,
    {
        *self = self.cmid().edit(client, edit).await?;
        Ok(())
    }

    /// Delete this message.
    ///
    /// # Errors
    ///
    /// See [`ChannelMessageId::delete`](ChannelMessageId)
    pub async fn delete<Client: AsRef<DiscordClient> + Send>(self, client: Client) -> ClientResult<()> {
        self.cmid().delete(client).await
    }

    /// React to this message
    ///
    /// # Errors
    ///
    /// See [`ChannelMessageId::react`](ChannelMessageId)
    pub async fn react<E, Client>(&self, client: Client, emoji: E) -> ClientResult<()>
        where E: Into<Emoji> + Send,
              Client: AsRef<DiscordClient> + Send,
    {
        self.cmid().react(client, emoji).await
    }

    /// Pin this message
    ///
    /// # Errors
    ///
    /// See [`ChannelMessageId::pin`](ChannelMessageId)
    pub async fn pin<Client: AsRef<DiscordClient> + Send>(&self, client: Client) -> ClientResult<()> {
        self.cmid().pin(client).await
    }

    /// Unpin this message
    ///
    /// # Errors
    ///
    /// See [`ChannelMessageId::unpin`](ChannelMessageId)
    pub async fn unpin<Client: AsRef<DiscordClient> + Send>(&self, client: Client) -> ClientResult<()> {
        self.cmid().unpin(client).await
    }
}

/// An attachment (often an image) on a message.
/// Instances of this struct come from its `impl`s of `From<P>, From<(String, P)> where P: AsRef<Path>`
/// (for sending files, with an optionally specified name) and `From<(String, Vec<u8>)>` for sending
/// arbitrary byte streams by name. There also exists `From<(String, AttachmentSource)>` if for some
/// reason you have an [AttachmentSource] already.
///
/// The name will have **any** whitespace removed, since Discord cannot handle file names with
/// spaces.
#[derive(Clone, Debug)]
pub struct MessageAttachment {
    name: String,
    source: AttachmentSource,
}

impl PartialEq for MessageAttachment {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for MessageAttachment {}

impl Hash for MessageAttachment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

// todo consider making Bytes an Rc/Arc?
/// Represents a file that can be used as an attachment in a [CreateMessage].
#[derive(Clone, Debug)]
pub enum AttachmentSource {
    /// Upload the file at this location.
    Path(PathBuf),
    /// Upload a file with these contents.
    Bytes(Vec<u8>),
    /// Attach the file at this url.
    Url(Url),
}

impl AttachmentSource {
    fn into_bytes(self) -> ClientResult<Vec<u8>> {
        match self {
            Self::Path(path) => std::fs::read(path).map_err(ClientError::Io),
            Self::Bytes(bytes) => Ok(bytes),
            // todo might have to make it return just Option<Vec<u8>>,
            Self::Url(_) => Ok(Vec::new())
        }
    }
}

macro_rules! att_from {
    (ref $ty:ty) => {
        impl<'a> From<&'a $ty> for MessageAttachment {
            fn from(path: &'a $ty) -> Self {
                att_from(path)
            }
        }
    };
    ($ty:ty) => {
        impl From<$ty> for MessageAttachment {
            fn from(path: $ty) -> Self {
                att_from(path)
            }
        }
        att_from!(ref $ty);
    };
}

fn att_from<P: AsRef<Path>>(path: P) -> MessageAttachment {
    let path = path.as_ref();
    let name = path.file_name()
        .expect("attachments must have a name")
        .to_string_lossy()
        .to_string();
    (name, path).into()
}

// can't do `impl<P: AsRef<Path>> From<P> for MessageAttachment { ... }` because `(String, T)`
// "could" implement `AsRef<Path>` in the future (even though it definitely never will).
// Instead, just macro it up ig
att_from!(ref Path);
att_from!(PathBuf);
att_from!(ref str);
att_from!(String);
att_from!(ref std::ffi::OsStr);
att_from!(std::ffi::OsString);

impl<'a, S: ToString> From<(S, &'a Path)> for MessageAttachment {
    fn from((name, path): (S, &'a Path)) -> Self {
        (name, AttachmentSource::Path(path.into())).into()
    }
}

impl<S: ToString> From<(S, Vec<u8>)> for MessageAttachment {
    fn from((name, bytes): (S, Vec<u8>)) -> Self {
        (name, AttachmentSource::Bytes(bytes)).into()
    }
}

impl<S: ToString> From<(S, Url)> for MessageAttachment {
    fn from((name, url): (S, Url)) -> Self {
        (name, AttachmentSource::Url(url)).into()
    }
}

impl MessageAttachment {
    /// Get an attachment that will attach a file at some `url` to a message.
    pub fn from_url<S: ToString, U: IntoUrl>(name: S, url: U) -> reqwest::Result<Self> {
        Ok(Self::from((name, url.into_url()?)))
    }
}

impl<S: ToString> From<(S, AttachmentSource)> for MessageAttachment {
    fn from((name, source): (S, AttachmentSource)) -> Self {
        let mut name = name.to_string();
        name.retain(|c| !c.is_ascii_whitespace());
        Self { name, source }
    }
}

/// Sent to Discord to create a message with [`DiscordClient::create_message`]. The easist way to
/// send a method is using the [MessageChannelExt::send] method, which takes accepts any type that
/// implements `Into<CreateMessage>`, as described below.
///
/// This can be created most easily `From`:
/// * any type that impls `Into<Cow<'static, str>>` (most notably
/// `&'static str` and `String`), using that string as the [content](CreateMessage::content),
/// * [RichEmbed], which is an embed builder type, using that as the [embed](CreateMessage::embed),
/// * [MessageAttachment], using that attachment as the single attachment in
/// [files](CreateMessage::files),
/// * [Message], using that message's content, its first embed, if it is tts, etc,
/// * and of course [CreateMessage] itself.
///
/// This type also uses the builder pattern for field by field configuration and construction, if
/// desired. This starts with either [create_message] or [CreateMessage::build].
#[derive(Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CreateMessage {
    /// the message contents (up to 2000 characters)
    pub content: Cow<'static, str>,
    /// a nonce that can be used for optimistic message sending
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<u64>,
    /// true if this is a TTS message
    pub tts: bool,
    /// the contents of the file being sent
    #[serde(skip_serializing)]
    pub files: HashSet<MessageAttachment>,
    /// embedded rich content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<RichEmbed>,
    /// allowed mentions for a message
    pub allowed_mentions: Option<AllowedMentions>,
    /// include to make your message a reply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
}

impl<S: Into<Cow<'static, str>>> From<S> for CreateMessage {
    fn from(s: S) -> Self {
        let mut msg = Self::default();
        msg.content(s);
        msg
    }
}

impl From<RichEmbed> for CreateMessage {
    fn from(e: RichEmbed) -> Self {
        Self { embed: Some(e), ..Default::default() }
    }
}

impl From<MessageAttachment> for CreateMessage {
    fn from(att: MessageAttachment) -> Self {
        let mut msg = Self::default();
        msg.files.insert(att);
        msg
    }
}

impl From<Message> for CreateMessage {
    fn from(mut message: Message) -> Self {
        Self {
            content: message.content.into(),
            nonce: None,
            tts: message.tts,
            // todo verify that this works
            files: message.attachments.into_iter()
                .map(|a| MessageAttachment::from_url(a.filename, &a.url).unwrap())
                .collect(),
            embed: message.embeds.pop().map(|embed| embed.into()),
            // todo
            allowed_mentions: None,
            message_reference: message.message_reference,
        }
    }
}

/// Easily build a message to create in some channel with [MessageChannelExt::send] and similar
/// methods. Most useful for creating messages with both content and an embed, since
/// [CreateMessage]'s `From` impls provide simpler ways to get a [CreateMessage] for just one of
/// them.
///
/// ```rust
/// # use discorsd::http::channel::create_message;
/// create_message(|m| {
///     m.content("Message Content");
///     m.embed(|e| {
///         e.title("Embed Title Too!")
///     })
/// });
/// ```
pub fn create_message<F: FnOnce(&mut CreateMessage)>(builder: F) -> CreateMessage {
    CreateMessage::build(builder)
}

impl CreateMessage {
    /// Easily build a message to create in some channel with [MessageChannelExt::send] and similar
    /// methods. Most useful for creating messages with both content and an embed, since
    /// `CreateMessage`'s `From` impls provide simpler ways to get a `CreateMessage` for just one of
    /// them.
    ///
    /// ```rust
    /// # use discorsd::http::channel::CreateMessage;
    /// CreateMessage::build(|m| {
    ///     m.content("Message Content");
    ///     m.embed(|e| {
    ///         e.title("Embed Title Too!")
    ///     })
    /// });
    /// ```
    pub fn build<F: FnOnce(&mut Self)>(builder: F) -> Self {
        Self::build_with(Self::default(), builder)
    }

    /// Build a `CreateMessage` by modifying an already existing `CreateMessage`.
    pub fn build_with<F: FnOnce(&mut Self)>(mut message: Self, builder: F) -> Self {
        builder(&mut message);
        message
    }

    /// Set this message's [content](Self::content).
    pub fn content<S: Into<Cow<'static, str>>>(&mut self, content: S) {
        self.content = content.into();
    }

    /// Build a [RichEmbed] and attach it to this message.
    pub fn embed<F: FnOnce(&mut RichEmbed)>(&mut self, builder: F) {
        let embed = self.embed.take().unwrap_or_default();
        self.embed_with(embed, builder);
    }

    /// Build a [RichEmbed] by modifying an already existing `RichEmbed` and attach it to this
    /// message.
    pub fn embed_with<F: FnOnce(&mut RichEmbed)>(&mut self, embed: RichEmbed, builder: F) {
        self.embed = Some(RichEmbed::build(embed, builder));
    }

    /// Attach an image to this message. See [MessageAttachment] for details about what types impl
    /// `Into<MessageAttachment>`.
    pub fn image<A: Into<MessageAttachment>>(&mut self, attachment: A) {
        self.files.insert(attachment.into());
    }

    // todo also set whether it pings the message sender or not
    /// Send this message a a reply to another message.
    pub fn reply(&mut self, message: MessageId) {
        self.message_reference = Some(MessageReference::reply(message));
    }
}

/// A builder for an embed in a message.
#[derive(Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RichEmbed {
    /// title of embed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    /// description of embed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    /// url of embed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Cow<'static, str>>,
    /// timestamp of embed content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    /// color code of the embed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// footer information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<EmbedFooter>,
    /// image information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<EmbedImage>,
    /// thumbnail information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<EmbedThumbnail>,
    /// video information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<EmbedVideo>,
    /// provider information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<EmbedProvider>,
    /// author information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<EmbedAuthor>,
    /// fields information
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EmbedField>,
    /// files, passed off to message_create.
    #[serde(skip_serializing)]
    pub(crate) files: HashSet<MessageAttachment>,
}

impl From<Embed> for RichEmbed {
    fn from(embed: Embed) -> Self {
        Self {
            title: embed.title.map(Cow::from),
            description: embed.description.map(Cow::from),
            url: embed.url.map(Cow::from),
            timestamp: embed.timestamp,
            color: embed.color,
            footer: embed.footer,
            image: embed.image,
            thumbnail: embed.thumbnail,
            video: embed.video,
            provider: embed.provider,
            author: embed.author,
            fields: embed.fields,
            files: HashSet::new(),
        }
    }
}

/// Easily build an embed to be sent in a message.
///
/// ```rust
/// # use discorsd::http::ClientResult;
/// # use std::sync::Arc;
/// # use discorsd::model::message::{Message, Color};
/// # use discorsd::BotState;
/// # use discorsd::http::channel::{MessageChannelExt, embed};
/// async fn respond_to_message<B: Send + Sync>(message: Message, state: Arc<BotState<B>>) -> ClientResult<Message> {
///     message.channel.send(state, embed(|e| {
///         e.title("My Embed Title");
///         e.color(Color::RED);
///         e.timestamp_now();
///         // etc...
///     })).await
/// }
/// ```
pub fn embed<F: FnOnce(&mut RichEmbed)>(f: F) -> RichEmbed {
    RichEmbed::build_new(f)
}

/// Similar to [embed], but works by modifying an existing embed rather than creating an entire new
/// [RichEmbed]. [Embed]s already sent to Discord implement `Into<RichEmbed>`.
///
/// ```rust
/// # use discorsd::http::ClientResult;
/// # use std::sync::Arc;
/// # use discorsd::model::message::{Message, Color};
/// # use discorsd::BotState;
/// # use discorsd::http::channel::{MessageChannelExt, embed, embed_with};
/// # use std::borrow::{Cow, Borrow};
/// async fn respond_to_message<B: Send + Sync>(mut message: Message, state: Arc<BotState<B>>) -> ClientResult<Message> {
///     let embed = message.embeds.pop().unwrap_or_else(Default::default);
///     message.channel.send(state, embed_with(embed.into(), |e| {
///         e.title("A new embed, with an old description");
///         let old_description = e.description.as_ref()
///             .map(Cow::borrow)
///             .unwrap_or("{empty}")
///             .to_owned();
///         e.description(format!("The old description was: {}", old_description));
///         e.color(Color::RED);
///         e.timestamp_now();
///         // etc...
///     })).await
/// }
/// ```
pub fn embed_with<F: FnOnce(&mut RichEmbed)>(embed: RichEmbed, f: F) -> RichEmbed {
    embed.build(f)
}

impl RichEmbed {
    /// Easily build an embed to be sent in a message.
    ///
    /// ```rust
    /// # use discorsd::model::message::Color;
    /// # use discorsd::http::channel::embed;
    /// embed(|e| {
    ///     e.title("My Embed Title");
    ///     e.color(Color::RED);
    ///     e.timestamp_now();
    ///     // etc...
    /// });
    /// ```
    pub fn build_new<F: FnOnce(&mut Self)>(builder: F) -> Self {
        Self::build(Self::default(), builder)
    }

    /// Build a `CreateMessage` by modifying an already existing `CreateMessage`.
    pub fn build<F: FnOnce(&mut Self)>(mut self, builder: F) -> Self {
        builder(&mut self);
        self
    }

    // todo images of each of these? maybe just one image of all of them set
    /// Set this embed's [title](Self::title).
    pub fn title<S: Into<Cow<'static, str>>>(&mut self, title: S) {
        self.title = Some(title.into());
    }

    /// Set this embed's [description](Self::description).
    pub fn description<S: Into<Cow<'static, str>>>(&mut self, description: S) {
        self.description = Some(description.into());
    }

    /// Set this embed's [url](Self::url), which makes the embed's [title](Self::title) a hyperlink
    /// if it is set, or makes this url the title if not.
    pub fn url<S: Into<Cow<'static, str>>>(&mut self, url: S) {
        self.url = Some(url.into());
    }

    /// Sets this embed's [timestamp](Self::timestamp).
    ///
    /// To set the timestamp to the current time, use [timestamp_now](Self::timestamp_now).
    pub fn timestamp<Tz: TimeZone>(&mut self, timestamp: &DateTime<Tz>) {
        self.timestamp = Some(timestamp.with_timezone(&Utc));
    }

    /// Sets this embed's [timestamp](Self::timestamp) to the current time.
    ///
    /// To set the timestamp to an arbitrary time, use [timestamp](Self::timestamp).
    pub fn timestamp_now(&mut self) {
        self.timestamp = Some(chrono::Utc::now());
    }

    /// Sets this embed's [color](Self::color).
    pub fn color(&mut self, color: Color) {
        self.color = Some(color);
    }

    /// Adds a footer to this embed with the specified text.
    ///
    /// To add a footer with an image, use [footer](Self::footer).
    pub fn footer_text<S: ToString>(&mut self, footer: S) {
        self.footer = Some(EmbedFooter::new(footer));
    }

    // todo example because of `A`
    /// Adds a footer to this embed with the specified text and image.
    ///
    /// To add a footer just text, use [footer_text](Self::footer_text).
    pub fn footer<S: ToString, A: Into<MessageAttachment>>(&mut self, text: S, icon: A) {
        let attachment = icon.into();
        self.footer = Some(EmbedFooter::with_icon(text, format!("attachment://{}", attachment.name)));
        self.files.insert(attachment);
    }

    /// Attach an image to this embed.
    pub fn image<A: Into<MessageAttachment>>(&mut self, image: A) {
        let attachment = image.into();
        self.image = Some(EmbedImage::new(format!("attachment://{}", attachment.name)));
        self.files.insert(attachment);
    }

    /// Attach a thumbnail to this embed.
    pub fn thumbnail<A: Into<MessageAttachment>>(&mut self, image: A) {
        let attachment = image.into();
        self.thumbnail = Some(EmbedThumbnail::new(format!("attachment://{}", attachment.name)));
        self.files.insert(attachment);
    }

    /// Set the embed's [author](Self::author) based on a [User]'s name and icon url.
    pub fn authored_by(&mut self, user: &User) {
        self.author = Some(user.into());
        // self.files.insert(path.to_string_lossy().to_string(), path.to_path_buf());
    }

    // // todo figure out how to get the file nicely and add it to `self.files`.
    // //  Probably best way is to take an `EmbedAuthor` and somehow check if it needs to upload files?
    // //  maybe not, that could be hard/inconsistent (like if they use this with a User::into().
    // //  maybe a param `needs_upload: bool`?
    // pub fn author<S: ToString, U: ToString, I: AsRef<Path>>(&mut self, name: S, url: U, icon_url: I) -> &mut Self {
    //     todo!("see above");
    //     // self.author = Some(EmbedAuthor {
    //     //     name: Some(),
    //     //     url: None,
    //     //     icon_url: None,
    //     //     proxy_icon_url: None
    //     // });
    // }

    /// Adds a new field to this embed with the specified name and value.
    ///
    /// To add an inline field, use [add_inline_field](Self::add_inline_field).
    ///
    /// # Panics
    ///
    /// Panics if either `name` or `value` are empty. To add a blank field, use
    /// [add_blank_field](Self::add_blank_field).
    pub fn add_field<S: ToString, V: ToString>(&mut self, name: S, value: V) {
        self.field(EmbedField::new(name, value))
    }

    /// Adds a new inline field to this embed with the specified name and value. Inline fields will
    /// be placed side-by-side in the embed.
    ///
    /// # Panics
    ///
    /// Panics if either `name` or `value` are empty. To add a blank inline field, use
    /// [add_blank_inline_field](Self::add_blank_inline_field).
    pub fn add_inline_field<S: ToString, V: ToString>(&mut self, name: S, value: V) {
        self.field(EmbedField::new_inline(name, value))
    }

    /// Adds a blank field ([EmbedField::blank]) to this embed. Useful for spacing embed fields.
    pub fn add_blank_field(&mut self) {
        self.field(EmbedField::blank())
    }

    /// Adds a blank inline field ([EmbedField::blank_inline]) to this embed. Useful for spacing
    /// embed fields.
    pub fn add_blank_inline_field(&mut self) {
        self.field(EmbedField::blank_inline())
    }

    /// Adds a field to this embed. `Into<EmbedField>` is implemented on `(N, V)` and `(N, V, bool)`,
    /// where `N: ToString, V: ToString`. `N` is the name for the field, `V` is the value, and if
    /// a bool is present, it determines if the field is inline (the 2 element tuple is not inline).
    ///
    /// The [add_field](Self::add_field) method may be easier to use in most situations.
    ///
    /// All of the three below calls to `field` below add the same field to `e`.
    /// ```rust
    /// # use discorsd::http::channel::embed;
    /// # use discorsd::model::message::EmbedField;
    /// embed(|e| {
    ///     e.field(EmbedField::new("name", "value"));
    ///     e.field(("name", "value"));
    ///     e.field(("name", "value", false));
    /// });
    /// ```
    pub fn field<F: Into<EmbedField>>(&mut self, field: F) {
        self.fields.push(field.into());
    }

    // todo when the array `into_iter` is fixed switch to that.
    // todo when const-panicking or something exists, all of the types that impl Into<Field> will
    //  be const constructable, so this can take a &'static dyn Into<Field>
    /// Adds multiple fields to this embed. See [field](Self::field) for more information on adding
    /// fields.
    ///
    /// ```rust
    /// # use discorsd::http::channel::embed;
    /// # use discorsd::model::message::EmbedField;
    /// embed(|e| {
    ///     e.fields([
    ///         ("name 1", "value 1", false),
    ///         ("inline 1", "inline value 1", true),
    ///         EmbedField::blank_inline_tuple(),
    ///         ("inline 2", "inline value 2", true),
    ///     ].iter().copied())
    /// });
    /// ```
    pub fn fields<F, I>(&mut self, fields: I)
        where F: Into<EmbedField>,
              I: IntoIterator<Item=F> {
        self.fields.extend(fields.into_iter().map(F::into));
    }
}

/// Sent to Discord to create a message with [`DiscordClient::edit_message`]. To create an
/// `EditMessage`, use one of [EditMessage::build], `From<RichEmbed>`, or `From<Message>`.
///
/// Params with nested `Option`s are serialized as follows:
///
/// `None` => field is not changed
///
/// `Some(None)` => field is removed (at least one of `content`, `embed`) must be present on a message
///
/// `Some(Some(foo))` => field is edited to be `foo`
#[derive(Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct EditMessage {
    /// The new contents of the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Option<Cow<'static, str>>>,
    /// The new embed for the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<Option<RichEmbed>>,
    /// Only [SUPPRESS_EMBEDS](MessageFlags::SUPPRESS_EMBEDS) can be set/unset, but trying to send
    /// other flags is not an error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// New allowed mentions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
}

impl<S: Into<Cow<'static, str>>> From<S> for EditMessage {
    fn from(s: S) -> Self {
        let mut msg = Self::default();
        msg.content(s);
        msg
    }
}

impl From<RichEmbed> for EditMessage {
    fn from(e: RichEmbed) -> Self {
        Self { embed: Some(Some(e)), ..Default::default() }
    }
}

impl From<Message> for EditMessage {
    fn from(mut message: Message) -> Self {
        Self {
            content: Some(Some(message.content.into())),
            embed: Some(message.embeds.pop().map(RichEmbed::from)),
            flags: message.flags,
            // todo
            allowed_mentions: None,
        }
    }
}

impl EditMessage {
    /// Build an [EditMessage] for use [Message::edit] and other similar methods.
    ///
    /// ```rust
    /// # use discorsd::http::ClientResult;
    /// # use std::sync::Arc;
    /// # use discorsd::model::message::{Message, Color};
    /// # use discorsd::BotState;
    /// # use discorsd::http::channel::{MessageChannelExt, embed, EditMessage};
    /// async fn respond_to_message<B: Send + Sync>(mut message: Message, state: Arc<BotState<B>>) -> ClientResult<()> {
    ///     message.edit(state, EditMessage::build(|m| {
    ///          m.clear_content();
    ///          m.embed(|e| {
    ///              e.title("Now there's an embed!")
    ///          })
    ///      })).await
    /// }
    /// ```
    pub fn build<F: FnOnce(&mut Self)>(f: F) -> Self {
        Self::build_with(Self::default(), f)
    }

    /// Similar to [build](Self::build), but modifies an already existing [EditMessage] rather than
    /// creating a new one.
    pub fn build_with<F: FnOnce(&mut Self)>(mut edit: Self, f: F) -> Self {
        f(&mut edit);
        edit
    }

    /// Edit the content of this message.
    pub fn content<S: Into<Cow<'static, str>>>(&mut self, content: S) {
        self.content = Some(Some(content.into()));
    }

    /// Clear the content of this message.
    pub fn clear_content(&mut self) {
        self.content = Some(None);
    }

    /// Edit the embed of this message. The `&mut RichEmbed` your `builder` function operates on is
    /// the current [embed](Self::embed) of this [EditMessage], for instance if this [EditMessage]
    /// was created with the impl `From<Message>` or `From<RichEmbed>`.
    pub fn embed<F: FnOnce(&mut RichEmbed)>(&mut self, builder: F) {
        let embed = self.embed.as_mut()
            .and_then(Option::take)
            .unwrap_or_default();
        self.embed = Some(Some(RichEmbed::build(embed, builder)));
    }

    /// Clear the embed of this message.
    pub fn clear_embed(&mut self) {
        self.embed = Some(None);
    }
}

pub(in super) trait MessageWithFiles: Serialize {
    /// yeet the files out of `self`
    fn take_files(&mut self) -> HashSet<MessageAttachment>;

    /// true if content, embeds, etc are present
    fn has_other_content(&self) -> bool;
}

impl DiscordClient {
    pub(in super) async fn send_message_with_files<M: MessageWithFiles + Send + Sync>(
        &self,
        route: Route,
        mut message: M,
    ) -> ClientResult<Message> {
        let files = message.take_files();
        if files.is_empty() {
            self.post(route, message).await
        } else {
            let files = files.into_iter()
                .map(|MessageAttachment { name, source }|
                    source.into_bytes().map(|contents| (name, contents))
                )
                .collect::<ClientResult<Vec<(String, Vec<u8>)>>>()?;
            let make_multipart = || {
                let mut form = files
                    .clone()
                    .into_iter()
                    .map(|(name, contents)| Part::bytes(contents).file_name(name))
                    .enumerate()
                    .fold(Form::new(), |form, (i, part)| form.part(i.to_string(), part));
                if message.has_other_content() {
                    form = form.text("payload_json", serde_json::to_string(&message).ok()?);
                }
                Some(form)
            };
            self.post_multipart(route, make_multipart).await
        }
    }
}

impl MessageWithFiles for CreateMessage {
    fn take_files(&mut self) -> HashSet<MessageAttachment> {
        use std::mem;
        let mut files = mem::take(&mut self.files);
        if let Some(embed) = &mut self.embed {
            files.extend(mem::take(&mut embed.files));
        }
        files
    }

    fn has_other_content(&self) -> bool {
        !self.content.is_empty() || self.embed.is_some()
    }
}

impl MessageWithFiles for WebhookMessage {
    fn take_files(&mut self) -> HashSet<MessageAttachment> {
        use std::mem;
        let mut files = mem::take(&mut self.files);
        files.extend(
            self.embeds.iter_mut()
                .map(|e| &mut e.files)
                .flat_map(mem::take)
        );
        files
    }

    fn has_other_content(&self) -> bool {
        !self.content.is_empty() || !self.embeds.is_empty()
    }
}