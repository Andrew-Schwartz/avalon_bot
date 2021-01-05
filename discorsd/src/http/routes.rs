use Route::*;

use crate::cache::Cache;
use crate::http::model::{Channel, Emoji};
use crate::http::model::ids::*;

#[derive(Debug, Clone)]
pub enum Route {
    // general
    GetGateway,
    ApplicationInfo,

    // channels
    GetChannel(ChannelId),
    TriggerTyping(ChannelId),
    GetPinnedMessages(ChannelId),
    PinMessage(ChannelId, MessageId),
    UnpinMessage(ChannelId, MessageId),

    // messages
    GetMessage(ChannelId, MessageId),
    PostMessage(ChannelId),
    EditMessage(ChannelId, MessageId),
    DeleteMessage(ChannelId, MessageId),

    // reactions
    CreateReaction(ChannelId, MessageId, Emoji),
    DeleteOwnReaction(ChannelId, MessageId, Emoji),
    DeleteUserReaction(ChannelId, MessageId, Emoji, UserId),

    // commands
    GetGlobalCommands(ApplicationId),
    CreateGlobalCommand(ApplicationId),
    EditGlobalCommand(ApplicationId, CommandId),
    DeleteGlobalCommand(ApplicationId, CommandId),
    GetGuildCommands(ApplicationId, GuildId),
    CreateGuildCommand(ApplicationId, GuildId),
    EditGuildCommand(ApplicationId, GuildId, CommandId),
    DeleteGuildCommand(ApplicationId, GuildId, CommandId),
    CreateInteractionResponse(InteractionId, String),
    EditInteractionResponse(ApplicationId, String),
    DeleteInteractionResponse(ApplicationId, String),
    CreateFollowupMessage(ApplicationId, String),
    EditFollowupMessage(ApplicationId, String, MessageId),
    DeleteFollowupMessage(ApplicationId, String, MessageId),

    // users
    GetUser(UserId),
    CreateDm,
}

impl Route {
    pub fn url(&self) -> String {
        match self {
            GetGateway => api!("/gateway/bot"),
            ApplicationInfo => api!("/oauth2/applications/@me"),

            GetChannel(c) => api!("/channels/{}", c),
            TriggerTyping(c) => api!("/channels/{}/typing", c),
            GetPinnedMessages(c) => api!("/channels/{}/pins", c),
            PinMessage(c, m) => api!("/channels/{}/pins/{}", c, m),
            UnpinMessage(c, m) => api!("/channels/{}/pins/{}", c, m),

            GetMessage(c, m) => api!("/channels/{}/messages/{}", c, m),
            PostMessage(c) => api!("/channels/{}/messages", c),
            EditMessage(c, m) => api!("/channels/{}/messages/{}", c, m),
            DeleteMessage(c, m) => api!("/channels/{}/messages/{}", c, m),

            CreateReaction(c, m, e) => api!("/channels/{}/messages/{}/reactions/{}/@me", c, m, e.as_reaction()),
            DeleteOwnReaction(c, m, e) => api!("/channels/{}/messages/{}/reactions/{}/@me", c, m, e.as_reaction()),
            DeleteUserReaction(c, m, e, u) => api!("/channels/{}/messages/{}/reactions/{}/{}", c, m, e.as_reaction(), u),

            GetGlobalCommands(a) => api!("/applications/{}/commands", a),
            CreateGlobalCommand(a) => api!("/applications/{}/commands", a),
            EditGlobalCommand(a, c) => api!("/applications/{}/commands/{}", a, c),
            DeleteGlobalCommand(a, c) => api!("/applications/{}/commands/{}", a, c),
            GetGuildCommands(a, g) => api!("/applications/{}/guilds/{}/commands", a, g),
            CreateGuildCommand(a, g) => api!("/applications/{}/guilds/{}/commands", a, g),
            EditGuildCommand(a, g, c) => api!("/applications/{}/guilds/{}/commands/{}", a, g, c),
            DeleteGuildCommand(a, g, c) => api!("/applications/{}/guilds/{}/commands/{}", a, g, c),
            CreateInteractionResponse(i, t) => api!("/interactions/{}/{}/callback", i, t),
            EditInteractionResponse(a, t) => api!("/webhooks/{}/{}/messages/@original", a, t),
            DeleteInteractionResponse(a, t) => api!("/webhooks/{}/{}/messages/@original", a, t),
            CreateFollowupMessage(a, t) => api!("/webhooks/{}/{}", a, t),
            EditFollowupMessage(a, t, m) => api!("/webhooks/{}/{}/messages/{}", a, t, m),
            DeleteFollowupMessage(a, t, m) => api!("/webhooks/{}/{}/messages/{}", a, t, m),
            GetUser(u) => api!("/users/{}", u),
            CreateDm => api!("/users/@me/channels"),
        }
    }

    pub async fn debug_with_cache(&self, cache: &Cache) -> String {
        let channel = |channel: ChannelId| async move {
            let guild = if let Some(guild) = cache.channel(channel).await.and_then(|c| c.guild_id()) {
                cache.guild(guild).await
                    .and_then(|g| g.name)
                    .map(|n| n + "/")
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let channel = match cache.channel(channel).await {
                Some(Channel::Text(t)) => t.name,
                Some(Channel::Dm(dm)) => format!("DM: {}", dm.recipient.username),
                Some(Channel::Voice(v)) => v.name,
                Some(Channel::Category(c)) => c.name,
                Some(Channel::News(n)) => n.name,
                Some(Channel::Store(s)) => s.name,
                Some(Channel::GroupDm(_)) => unreachable!("bots can't be in group dms"),
                None => channel.to_string(),
            };
            format!("{}{}", guild, channel)
        };
        let user = |user: UserId| async move {
            cache.user(user).await
                .map(|u| u.username)
                .unwrap_or_else(|| user.to_string())
        };
        let command = |command: CommandId| async move {
            cache.command(command).await
                .map(|c| c.name)
                .unwrap_or_else(|| command.to_string())
        };
        let guild = |guild: GuildId| async move {
            cache.guild(guild).await
                .and_then(|g| g.name)
                .unwrap_or_else(|| guild.to_string())
        };

        match self {
            GetGateway => String::from("Get Gateway"),
            ApplicationInfo => String::from("Get Application Info"),
            &GetChannel(c) => format!("Get Channel `{}`", channel(c).await),
            &TriggerTyping(c) => format!("Trigger Typing `{}`", channel(c).await),
            &GetPinnedMessages(c) => format!("Get Pinned Messages `{}`", channel(c).await),
            &PinMessage(c, m) => format!("Pin Message `{}/{}`", channel(c).await, m),
            &UnpinMessage(c, m) => format!("Unpin Message `{}/{}`", channel(c).await, m),
            &GetMessage(c, m) => format!("Get Message `{}/{}`", channel(c).await, m),
            &PostMessage(c) => format!("Post Message `{}`", channel(c).await),
            &EditMessage(c, m) => format!("Edit Message `{}/{}`", channel(c).await, m),
            &DeleteMessage(c, m) => format!("Delete Message `{}/{}`", channel(c).await, m),
            CreateReaction(c, m, e) => format!(
                "Create Reaction `{}/{}/{}`",
                channel(*c).await, m, e.as_reaction()
            ),
            DeleteOwnReaction(c, m, e) => format!(
                "Delete Own Reaction `{}/{}/{}`",
                channel(*c).await, m, e.as_reaction()
            ),
            DeleteUserReaction(c, m, e, u) => format!(
                "Delete User Reaction `{}/{}/{}/{}`",
                channel(*c).await, m, e.as_reaction(), user(*u).await
            ),
            // don't display ApplicationId because it'll always be the same
            GetGlobalCommands(_) => format!("Get Global Commands"),
            CreateGlobalCommand(_) => format!("Create Global Command"),
            &EditGlobalCommand(_, c) => format!("Edit Global Command `{}`", command(c).await),
            &DeleteGlobalCommand(_, c) => format!("Delete Global Command `{}`", command(c).await),
            &GetGuildCommands(_, g) => format!("Get Guild Commands `{}`", guild(g).await),
            &CreateGuildCommand(_, g) => format!("Create Guild Command `{}`", guild(g).await),
            &EditGuildCommand(_, g, c) => format!(
                "Edit Guild Command `{}/{}`",
                guild(g).await, command(c).await
            ),
            &DeleteGuildCommand(_, g, c) => format!(
                "Delete Guild Command `{}/{}`",
                guild(g).await, command(c).await
            ),
            // todo do I want to display the token too?
            CreateInteractionResponse(_, _) => format!("Create Interaction Response"),
            EditInteractionResponse(_, _) => format!("Edit Interaction Response"),
            DeleteInteractionResponse(_, _) => format!("Delete Interaction Response"),
            CreateFollowupMessage(_, _) => format!("Create Followup Message"),
            EditFollowupMessage(_, _, m) => format!("Edit Followup Message `{}`", m),
            DeleteFollowupMessage(_, _, m) => format!("Delete Followup Message `{}`", m),
            &GetUser(u) => format!("Get User `{}`", user(u).await),
            CreateDm => format!("Create Dm"),
        }
    }
}