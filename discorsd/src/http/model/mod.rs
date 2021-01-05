use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use thiserror::Error;

pub use channel::*;
pub use emoji::*;
pub use guild::*;
pub use ids::*;
pub use interaction::*;
pub use message::*;
pub use permissions::*;
pub use user::*;
pub use voice::*;

#[macro_use]
pub mod ids;
pub mod guild;
pub mod voice;
pub mod permissions;
pub mod emoji;
pub mod user;
pub mod channel;
pub mod message;
pub mod interaction;

/// Information returned from the `/gateway/bot` endpoint, as in [DiscordClient::gateway]
#[derive(Deserialize, Debug)]
pub struct BotGateway {
    /// The WSS URL that can be used for connecting to the gateway
    pub url: String,
    /// The recommended number of shards to use when connecting
    shards: u64,
    // /// Information on the current session start limit
    // session_start_limit: session_start_limit,
}

#[derive(Deserialize, Debug, Error)]
pub struct DiscordError {
    pub code: DiscordErrorType,
    pub message: String,
    pub errors: serde_json::Value,
}

impl Display for DiscordError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// https://discord.com/developers/docs/topics/opcodes-and-status-codes#json-json-error-codes
#[derive(Deserialize_repr, Debug)]
#[repr(u32)]
pub enum DiscordErrorType {
    /// General error (such as a malformed request body, amongst other things)
    General = 0,
    /// Unknown account
    UnknownAccount = 10001,
    /// Unknown application
    UnknownApplication = 10002,
    /// Unknown channel
    UnknownChannel = 10003,
    /// Unknown guild
    UnknownGuild = 10004,
    /// Unknown integration
    UnknownIntegration = 10005,
    /// Unknown invite
    UnknownInvite = 10006,
    /// Unknown member
    UnknownMember = 10007,
    /// Unknown message
    UnknownMessage = 10008,
    /// Unknown permission overwrite
    UnknownPermissionOverwrite = 10009,
    /// Unknown provider
    UnknownProvider = 10010,
    /// Unknown role
    UnknownRole = 10011,
    /// Unknown token
    UnknownToken = 10012,
    /// Unknown user
    UnknownUser = 10013,
    /// Unknown emoji
    UnknownEmoji = 10014,
    /// Unknown webhook
    UnknownWebhook = 10015,
    /// Unknown ban
    UnknownBan = 10026,
    /// Unknown SKU
    UnknownSKU = 10027,
    /// Unknown Store Listing
    UnknownStoreListing = 10028,
    /// Unknown entitlement
    UnknownEntitlement = 10029,
    /// Unknown build
    UnknownBuild = 10030,
    /// Unknown lobby
    UnknownLobby = 10031,
    /// Unknown branch
    UnknownBranch = 10032,
    /// Unknown redistributable
    UnknownRedistributable = 10036,
    /// Unknown guild template
    UnknownGuildTemplate = 10057,
    /// Bots cannot use this endpoint
    BotForbidden = 20001,
    /// Only bots can use this endpoint
    OnlyBots = 20002,
    /// This message cannot be edited due to announcement rate limits
    AnnouncementEditRateLimit = 20022,
    /// The channel you are writing has hit the write rate limit
    ChannelWriteRateLimit = 20028,
    /// Maximum number of guilds reached (100)
    MaxGuilds = 30001,
    /// Maximum number of friends reached (1000)
    MaxFriends = 30002,
    /// Maximum number of pins reached for the channel (50)
    MaxPins = 30003,
    /// Maximum number of guild roles reached (250)
    MaxGuildRoles = 30005,
    /// Maximum number of webhooks reached (10)
    MaxWebhooks = 30007,
    /// Maximum number of reactions reached (20)
    MaxReactions = 30010,
    /// Maximum number of guild channels reached (500)
    MaxGuildChannels = 30013,
    /// Maximum number of attachments in a message reached (10)
    MaxAttachments = 30015,
    /// Maximum number of invites reached (1000)
    MaxInvites = 30016,
    /// Guild already has a template
    GuildTemplateRepeat = 30031,
    /// Unauthorized. Provide a valid token and try again
    Unauthorized = 40001,
    /// You need to verify your account in order to perform this action
    Unverified = 40002,
    /// Request entity too large. Try sending something smaller in size
    RequestTooLarge = 40005,
    /// This feature has been temporarily disabled server-side
    FeatureTempDisabled = 40006,
    /// The user is banned from this guild
    UserBanned = 40007,
    /// This message has already been crossposted
    AlreadyCrossposted = 40033,
    /// Missing access
    MissingAccess = 50001,
    /// Invalid account type
    InvalidAccountType = 50002,
    /// Cannot execute action on a DM channel
    CannotExecuteActionDm = 50003,
    /// Guild widget disabled
    GuildWidgetDisabled = 50004,
    /// Cannot edit a message authored by another user
    EditOtherUserMessage = 50005,
    /// Cannot send an empty message
    EmptyMessage = 50006,
    /// Cannot send messages to this user
    CannotSendToUser = 50007,
    /// Cannot send messages in a voice channel
    CannotSendInVoiceChannel = 50008,
    /// Channel verification level is too high for you to gain access
    NotChannelVerified = 50009,
    /// OAuth2 application does not have a bot
    OAuth2NoBot = 50010,
    /// OAuth2 application limit reached
    OAuth2ApplicationLimit = 50011,
    /// Invalid OAuth2 state
    InvalidOAuth2State = 50012,
    /// You lack permissions to perform that action
    Permissions = 50013,
    /// Invalid authentication token provided
    InvalidToken = 50014,
    /// Note was too long
    NoteTooLong = 50015,
    /// Provided too few or too many messages to delete. Must provide at least 2 and fewer than 100 messages to delete
    MessageDeleteNumber = 50016,
    /// A message can only be pinned to the channel it was sent in
    MessagePinInWrongChannel = 50019,
    /// Invite code was either invalid or taken
    InviteCode = 50020,
    /// Cannot execute action on a system message
    CannotExecuteActionSystemMessage = 50021,
    /// Cannot execute action on this channel type
    CannotExecuteActionChannelType = 50024,
    /// Invalid OAuth2 access token provided
    InvalidOAuth2Token = 50025,
    /// "Invalid Recipient(s)"
    InvalidRecipients = 50033,
    /// A message provided was too old to bulk delete
    BulkDeleteTooOld = 50034,
    /// Invalid form body (returned for both application/json and multipart/form-data bodies), or invalid Content-Type provided
    InvalidFormBodyOrContentType = 50035,
    /// An invite was accepted to a guild the application's bot is not in
    InviteAccepted = 50036,
    /// Invalid API version provided
    InvalidAPIVersion = 50041,
    /// Cannot delete a channel required for Community guilds
    DeleteRequiredCommunityGuildChannel = 50074,
    /// Invalid sticker sent
    InvalidSticker = 50081,
    /// Reaction was blocked
    ReactionBlocked = 90001,
    /// API resource is currently overloaded. Try again a little later
    ApiResourceOverloaded = 130000,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Application {
    /// the id of the app
    pub id: ApplicationId,
    /// the name of the app
    pub name: String,
    /// the icon hash of the app
    pub icon: Option<String>,
    /// the description of the app
    pub description: String,
    /// an array of rpc origin urls, if rpc is enabled
    #[serde(default)]
    pub rpc_origins: Vec<String>,
    /// when false only app owner can join the app's bot to guilds
    pub bot_public: bool,
    /// when true the app's bot will only join upon completion of the full oauth2 code grant flow
    pub bot_require_code_grant: bool,
    /// partial user object containing info on the owner of the application
    // todo is just partial?
    pub owner: User,
    /// if this application is a game sold on Discord, this field will be the summary field for the store page of its primary sku
    pub summary: String,
    /// the base64 encoded key for the GameSDK's GetTicket
    pub verify_key: String,
    /// if the application belongs to a team, this will be a list of the members of that team
    pub team: Option<Team>,
    /// if this application is a game sold on Discord, this field will be the guild to which it has been linked
    pub guild_id: Option<Guild>,
    /// if this application is a game sold on Discord, this field will be the id of the "Game SKU" that is created, if exists
    pub primary_sku_id: Option<SkuId>,
    /// if this application is a game sold on Discord, this field will be the URL slug that links to the store page
    pub slug: Option<String>,
    /// if this application is a game sold on Discord, this field will be the hash of the image on store embeds
    pub cover_image: Option<String>,
    /// the application's public flags
    pub flags: Option<u32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Team {
    /// a hash of the image of the team's icon
    pub icon: Option<String>,
    /// the unique id of the team
    pub id: TeamId,
    /// the members of the team
    pub members: Vec<TeamMember>,
    /// the user id of the current team owner
    pub owner_user_id: UserId,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TeamMember {
    /// the user's membership state on the team
    pub membership_state: MembershipState,
    /// will always be ["*"]
    pub permissions: (String, ),
    /// the id of the parent team of which they are a member
    pub team_id: TeamId,
    /// the avatar, discriminator, id, and username of the user
    pub user: User,
}

#[derive(Deserialize_repr, Serialize_repr, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum MembershipState {
    Invited = 1,
    Accepted = 2,
}

pub trait ImageFormat {
    const EXTENSION: &'static str;

    const ANIMATED: bool;
}

pub trait StillImage: ImageFormat {}

pub struct Jpeg;

impl ImageFormat for Jpeg {
    const EXTENSION: &'static str = "jpeg";
    const ANIMATED: bool = false;
}

impl StillImage for Jpeg {}

pub struct Png;

impl ImageFormat for Png {
    const EXTENSION: &'static str = "png";
    const ANIMATED: bool = false;
}

impl StillImage for Png {}

pub struct WebP;

impl ImageFormat for WebP {
    const EXTENSION: &'static str = "webp";
    const ANIMATED: bool = false;
}

impl StillImage for WebP {}

pub struct Gif;

impl ImageFormat for Gif {
    const EXTENSION: &'static str = "gif";
    const ANIMATED: bool = true;
}
