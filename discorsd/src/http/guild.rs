//! Discord API requests involving guilds.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::BotState;
use crate::commands::{ApplicationCommand, CommandPermissions, GuildCommandPermissions};
use crate::http::{ClientResult, DiscordClient};
use crate::http::routes::Route::*;
use crate::model::guild::GuildMember;
use crate::model::ids::{CommandId, GuildId, Id, RoleId, UserId};
use crate::model::message::Color;
use crate::model::permissions::{Permissions, Role};

impl DiscordClient {
    /// Returns a [GuildMember](GuildMember) for the specified user.
    pub async fn get_guild_member(&self, guild: GuildId, user: UserId) -> ClientResult<GuildMember> {
        self.get(GetGuildMember(guild, user)).await
    }

    // todo link docs
    /// Adds a role to a guild member.
    /// Requires the [Permissions::MANAGE_ROLES](Permissions::MANAGE_ROLES) permission
    ///
    /// Fires a Guild Member Update Gateway event.
    pub async fn add_guild_member_role(
        &self,
        guild: GuildId,
        user: UserId,
        role: RoleId,
    ) -> ClientResult<()> {
        self.put_unit(AddGuildMemberRole(guild, user, role), Some("")).await
    }

    /// Removes a role to a guild member.
    /// Requires the [Permissions::MANAGE_ROLES](Permissions::MANAGE_ROLES) permission
    ///
    /// Fires a Guild Member Update Gateway event.
    pub async fn remove_guild_member_role(
        &self,
        guild: GuildId,
        user: UserId,
        role: RoleId,
    ) -> ClientResult<()> {
        self.delete(RemoveGuildMemberRole(guild, user, role)).await
    }

    /// Returns a list of role objects for the guild
    pub async fn get_guild_roles(&self, guild: GuildId) -> ClientResult<Vec<Role>> {
        self.get(GetGuildRoles(guild)).await
    }

    /// Create a new role for the guild. Requires the [Permissions::MANAGE_ROLES](Permissions::MANAGE_ROLES) permission.
    ///
    /// Fires a Guild Role Create Gateway event.
    pub async fn create_guild_role(&self, guild: GuildId, role: CreateRole) -> ClientResult<Role> {
        self.post(CreateGuildRole(guild), role).await
    }
}

// todo more of these (only getters, since other ones trigger events)
impl<B: Send + Sync + 'static> BotState<B> {
    pub async fn cache_guild_member(&self, guild: GuildId, user: UserId) -> ClientResult<GuildMember> {
        let member = self.client.get_guild_member(guild, user).await?;
        let mut guard = self.cache.members.write().await;
        let members = guard.entry(user).or_default();
        members.insert(guild, member.clone());
        Ok(member)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct CreateRole {
    /// name of the role
	///
	/// default: `new role`
    pub name: Option<String>,
    /// bitwise value of the enabled/disabled permissions
	///
	/// default: `@everyone` permissions in guild
    pub permissions: Option<Permissions>,
    /// RGB color value
	///
	/// default: `0` (does not change name color)
    pub color: Option<Color>,
    /// whether the role should be displayed separately in the sidebar
	///
	/// default: `false`
    pub hoist: bool,
    /// whether the role should be mentionable
	///
	/// default: `false`
    pub mentionable: bool,
}

impl CreateRole {
    pub fn new<N: Into<String>>(name: N) -> Self {
        Self { name: Some(name.into()), ..Self::default() }
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

// todo impl a similar trait on guild
#[async_trait]
pub trait CommandPermsExt {
    async fn edit_permissions<B, State>(
        // todo should this really take mut self?
        &self,
        state: State,
        guild: GuildId,
        permissions: Vec<CommandPermissions>,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send;

    async fn default_permissions<B, State>(
        &self,
        state: State,
        guild: GuildId,
        usable: bool,
    ) -> ClientResult<ApplicationCommand>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send;

    async fn allow_roles<B, State, Roles, R>(
        &self,
        state: State,
        guild: GuildId,
        roles: Roles,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
              Roles: IntoIterator<Item=R> + Send,
              R: Id<Id=RoleId>,
    {
        let permissions = roles.into_iter()
            .map(|id| id.id())
            .map(CommandPermissions::allow_role)
            .collect();
        self.edit_permissions(state, guild, permissions).await
    }

    async fn disallow_roles<B, State, Roles, R>(
        &self,
        state: State,
        guild: GuildId,
        roles: Roles,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
              Roles: IntoIterator<Item=R> + Send,
              R: Id<Id=RoleId>,
    {
        let permissions = roles.into_iter()
            .map(|id| id.id())
            .map(CommandPermissions::disallow_role)
            .collect();
        self.edit_permissions(state, guild, permissions).await
    }

    async fn allow_users<B, State, Users, U>(
        &self,
        state: State,
        guild: GuildId,
        users: Users,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
              Users: IntoIterator<Item=U> + Send,
              U: Id<Id=UserId>,
    {
        let permissions = users.into_iter()
            .map(|id| id.id())
            .map(CommandPermissions::allow_user)
            .collect();
        self.edit_permissions(state, guild, permissions).await
    }

    async fn disallow_users<B, State, Users, U>(
        &self,
        state: State,
        guild: GuildId,
        users: Users,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
              Users: IntoIterator<Item=U> + Send,
              U: Id<Id=UserId>
    {
        let permissions = users.into_iter()
            .map(|id| id.id())
            .map(CommandPermissions::disallow_user)
            .collect();
        self.edit_permissions(state, guild, permissions).await
    }
}

// todo maybe it makes more sense to specifically impl this on `&CommandId`, `Command`, and `&mut Command`??
//  and ApplicationCommand
#[async_trait]
impl<C: Id<Id=CommandId> + Send + Sync> CommandPermsExt for C {
    async fn edit_permissions<B, State>(
        &self,
        state: State,
        guild: GuildId,
        permissions: Vec<CommandPermissions>,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
    {
        let state = state.as_ref();
        let id = self.id();
        state.client.edit_application_command_permissions(
            state.application_id(),
            guild,
            id,
            permissions,
        ).await
    }

    async fn default_permissions<B, State>(
        &self,
        state: State,
        guild: GuildId,
        usable: bool,
    ) -> ClientResult<ApplicationCommand>
        where
            B: Send + Sync + 'static,
            State: AsRef<BotState<B>> + Send,
    {
        let state = state.as_ref();
        state.client.edit_guild_command(
            state.application_id(),
            guild,
            self.id(),
            None,
            None,
            None,
            Some(usable),
        ).await
    }
}

// todo duplicate ^ here?
#[async_trait]
pub trait GuildCommandPermsExt {
    /// This endpoint will overwrite ALL existing permissions for all commands in a guild, even
    /// those not in the `permissions` list.
    async fn batch_edit_permissions<B, State>(
        &self,
        state: State,
        permissions: Vec<GuildCommandPermissions>,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send;
}

#[async_trait]
impl<G: Id<Id=GuildId> + Send + Sync> GuildCommandPermsExt for G {
    async fn batch_edit_permissions<B, State>(
        &self,
        state: State,
        permissions: Vec<GuildCommandPermissions>,
    ) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
    {
        let state = state.as_ref();
        let id = self.id();
        state.client.batch_edit_application_command_permissions(
            state.application_id(),
            id,
            permissions,
        ).await
    }
}