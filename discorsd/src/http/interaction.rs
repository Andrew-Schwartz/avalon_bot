use std::collections::HashSet;

use serde::Serialize;

use crate::model::ids::*;
use crate::http::{ClientResult, DiscordClient};
use crate::http::channel::{RichEmbed, MessageAttachment};
use crate::http::routes::Route::*;

pub use crate::model::interaction::message;
use std::borrow::Cow;
use crate::model::message::{AllowedMentions, Message};
use crate::model::interaction::{ApplicationCommand, Command, TopLevelOption, InteractionResponse, InteractionMessage};

impl DiscordClient {
    /// Fetch all of the global commands for your application.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Vec<ApplicationCommand>`
    pub async fn get_global_commands(&self, application: ApplicationId) -> ClientResult<Vec<ApplicationCommand>> {
        self.get(GetGlobalCommands(application)).await
    }

    /// Create a new global command. New global commands will be available in all guilds after 1 hour.
    ///
    /// Creating a command with the same name as an existing command for your application will
    /// overwrite the old command.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `ApplicationCommand`
    pub async fn create_global_command(
        &self,
        application: ApplicationId,
        command: Command,
    ) -> ClientResult<ApplicationCommand> {
        self.post(CreateGlobalCommand(application), command).await
    }

    /// Edit a global command. Updates will be available in all guilds after 1 hour.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `ApplicationCommand`
    pub async fn edit_global_command<'a>(
        &self,
        application: ApplicationId,
        id: CommandId,
        new_name: Option<&'a str>,
        new_description: Option<&'a str>,
        new_options: Option<TopLevelOption>,
    ) -> ClientResult<ApplicationCommand> {
        self.patch(
            EditGlobalCommand(application, id),
            Edit {
                name: new_name,
                description: new_description,
                options: new_options
            }
        ).await
    }

    /// Deletes a global command.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_global_command(
        &self,
        application: ApplicationId,
        id: CommandId,
    ) -> ClientResult<()> {
        self.delete(DeleteGlobalCommand(application, id)).await
    }

    /// Fetch all of the guild commands for your application for a specific guild.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Vec<ApplicationCommand>`
    pub async fn get_guild_commands(&self, application: ApplicationId, guild: GuildId) -> ClientResult<Vec<ApplicationCommand>> {
        self.get(GetGuildCommands(application, guild)).await
    }

    /// Create a new guild command. New guild commands will be available in the guild immediately.
    ///
    /// Creating a command with the same name as an existing command for your application will
    /// overwrite the old command.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `ApplicationCommand`
    pub async fn create_guild_command(
        &self,
        application: ApplicationId,
        guild: GuildId,
        command: Command,
    ) -> ClientResult<ApplicationCommand> {
        self.post(CreateGuildCommand(application, guild), command).await
    }

    /// Edit a guild command. Updates for guild commands will be available immediately.
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `ApplicationCommand`
    pub async fn edit_guild_command<'a>(
        &self,
        application: ApplicationId,
        guild: GuildId,
        id: CommandId,
        new_name: Option<&'a str>,
        new_description: Option<&'a str>,
        new_options: Option<TopLevelOption>,
    ) -> ClientResult<ApplicationCommand> {
        self.patch(
            EditGuildCommand(application, guild, id),
            Edit {
                name: new_name,
                description: new_description,
                options: new_options,
            },
        ).await
    }

    /// Delete a guild command.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_guild_command(
        &self,
        application: ApplicationId,
        guild: GuildId,
        id: CommandId,
    ) -> ClientResult<()> {
        self.delete(DeleteGuildCommand(application, guild, id)).await
    }

    /// Create a response to an Interaction from the gateway.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn create_interaction_response(
        &self,
        interaction: InteractionId,
        token: &str,
        response: InteractionResponse,
    ) -> ClientResult<InteractionResponse> {
        self.post_unit(
            CreateInteractionResponse(interaction, token.into()),
            &response,
        ).await.map(|_| response)
    }

    // todo link to EditWebhookMessage?
    /// Edits the initial Interaction response. Functions the same as Edit Webhook Message.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn edit_interaction_response(
        &self,
        application: ApplicationId,
        token: &str,
        message: InteractionMessage,
    ) -> ClientResult<InteractionMessage> {
        self.patch_unit(
            EditInteractionResponse(application, token.into()),
            &message,
        ).await.map(|_| message)
    }

    /// Deletes the initial Interaction response.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_interaction_response(
        &self,
        application: ApplicationId,
        token: &str,
    ) -> ClientResult<()> {
        self.delete(DeleteInteractionResponse(application, token.into())).await
    }

    // todo link
    /// Create a followup message for an Interaction. Functions the same as Execute Webhook
    ///
    /// # Errors
    ///
    /// If the http request fails, or fails to deserialize the response into a `Message`
    pub async fn create_followup_message(
        &self,
        application: ApplicationId,
        token: &str,
        message: WebhookMessage,
    ) -> ClientResult<Message> {
        self.send_message_with_files(CreateFollowupMessage(application, token.into()), message).await
    }

    // todo link
    /// Edits a followup message for an Interaction. Functions the same as Edit Webhook Message.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn edit_followup_message(
        &self,
        application: ApplicationId,
        token: &str,
        message: MessageId,
        edit: InteractionResponse,
    ) -> ClientResult<InteractionResponse> {
        self.patch_unit(
            EditFollowupMessage(application, token.into(), message),
            &edit,
        ).await.map(|_| edit)
    }

    /// Deletes a followup message for an Interaction.
    ///
    /// # Errors
    ///
    /// If the http request fails
    pub async fn delete_followup_message(
        &self,
        application: ApplicationId,
        token: &str,
        message: MessageId,
    ) -> ClientResult<()> {
        self.delete(DeleteFollowupMessage(application, token.into(), message)).await
    }
}

#[derive(Serialize)]
struct Edit<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<TopLevelOption>,
}

// todo make this not own the strings?
#[derive(Serialize, Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct WebhookMessage {
    /// the message contents (up to 2000 characters)
    pub content: Cow<'static, str>,
    /// override the default username of the webhook
    pub username: Option<Cow<'static, str>>,
    /// override the default avatar of the webhook
    pub avatar_url: Option<Cow<'static, str>>,
    /// true if this is a TTS message
    pub tts: bool,
    /// the contents of the file being sent
    #[serde(skip)]
    pub files: HashSet<MessageAttachment>,
    /// embedded rich content, up to 10
    pub embeds: Vec<RichEmbed>,
    /// allowed mentions for the message
    pub allowed_mentions: Option<AllowedMentions>,
}

pub fn webhook_message<F: FnOnce(&mut WebhookMessage)>(builder: F) -> WebhookMessage {
    WebhookMessage::build(builder)
}

impl WebhookMessage {
    pub fn build<F: FnOnce(&mut Self)>(builder: F) -> Self {
        let mut message = Self::default();
        builder(&mut message);
        message
    }

    pub fn content<S: Into<Cow<'static, str>>>(&mut self, content: S) -> &mut Self {
        self.content = content.into();
        self
    }

    pub fn username<S: Into<Cow<'static, str>>>(&mut self, username: S) -> &mut Self {
        self.username = Some(username.into());
        self
    }

    pub fn avatar_url<S: Into<Cow<'static, str>>>(&mut self, avatar_url: S) -> &mut Self {
        self.avatar_url = Some(avatar_url.into());
        self
    }

    // todo error, don't panic
    /// add [n](n) embed to the [WebhookMessage](WebhookMessage)
    ///
    /// # Panics
    ///
    /// Panics if adding [n](n) embeds will result in this [WebhookMessage](WebhookMessage) having
    /// more than 10 embeds.
    pub fn embeds<F: FnMut(usize, &mut RichEmbed)>(&mut self, n: usize, mut builder: F) -> &mut Self {
        if self.embeds.len() + n > 10 {
            panic!("can't send more than 10 embeds");
        } else {
            self.embeds.extend(
                (0..n).map(|i| RichEmbed::build(|e| builder(i, e)))
            );
            self
        }
    }

    /// add an embed to the [WebhookMessage](WebhookMessage)
    ///
    /// # Panics
    ///
    /// Panics if this message already has 10 or more embeds
    pub fn embed<F: FnOnce(&mut RichEmbed)>(&mut self, builder: F) -> &mut Self {
        if self.embeds.len() >= 10 {
            panic!("can't send more than 10 embeds");
        } else {
            self.embeds.push(RichEmbed::build(builder));
            self
        }
    }

    /// add an embed to the [WebhookMessage](WebhookMessage)
    ///
    /// # Errors
    ///
    /// Returns `Err(builder)` if this message already has 10 or more embeds
    pub fn try_embed<F: FnOnce(&mut RichEmbed)>(&mut self, builder: F) -> std::result::Result<&mut Self, F> {
        if self.embeds.len() >= 10 {
            Err(builder)
        } else {
            self.embeds.push(RichEmbed::build(builder));
            Ok(self)
        }
    }
}