use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use reqwest::header::HeaderMap;

use crate::http::routes::Route;
use crate::model::ids::*;

#[derive(Debug, Default)]
pub struct RateLimit {
    limit: Option<u32>,
    remaining: Option<u32>,
    reset: Option<Instant>,
}

impl RateLimit {
    fn limit(&self) -> Option<Duration> {
        match self.remaining {
            Some(remaining) if remaining == 0 => {
                let duration = self.reset.and_then(|reset| reset.checked_duration_since(Instant::now()))
                    // todo can be uoe(Duration::ZERO) when that's stabilized
                    .unwrap_or_else(|| Duration::new(0, 0));
                Some(duration)
            }
            _ => None,
        }
    }
}

impl fmt::Display for RateLimit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RateLimit")
            .field("limit", &self.limit)
            .field("remaining", &self.remaining)
            .field("reset", &self.reset.and_then(|reset| reset.checked_duration_since(Instant::now())))
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum BucketKey {
    GetGateway,
    ApplicationInfo,
    GetChannel(ChannelId),
    TriggerTyping(ChannelId),
    GetPinnedMessages(ChannelId),
    PinMessage(ChannelId),
    UnpinMessage(ChannelId),
    GetMessage(ChannelId),
    PostMessage(ChannelId),
    EditMessage(ChannelId),
    DeleteMessage(ChannelId),
    CreateReaction(ChannelId),
    DeleteOwnReaction(ChannelId),
    DeleteUserReaction(ChannelId),
    GetGlobalCommands,
    CreateGlobalCommand,
    EditGlobalCommand,
    DeleteGlobalCommand,
    BulkOverwriteGlobalCommands,
    GetGuildCommands(GuildId),
    CreateGuildCommand(GuildId),
    EditGuildCommand(GuildId),
    DeleteGuildCommand(GuildId),
    BulkOverwriteGuildCommands(GuildId),
    CreateInteractionResponse,
    EditInteractionResponse,
    DeleteInteractionResponse,
    CreateFollowupMessage,
    EditFollowupMessage,
    DeleteFollowupMessage,
    GetGuildApplicationCommandPermissions,
    GetApplicationCommandPermissions(GuildId),
    EditApplicationCommandPermissions(GuildId),
    BatchEditApplicationCommandPermissions,
    GetUser,
    CreateDm,
    GetGuildMember(GuildId),
    AddGuildMemberRole(GuildId),
    RemoveGuildMemberRole(GuildId),
    GetGuildRoles(GuildId),
    CreateGuildRole(GuildId),
}

impl From<&Route> for BucketKey {
    fn from(route: &Route) -> Self {
        match route {
            Route::GetGateway => Self::GetGateway,
            Route::ApplicationInfo => Self::ApplicationInfo,
            Route::GetChannel(c) => Self::GetChannel(*c),
            Route::TriggerTyping(c) => Self::TriggerTyping(*c),
            Route::GetPinnedMessages(c) => Self::GetPinnedMessages(*c),
            Route::PinMessage(c, _) => Self::PinMessage(*c),
            Route::UnpinMessage(c, _) => Self::UnpinMessage(*c),
            Route::GetMessage(c, _) => Self::GetMessage(*c),
            Route::PostMessage(c) => Self::PostMessage(*c),
            Route::EditMessage(c, _) => Self::EditMessage(*c),
            Route::DeleteMessage(c, _) => Self::DeleteMessage(*c),
            Route::CreateReaction(c, _, _) => Self::CreateReaction(*c),
            Route::DeleteOwnReaction(c, _, _) => Self::DeleteOwnReaction(*c),
            Route::DeleteUserReaction(c, _, _, _) => Self::DeleteUserReaction(*c),
            Route::GetGlobalCommands(_) => Self::GetGlobalCommands,
            Route::CreateGlobalCommand(_) => Self::CreateGlobalCommand,
            Route::EditGlobalCommand(_, _) => Self::EditGlobalCommand,
            Route::DeleteGlobalCommand(_, _) => Self::DeleteGlobalCommand,
            Route::BulkOverwriteGlobalCommands(_) => Self::BulkOverwriteGlobalCommands,
            Route::GetGuildCommands(_, g) => Self::GetGuildCommands(*g),
            Route::CreateGuildCommand(_, g) => Self::CreateGuildCommand(*g),
            Route::EditGuildCommand(_, g, _) => Self::EditGuildCommand(*g),
            Route::DeleteGuildCommand(_, g, _) => Self::DeleteGuildCommand(*g),
            Route::BulkOverwriteGuildCommands(_, g) => Self::BulkOverwriteGuildCommands(*g),
            Route::CreateInteractionResponse(_, _) => Self::CreateInteractionResponse,
            Route::EditInteractionResponse(_, _) => Self::EditInteractionResponse,
            Route::DeleteInteractionResponse(_, _) => Self::DeleteInteractionResponse,
            Route::CreateFollowupMessage(_, _) => Self::CreateFollowupMessage,
            Route::EditFollowupMessage(_, _, _) => Self::EditFollowupMessage,
            Route::DeleteFollowupMessage(_, _, _) => Self::DeleteFollowupMessage,
            Route::GetGuildApplicationCommandPermissions(_, _) => Self::GetGuildApplicationCommandPermissions,
            Route::GetApplicationCommandPermissions(_, g, _) => Self::GetApplicationCommandPermissions(*g),
            Route::EditApplicationCommandPermissions(_, g, _) => Self::EditApplicationCommandPermissions(*g),
            Route::BatchEditApplicationCommandPermissions(_, _) => Self::BatchEditApplicationCommandPermissions,
            Route::GetUser(_) => Self::GetUser,
            Route::CreateDm => Self::CreateDm,
            Route::GetGuildMember(g, _) => Self::GetGuildMember(*g),
            Route::AddGuildMemberRole(g, _, _) => Self::AddGuildMemberRole(*g),
            Route::RemoveGuildMemberRole(g, _, _) => Self::RemoveGuildMemberRole(*g),
            Route::GetGuildRoles(g) => Self::GetGuildRoles(*g),
            Route::CreateGuildRole(g) => Self::CreateGuildRole(*g),
        }
    }
}

#[derive(Debug, Default)]
pub struct RateLimiter(HashMap<BucketKey, RateLimit>);

impl RateLimiter {
    pub async fn rate_limit(&self, key: &BucketKey) {
        if let Some(rate_limit) = self.0.get(key) {
            if let Some(duration) = rate_limit.limit() {
                log::info!("{:?} ==> {}", key, rate_limit);
                tokio::time::delay_for(duration).await;
            }
        }
    }

    pub fn update(&mut self, key: BucketKey, headers: &HeaderMap) {
        let rate_limit = self.0.entry(key).or_default();
        if let Some(limit) = headers.get("X-RateLimit-Limit") {
            rate_limit.limit = Some(limit.to_str().unwrap().parse().unwrap());
        }
        if let Some(remaining) = headers.get("X-RateLimit-Remaining") {
            rate_limit.remaining = Some(remaining.to_str().unwrap().parse().unwrap());
        }
        if let Some(reset_after) = headers.get("X-RateLimit-Reset-After") {
            let secs = reset_after.to_str().unwrap().parse().unwrap();
            rate_limit.reset = Some(Instant::now() + Duration::from_secs_f64(secs));
        }
        // println!("{:?} ==> {}", key, rate_limit);
    }
}