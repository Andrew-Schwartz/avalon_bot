use Route::*;

use crate::cache::Cache;
use crate::model::channel::Channel;
use crate::model::emoji::Emoji;
use crate::model::ids::*;

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
//    // although not used in the actual url, names are included to help debugging
    GetGlobalCommands(ApplicationId),
    CreateGlobalCommand(ApplicationId),
    EditGlobalCommand(ApplicationId, CommandId),
    DeleteGlobalCommand(ApplicationId, CommandId),
    BulkOverwriteGlobalCommands(ApplicationId),
    GetGuildCommands(ApplicationId, GuildId),
    CreateGuildCommand(ApplicationId, GuildId),
    EditGuildCommand(ApplicationId, GuildId, CommandId),
    DeleteGuildCommand(ApplicationId, GuildId, CommandId),
    BulkOverwriteGuildCommands(ApplicationId, GuildId),
    CreateInteractionResponse(InteractionId, String),
    EditInteractionResponse(ApplicationId, String),
    DeleteInteractionResponse(ApplicationId, String),
    CreateFollowupMessage(ApplicationId, String),
    EditFollowupMessage(ApplicationId, String, MessageId),
    DeleteFollowupMessage(ApplicationId, String, MessageId),
    GetGuildApplicationCommandPermissions(ApplicationId, GuildId),
    GetApplicationCommandPermissions(ApplicationId, GuildId, CommandId),
    EditApplicationCommandPermissions(ApplicationId, GuildId, CommandId),
    BatchEditApplicationCommandPermissions(ApplicationId, GuildId),

    // users
    GetUser(UserId),
    CreateDm,

    // guilds
    GetGuildMember(GuildId, UserId),
    AddGuildMemberRole(GuildId, UserId, RoleId),
    RemoveGuildMemberRole(GuildId, UserId, RoleId),
    GetGuildRoles(GuildId),
    CreateGuildRole(GuildId),
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
            BulkOverwriteGlobalCommands(a) => api!("/applications/{}/commands", a),
            GetGuildCommands(a, g) => api!("/applications/{}/guilds/{}/commands", a, g),
            CreateGuildCommand(a, g) => api!("/applications/{}/guilds/{}/commands", a, g),
            EditGuildCommand(a, g, c) => api!("/applications/{}/guilds/{}/commands/{}", a, g, c),
            DeleteGuildCommand(a, g, c) => api!("/applications/{}/guilds/{}/commands/{}", a, g, c),
            BulkOverwriteGuildCommands(a, g) => api!("/applications/{}/guilds/{}/commands", a, g),
            CreateInteractionResponse(i, t) => api!("/interactions/{}/{}/callback", i, t),
            EditInteractionResponse(a, t) => api!("/webhooks/{}/{}/messages/@original", a, t),
            DeleteInteractionResponse(a, t) => api!("/webhooks/{}/{}/messages/@original", a, t),
            CreateFollowupMessage(a, t) => api!("/webhooks/{}/{}", a, t),
            EditFollowupMessage(a, t, m) => api!("/webhooks/{}/{}/messages/{}", a, t, m),
            DeleteFollowupMessage(a, t, m) => api!("/webhooks/{}/{}/messages/{}", a, t, m),
            GetGuildApplicationCommandPermissions(a, g) => api!("/applications/{}/guilds/{}/commands/permissions", a, g),
            GetApplicationCommandPermissions(a, g, c) => api!("/applications/{}/guilds/{}/commands/{}/permissions", a, g, c),
            EditApplicationCommandPermissions(a, g, c) => api!("/applications/{}/guilds/{}/commands/{}/permissions", a, g, c),
            BatchEditApplicationCommandPermissions(a, g) => api!("/applications/{}/guilds/{}/commands/permissions", a, g),

            GetUser(u) => api!("/users/{}", u),
            CreateDm => api!("/users/@me/channels"),

            GetGuildMember(g, u) => api!("/guilds/{}/members/{}", g, u),
            AddGuildMemberRole(g, u, r) => api!("/guild/{}/members/{}/roles/{}", g, u, r),
            RemoveGuildMemberRole(g, u, r) => api!("/guild/{}/members/{}/roles/{}", g, u, r),
            GetGuildRoles(g) => api!("/guilds/{}/roles", g),
            CreateGuildRole(g) => api!("/guilds/{}/roles", g),
        }
    }

    #[allow(clippy::too_many_lines)]
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
            cache.user(user).await.map_or_else(|| user.to_string(), |u| u.username)
        };
        let command = |command: CommandId| async move {
            cache.command(command).await.map_or_else(|| command.to_string(), |c| c.name)
        };
        let guild = |guild: GuildId| async move {
            cache.guild(guild).await
                .and_then(|g| g.name)
                .unwrap_or_else(|| guild.to_string())
        };
        let role = |guild: GuildId, role: RoleId| async move {
            cache.guild(guild).await
                .and_then(|mut g| g.roles
                    // cache.whatever() clones it, so we're free to yoink the role
                    .remove(role)
                    .map(|r| r.name))
                .unwrap_or_else(|| role.to_string())
        };

        #[allow(clippy::useless_format)]
        match self {
            GetGateway => String::from("GetGateway"),
            ApplicationInfo => String::from("GetApplicationInfo"),
            &GetChannel(c) => format!("GetChannel({})", channel(c).await),
            &TriggerTyping(c) => format!("TriggerTyping({})", channel(c).await),
            &GetPinnedMessages(c) => format!("GetPinnedMessages({})", channel(c).await),
            &PinMessage(c, m) => format!("PinMessage({}/{})", channel(c).await, m),
            &UnpinMessage(c, m) => format!("UnpinMessage({}/{})", channel(c).await, m),
            &GetMessage(c, m) => format!("GetMessage({}/{})", channel(c).await, m),
            &PostMessage(c) => format!("PostMessage({})", channel(c).await),
            &EditMessage(c, m) => format!("EditMessage({}/{})", channel(c).await, m),
            &DeleteMessage(c, m) => format!("DeleteMessage({}/{})", channel(c).await, m),
            CreateReaction(c, m, e) => format!(
                "CreateReaction({}/{}/{})",
                channel(*c).await, m, e.as_reaction()
            ),
            DeleteOwnReaction(c, m, e) => format!(
                "DeleteOwnReaction({}/{}/{})",
                channel(*c).await, m, e.as_reaction()
            ),
            DeleteUserReaction(c, m, e, u) => format!(
                "DeleteUserReaction({}/{}/{}/{})",
                channel(*c).await, m, e.as_reaction(), user(*u).await
            ),
            // don't display ApplicationId because it'll always be the same
            GetGlobalCommands(_) => format!("GetGlobalCommands"),
            CreateGlobalCommand(_) => format!("CreateGlobalCommand"),
            &EditGlobalCommand(_, c) => format!("EditGlobalCommand({})", command(c).await),
            &DeleteGlobalCommand(_, c) => format!("DeleteGlobalCommand({})", command(c).await),
            BulkOverwriteGlobalCommands(_) => format!("BulkOverwriteGlobalCommands"),
            &GetGuildCommands(_, g) => format!("GetGuildCommands({})", guild(g).await),
            &CreateGuildCommand(_, g) => format!("CreateGuildCommand({})", guild(g).await),
            &EditGuildCommand(_, g, c) => format!(
                "EditGuildCommand({}/{})",
                guild(g).await, command(c).await
            ),
            &DeleteGuildCommand(_, g, c) => format!(
                "DeleteGuildCommand({}/{})",
                guild(g).await, command(c).await
            ),
            &BulkOverwriteGuildCommands(_, g) => format!(
                "BulkOverwriteGuildCommands({})",
                guild(g).await
            ),
            CreateInteractionResponse(_, _) => format!("CreateInteractionResponse"),
            EditInteractionResponse(_, _) => format!("EditInteractionResponse"),
            DeleteInteractionResponse(_, _) => format!("DeleteInteractionResponse"),
            CreateFollowupMessage(_, _) => format!("CreateFollowupMessage"),
            EditFollowupMessage(_, _, m) => format!("EditFollowupMessage({})", m),
            DeleteFollowupMessage(_, _, m) => format!("DeleteFollowupMessage({})", m),
            &GetGuildApplicationCommandPermissions(_, g) => format!(
                "GetGuildApplicationCommandPermissions({})",
                guild(g).await,
            ),
            &GetApplicationCommandPermissions(_, g, c) => format!(
                "GetApplicationCommandPermissions({}, {})",
                guild(g).await, command(c).await
            ),
            &EditApplicationCommandPermissions(_, g, c) => format!(
                "EditApplicationCommandPermissions({}, {})",
                guild(g).await, command(c).await
            ),
            &BatchEditApplicationCommandPermissions(_, g) => format!(
                "BatchEditApplicationCommandPermissions({})",
                guild(g).await,
            ),
            &GetUser(u) => format!("GetUser({})", user(u).await),
            CreateDm => format!("CreateDm"),
            &GetGuildMember(g, u) => format!("GetGuildMember({}, {})", g, u),
            &AddGuildMemberRole(g, u, r) => format!(
                "AddGuildMemberRole({}, {}, {})",
                guild(g).await, user(u).await, role(g, r).await
            ),
            &RemoveGuildMemberRole(g, u, r) => format!(
                "RemoveGuildMemberRole({}, {}, {})",
                guild(g).await, user(u).await, role(g, r).await
            ),
            &GetGuildRoles(g) => format!("GetGuildRoles({})", guild(g).await),
            &CreateGuildRole(g) => format!("CreateGuildRole({})", guild(g).await),
        }
    }
}