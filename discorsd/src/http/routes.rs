use crate::http::model::Emoji;
use crate::http::model::ids::*;
use Route::*;

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
}