use std::borrow::Cow;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use reqwest::multipart::{Form, Part};
use serde::Serialize;

use crate::http::{ClientError, DiscordClient};
use crate::http::ClientResult;
use crate::http::interaction::WebhookMessage;
use crate::http::model::channel::Channel;
use crate::http::model::emoji::Emoji;
use crate::http::model::Id;
use crate::http::model::ids::{ChannelId, MessageId, UserId};
use crate::http::model::message::*;
use crate::http::model::user::User;
use crate::http::routes::Route::*;
use crate::http::routes::Route;

/// Channel related http requests
impl DiscordClient {
    /// Get a channel by ID. Returns a [channel](Channel) object.
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
    /// requires the 'READ_MESSAGE_HISTORY' permission to be present on the current user.
    pub async fn get_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<Message> {
        self.get(GetMessage(channel, message)).await
    }

    /// Post a message in the specified channel
    pub async fn create_message(&self, channel: ChannelId, message: CreateMessage) -> ClientResult<Message> {
        self.send_message_with_files(PostMessage(channel), message).await
    }

    /// [content](content): the new message contents (up to 2000 characters)
    ///
    /// [embed](embed): embedded rich content
    ///
    /// [flags](flags): edit the flags of a message (only `SUPPRESS_EMBEDS` can currently be set/unset)
    pub async fn edit_message(&self, channel: ChannelId, message: MessageId, edit: EditMessage) -> ClientResult<Message> {
        // not an error to send other flags
        // let flags = flags & MessageFlags::SUPPRESS_EMBEDS;
        self.patch(EditMessage(channel, message), edit).await
    }

    /// Delete a message. If operating on a guild channel and trying to delete a message that was
    /// not sent by the current user, this endpoint requires the `MANAGE_MESSAGES` permission.
    /// Fires a [MessageDelete](crate::shard::dispatch::DispatchPayload::MessageDelete) event.
    pub async fn delete_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<()> {
        self.delete(DeleteMessage(channel, message)).await
    }

    /// Create a reaction for the message. This endpoint requires the 'READ_MESSAGE_HISTORY'
    /// permission to be present on the current user. Additionally, if nobody else has reacted to
    /// the message using this emoji, this endpoint requires the 'ADD_REACTIONS' permission to be
    /// present on the current user.
    pub async fn create_reaction(&self, channel: ChannelId, message: MessageId, emoji: impl Into<Emoji>) -> ClientResult<()> {
        self.put_unit(CreateReaction(channel, message, emoji.into())).await
    }

    /// Delete a reaction the current user has made for the message.
    pub async fn delete_own_reaction(&self, channel: ChannelId, message: MessageId, emoji: impl Into<Emoji>) -> ClientResult<()> {
        self.delete(DeleteOwnReaction(channel, message, emoji.into())).await
    }

    /// Deletes another user's reaction. This endpoint requires the 'MANAGE_MESSAGES' permission to
    /// be present on the current user.
    pub async fn delete_user_reaction(&self, channel: ChannelId, message: MessageId, user: UserId, emoji: impl Into<Emoji>) -> ClientResult<()> {
        self.delete(DeleteUserReaction(channel, message, emoji.into(), user)).await
    }

    /// Post a typing indicator for the specified channel. Generally bots should not implement this
    /// route. However, if a bot is responding to a command and expects the computation to take a
    /// few seconds, this endpoint may be called to let the user know that the bot is processing
    /// their message. Returns a 204 empty response on success. Fires a
    /// [TypingStart](crate::shard::dispatch::DispatchPayload::TypingStart) event.
    pub async fn trigger_typing(&self, channel: ChannelId) -> ClientResult<()> {
        self.post_unit(TriggerTyping(channel), "").await
    }

    /// Returns all pinned messages in the channel
    pub async fn get_pinned_messages(&self, channel: ChannelId) -> ClientResult<Vec<Message>> {
        self.get(GetPinnedMessages(channel)).await
    }

    /// Pin a message in a channel. Requires the `MANAGE_MESSAGES` permission.
    ///
    /// The max pinned messages is 50.
    pub async fn add_pinned_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<()> {
        self.put_unit(PinMessage(channel, message)).await
    }

    /// Delete a pinned message in a channel. Requires the `MANAGE_MESSAGES` permission.
    pub async fn delete_pinned_message(&self, channel: ChannelId, message: MessageId) -> ClientResult<()> {
        self.delete(UnpinMessage(channel, message)).await
    }
}

#[async_trait]
pub trait ChannelExt: Id<Id=ChannelId> {
    // todo: take cache too and make sure we have permissions to post messages
    async fn send<Client, Msg>(&self, client: Client, message: Msg) -> ClientResult<Message>
        where Client: AsRef<DiscordClient> + Sync + Send,
              Msg: Into<CreateMessage> + Sync + Send,
    {
        client.as_ref().create_message(self.id(), message.into()).await
    }
}

impl<C: Id<Id=ChannelId>> ChannelExt for C {}

impl MessageRef {
    /// Edit this message
    pub async fn edit<Client, Msg>(&self, client: Client, edit: Msg) -> ClientResult<Message>
        where Client: AsRef<DiscordClient>,
              Msg: Into<EditMessage>,
    {
        client.as_ref().edit_message(self.channel, self.message, edit.into()).await
    }

    /// Delete this message.
    pub async fn delete<Client: AsRef<DiscordClient>>(&self, client: Client) -> ClientResult<()> {
        client.as_ref().delete_message(self.channel, self.message).await
    }

    pub async fn react<E, Client>(&self, client: Client, emoji: E) -> ClientResult<()>
        where E: Into<Emoji>,
              Client: AsRef<DiscordClient>,
    {
        client.as_ref()
            .create_reaction(self.channel, self.message, emoji)
            .await
    }

    pub async fn pin<Client: AsRef<DiscordClient>>(&self, client: Client) -> ClientResult<()> {
        client.as_ref().add_pinned_message(self.channel, self.message).await
    }

    pub async fn unpin<Client: AsRef<DiscordClient>>(&self, client: Client) -> ClientResult<()> {
        client.as_ref().delete_pinned_message(self.channel, self.message).await
    }
}

impl Message {
    /// Edit this message. The value of `self` is updated to the new message as shown in Discord
    pub async fn edit<Client, Msg>(&mut self, client: Client, edit: Msg) -> ClientResult<()>
        where Client: AsRef<DiscordClient>,
              Msg: Into<EditMessage>,
    {
        *self = MessageRef::from(&*self).edit(client, edit).await?;
        Ok(())
    }

    /// Delete this message.
    pub async fn delete<Client: AsRef<DiscordClient>>(self, client: Client) -> ClientResult<()> {
        MessageRef::from(self).delete(client).await
    }

    pub async fn react<E, Client>(&self, client: Client, emoji: E) -> ClientResult<()>
        where E: Into<Emoji>,
              Client: AsRef<DiscordClient>,
    {
        MessageRef::from(self).react(client, emoji).await
    }

    pub async fn pin<Client: AsRef<DiscordClient>>(&self, client: Client) -> ClientResult<()> {
        MessageRef::from(self).pin(client).await
    }

    pub async fn unpin<Client: AsRef<DiscordClient>>(&self, client: Client) -> ClientResult<()> {
        MessageRef::from(self).unpin(client).await
    }
}

/// what is sent to Discord to create a message with [DiscordClient::create_message]
#[derive(Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CreateMessage {
    /// the message contents (up to 2000 characters)
    pub content: String,
    /// a nonce that can be used for optimistic message sending
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<u64>,
    /// true if this is a TTS message
    pub tts: bool,
    /// the contents of the file being sent
    #[serde(skip_serializing)]
    pub files: HashMap<String, PathBuf>,
    /// embedded rich content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<RichEmbed>,
    /// allowed mentions for a message
    pub allowed_mentions: Option<AllowedMentions>,
    /// include to make your message a reply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
}

impl<S: ToString> From<S> for CreateMessage {
    fn from(s: S) -> Self {
        let mut msg = CreateMessage::default();
        msg.content(s);
        msg
    }
}

impl From<RichEmbed> for CreateMessage {
    fn from(e: RichEmbed) -> Self {
        let mut msg = CreateMessage::default();
        msg.embed = Some(e);
        msg
    }
}

pub fn create_message<F: FnOnce(&mut CreateMessage)>(builder: F) -> CreateMessage {
    CreateMessage::build(builder)
}

impl CreateMessage {
    pub fn build<F: FnOnce(&mut CreateMessage)>(builder: F) -> Self {
        Self::build_with(Self::default(), builder)
    }

    pub fn build_with<F: FnOnce(&mut CreateMessage)>(mut message: Self, builder: F) -> Self {
        builder(&mut message);
        message
    }

    pub fn content<S: ToString>(&mut self, content: S) {
        self.content = content.to_string();
    }

    pub fn embed<F: FnOnce(&mut RichEmbed)>(&mut self, builder: F) {
        let embed = self.embed.take().unwrap_or_default();
        self.embed = Some(RichEmbed::build_with(embed, builder));
    }

    pub fn image<P: AsRef<Path>>(&mut self, image: P) {
        let path = image.as_ref();
        let mut name = path.file_name()
            .expect("uploaded files must have a name")
            .to_string_lossy()
            .to_string();
        name.retain(|c| !c.is_whitespace());
        self.files.insert(name, path.to_path_buf());
    }

    pub fn reply(&mut self, message: MessageId) {
        self.message_reference = Some(MessageReference::reply(message));
    }
}

// todo maybe ~~some~~ ALL of these can be Cows?
/// Builder for Embeds
#[derive(Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RichEmbed {
    /// title of embed
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    /// description of embed
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// url of embed
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    /// timestamp of embed content
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<DateTime<Utc>>,
    /// color code of the embed
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<Color>,
    /// footer information
    #[serde(skip_serializing_if = "Option::is_none")]
    footer: Option<EmbedFooter>,
    /// image information
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<EmbedImage>,
    /// thumbnail information
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<EmbedThumbnail>,
    /// author information
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<EmbedAuthor>,
    /// fields information
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    fields: Vec<EmbedField>,
    /// files, passed off to message_create. the lifetimes (?) are to other things in `RichEmbed`
    #[serde(skip_serializing)]
    pub(crate) files: HashMap<String, PathBuf>,
    // files: HashMap<String, PathBuf>,
}

pub fn embed<F: FnOnce(&mut RichEmbed)>(f: F) -> RichEmbed {
    RichEmbed::build(f)
}

pub fn embed_with<F: FnOnce(&mut RichEmbed)>(embed: RichEmbed, f: F) -> RichEmbed {
    RichEmbed::build_with(embed, f)
}

impl RichEmbed {
    pub fn build<F: FnOnce(&mut Self)>(builder: F) -> RichEmbed {
        Self::build_with(Self::default(), builder)
    }

    pub fn build_with<F: FnOnce(&mut Self)>(mut embed: Self, builder: F) -> RichEmbed {
        builder(&mut embed);
        embed
    }

    pub fn title<S: ToString>(&mut self, title: S) {
        self.title = Some(title.to_string());
    }

    pub fn description<S: ToString>(&mut self, description: S) {
        self.description = Some(description.to_string());
    }

    pub fn url<S: ToString>(&mut self, url: S) {
        self.url = Some(url.to_string());
    }

    pub fn timestamp<Tz: TimeZone>(&mut self, timestamp: DateTime<Tz>) {
        self.timestamp = Some(timestamp.with_timezone(&Utc));
    }

    pub fn timestamp_now(&mut self) {
        self.timestamp = Some(chrono::Utc::now());
    }

    pub fn color(&mut self, color: Color) {
        self.color = Some(color);
    }

    pub fn footer_text<S: ToString>(&mut self, footer: S) {
        self.footer = Some(EmbedFooter::new(footer));
    }

    pub fn footer<S: ToString, P: AsRef<Path>>(&mut self, text: S, icon_url: P) {
        let path = icon_url.as_ref();
        let name = path.file_name()
            .expect("uploaded files must have a name")
            .to_string_lossy();
        self.footer = Some(EmbedFooter::with_icon(text, format!("attachment://{}", name)));
        self.files.insert(name.to_string(), path.to_path_buf());
    }

    pub fn image<P: AsRef<Path>>(&mut self, image: P) {
        let path = image.as_ref();
        let mut name = path.file_name()
            .expect("uploaded files must have a name")
            .to_string_lossy()
            .to_string();
        name.retain(|c| !c.is_whitespace());
        self.image = Some(EmbedImage::new(format!("attachment://{}", name)));
        self.files.insert(name, path.to_path_buf());
    }

    pub fn thumbnail<P: AsRef<Path>>(&mut self, image: P) {
        let path = image.as_ref();
        let name = path.file_name()
            .expect("uploaded files must have a name")
            .to_string_lossy();
        self.thumbnail = Some(EmbedThumbnail::new(format!("attachment://{}", name)));
        self.files.insert(name.to_string(), path.to_path_buf());
    }

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

    pub fn add_field<S: ToString, V: ToString>(&mut self, name: S, value: V) {
        self.field(EmbedField::new(name, value))
    }

    pub fn add_inline_field<S: ToString, V: ToString>(&mut self, name: S, value: V) {
        self.field(EmbedField::new_inline(name, value))
    }

    pub fn add_blank_field(&mut self) {
        self.field(EmbedField::blank())
    }

    pub fn add_blank_inline_field(&mut self) {
        self.field(EmbedField::blank_inline())
    }

    pub fn field<F: Into<EmbedField>>(&mut self, field: F) {
        self.fields.push(field.into());
    }

    pub fn fields<F, I>(&mut self, fields: I)
        where F: Into<EmbedField>,
              I: IntoIterator<Item=F> {
        self.fields.extend(fields.into_iter().map(F::into));
    }
}

impl Embed {
    pub fn build<F: FnOnce(&mut RichEmbed)>(self, builder: F) -> RichEmbed {
        let Self { title, description, url, timestamp, color, footer, image, thumbnail, author, fields, .. } = self;
        let mut rich = RichEmbed {
            title,
            description,
            url,
            timestamp,
            color,
            footer,
            image,
            thumbnail,
            author,
            fields,
            files: Default::default(),
        };
        builder(&mut rich);
        rich
    }
}

/// params with nested `Option`s are serialized as follows:
///
/// `None` => field is not changed
///
/// `Some(None)` => field is removed (at least one of `content`, `embed`) must be present on a message
///
/// `Some(Some(foo))` => field is edited to be `foo`
#[derive(Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct EditMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Option<Cow<'static, str>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<Option<RichEmbed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
}

impl<S: Into<Cow<'static, str>>> From<S> for EditMessage {
    fn from(s: S) -> Self {
        let mut msg = EditMessage::default();
        msg.content(s);
        msg
    }
}

impl From<RichEmbed> for EditMessage {
    fn from(e: RichEmbed) -> Self {
        let mut msg = EditMessage::default();
        msg.embed = Some(Some(e));
        msg
    }
}

impl EditMessage {
    pub fn build<F: FnOnce(&mut EditMessage)>(f: F) -> Self {
        Self::build_with(Self::default(), f)
    }

    pub fn build_with<F: FnOnce(&mut EditMessage)>(mut edit: Self, f: F) -> Self {
        f(&mut edit);
        edit
    }

    pub fn content<S: Into<Cow<'static, str>>>(&mut self, content: S) {
        self.content = Some(Some(content.into()));
    }

    pub fn clear_content(&mut self) {
        self.content = Some(None);
    }

    pub fn embed<F: FnOnce(&mut RichEmbed)>(&mut self, f: F) {
        let embed = self.embed.as_mut()
            .and_then(|opt| opt.take())
            .unwrap_or_default();
        self.embed = Some(Some(RichEmbed::build_with(embed, f)));
    }

    pub fn clear_embed(&mut self) {
        self.embed = Some(None);
    }
}

pub(in super) trait MessageWithFiles: Serialize {
    /// yeet the files out of `self`
    fn take_files(&mut self) -> HashMap<String, PathBuf>;

    /// true if content, embeds, etc are present
    fn has_other_content(&self) -> bool;
}

impl DiscordClient {
    pub(in super) async fn send_message_with_files<M: MessageWithFiles>(
        &self,
        route: Route,
        mut message: M,
    ) -> ClientResult<Message> {
        let files = message.take_files();
        if files.is_empty() {
            self.post(route, message).await
        } else {
            let files: ClientResult<Vec<(String, Vec<u8>)>> = files.into_iter()
                .map(|(name, file)| std::fs::read(file)
                    .map(|contents| (name, contents))
                    .map_err(ClientError::Io)
                )
                .collect();
            let files = files?;
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
    fn take_files(&mut self) -> HashMap<String, PathBuf, RandomState> {
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
    fn take_files(&mut self) -> HashMap<String, PathBuf, RandomState> {
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