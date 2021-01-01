use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::http::{ClientResult, DiscordClient};
use crate::http::channel::RichEmbed;
use crate::http::model::{AllowedMentions, ApplicationCommand, ApplicationId, Command, CommandId, GuildId, InteractionId, InteractionMessage, InteractionResponse, Message, MessageId, TopLevelOption};
use crate::http::routes::Route::*;

pub use super::model::interaction::message;

impl DiscordClient {
    /// Fetch all of the global commands for your application.
    pub async fn get_global_commands(&self, application: ApplicationId) -> ClientResult<Vec<ApplicationCommand>> {
        self.get(GetGlobalCommands(application)).await
    }

    /// Create a new global command. New global commands will be available in all guilds after 1 hour.
    ///
    /// Creating a command with the same name as an existing command for your application will
    /// overwrite the old command.
    pub async fn create_global_command(
        &self,
        application: ApplicationId,
        command: Command,
    ) -> ClientResult<ApplicationCommand> {
        self.post(CreateGlobalCommand(application), command).await
    }

    /// Edit a global command. Updates will be available in all guilds after 1 hour.
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
    pub async fn delete_global_command(
        &self,
        application: ApplicationId,
        id: CommandId,
    ) -> ClientResult<()> {
        self.delete(DeleteGlobalCommand(application, id)).await
    }

    /// Fetch all of the guild commands for your application for a specific guild.
    pub async fn get_guild_commands(&self, application: ApplicationId, guild: GuildId) -> ClientResult<Vec<ApplicationCommand>> {
        self.get(GetGuildCommands(application, guild)).await
    }

    /// Create a new guild command. New guild commands will be available in the guild immediately.
    ///
    /// Creating a command with the same name as an existing command for your application will
    /// overwrite the old command.
    pub async fn create_guild_command(
        &self,
        application: ApplicationId,
        guild: GuildId,
        command: Command,
    ) -> ClientResult<ApplicationCommand> {
        self.post(CreateGuildCommand(application, guild), command).await
    }

    /// Edit a guild command. Updates for guild commands will be available immediately.
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
    pub async fn delete_guild_command(
        &self,
        application: ApplicationId,
        guild: GuildId,
        id: CommandId,
    ) -> ClientResult<()> {
        self.delete(DeleteGuildCommand(application, guild, id)).await
    }

    /// Create a response to an Interaction from the gateway.
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
    pub async fn delete_interaction_response(
        &self,
        application: ApplicationId,
        token: &str,
    ) -> ClientResult<()> {
        self.delete(DeleteInteractionResponse(application, token.into())).await
    }

    // todo link
    /// Create a followup message for an Interaction. Functions the same as Execute Webhook
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
    pub content: String,
    /// override the default username of the webhook
    pub username: Option<String>,
    /// override the default avatar of the webhook
    pub avatar_url: Option<String>,
    /// true if this is a TTS message
    pub tts: bool,
    /// the contents of the file being sent
    pub files: HashMap<String, PathBuf>,
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

    pub fn content<S: ToString>(&mut self, content: S) -> &mut Self {
        self.content = content.to_string();
        self
    }

    pub fn username<S: ToString>(&mut self, username: S) -> &mut Self {
        self.username = Some(username.to_string());
        self
    }

    pub fn avatar_url<S: ToString>(&mut self, avatar_url: S) -> &mut Self {
        self.avatar_url = Some(avatar_url.to_string());
        self
    }

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
    /// panics if this message already has 10 or more embeds
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