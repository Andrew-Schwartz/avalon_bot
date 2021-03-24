use std::convert::TryFrom;
use std::marker::PhantomData;

use crate::BotState;
use crate::commands::{Interaction, SlashCommand};
use crate::errors::*;
use crate::http::{ClientError, DiscordClient};
use crate::model::{ids::*, interaction::*};
use crate::model::guild::GuildMember;
use crate::model::user::User;

#[allow(clippy::empty_enum)]
#[derive(Debug, PartialEq)]
pub enum Unused {}

#[allow(clippy::empty_enum)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Used {}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionUse<Usability> {
    /// id of the interaction
    pub id: InteractionId,
    /// the id of the command being invoked
    pub command: CommandId,
    /// the channel it was sent from
    pub channel: ChannelId,
    pub source: InteractionSource,
    // /// the guild it was sent from
    // pub guild: Option<GuildId>,
    // /// guild member data for the invoking user
    // pub member: Option<GuildMember>,
    // /// user object for the invoking user, if invoked in a DM
    // pub user: Option<User>,
    /// a continuation token for responding to the interaction
    pub token: String,
    _priv: PhantomData<Usability>,
}

impl<U> InteractionUse<U> {
    pub fn guild(&self) -> Option<GuildId> {
        match &self.source {
            InteractionSource::Guild(gs) => Some(gs.id),
            InteractionSource::Dm { .. } => None
        }
    }

    pub fn user(&self) -> &User {
        match &self.source {
            InteractionSource::Guild(GuildSource { member, .. }) => &member.user,
            InteractionSource::Dm { user } => user,
        }
    }

    pub fn member(&self) -> Option<&GuildMember> {
        match &self.source {
            InteractionSource::Guild(GuildSource { member, .. }) => Some(member),
            InteractionSource::Dm { .. } => None,
        }
    }
}

// its not actually self, you dumb clippy::nursery
#[allow(clippy::use_self)]
impl InteractionUse<Unused> {
    pub fn from(interaction: Interaction) -> (Self, ApplicationCommandInteractionData) {
        let Interaction { id, kind: _, data, source, channel_id, token } = interaction;
        let this = Self { id, command: data.id, channel: channel_id, source, token, _priv: PhantomData };
        (this, data)
    }

    // pub async fn respond<Client, Message>(self, client: Client, message: Message) -> Result<InteractionUse<Used>, ClientError>
    //     where Client: AsRef<DiscordClient> + Send,
    //           Message: Into<InteractionMessage> + Send
    // {
    //     let client = client.as_ref();
    //     client.create_interaction_response(
    //         self.id,
    //         &self.token,
    //         InteractionResponse::ChannelMessageWithSource(message.into()),
    //     ).await.map(|_| self.into())
    // }

    pub async fn respond<Client, Message>(self, client: Client, message: Message) -> Result<InteractionUse<Used>, ClientError>
        where Client: AsRef<DiscordClient> + Send,
              Message: Into<InteractionMessage> + Send,
    {
        let client = client.as_ref();
        client.create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::ChannelMessageWithSource(message.into()),
        ).await.map(|_| self.into())
    }

    // pub async fn ack<Client: AsRef<DiscordClient> + Send>(self, client: Client) -> Result<InteractionUse<Used>, ClientError> {
    //     let client = client.as_ref();
    //     client.create_interaction_response(
    //         self.id,
    //         &self.token,
    //         InteractionResponse::Acknowledge,
    //     ).await.map(|_| self.into())
    // }

    pub async fn defer<Client: AsRef<DiscordClient> + Send>(self, client: Client) -> Result<InteractionUse<Used>, ClientError> {
        let client = client.as_ref();
        client.create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::DeferredChannelMessageWithSource,
        ).await.map(|_| self.into())
    }
}

impl InteractionUse<Used> {
    pub async fn edit<B, State, Message>(&mut self, state: State, message: Message) -> Result<(), ClientError>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync,
              Message: Into<InteractionMessage> + Send,
    {
        let state = state.as_ref();
        state.client.edit_interaction_response(
            state.application_id().await,
            &self.token,
            message.into(),
        ).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete<B, State>(self, state: State) -> Result<(), ClientError>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync
    {
        let state = state.as_ref();
        state.client.delete_interaction_response(
            state.application_id().await,
            &self.token,
        ).await
    }
}

#[allow(clippy::use_self)]
impl From<InteractionUse<Unused>> for InteractionUse<Used> {
    fn from(InteractionUse { id, command, channel, source, token, _priv }: InteractionUse<Unused>) -> Self {
        Self { id, command, channel, source, token, _priv: PhantomData }
    }
}

// CommandData parsing things
// :( orphan rules :(
pub trait FromCommandOption {
    fn try_from(option: ApplicationCommandInteractionDataOption) -> Result<Self, CommandParseError>
        where Self: Sized;
}

pub trait CommandOptionInto<T> {
    fn try_into(self) -> Result<T, CommandParseError>;
}

impl<T: FromCommandOption> CommandOptionInto<T> for ApplicationCommandInteractionDataOption {
    fn try_into(self) -> Result<T, CommandParseError> {
        T::try_from(self)
    }
}

macro_rules! option_primitives {
    ($($ty:ty, $method:ident, $ctor_fn:ident, $ctor_ty:ty);+ $(;)?) => {
        $(
            impl FromCommandOption for $ty {
                fn try_from(value: ApplicationCommandInteractionDataOption) -> Result<Self, CommandParseError> {
                    let name = value.name;
                    value.value
                         .ok_or_else(|| CommandParseError::EmptyOption(name))?
                         .$method()
                         .map_err(|e| e.into())
                }
            }

            impl OptionCtor for $ty {
                type Data = $ctor_ty;

                fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption {
                    DataOption::$ctor_fn(cdo)
                }
            }
        )+
    };
}

option_primitives!(
    String,    string,  String,  &'static str;
    i64,       int,     Integer, Self;
    bool,      bool,    Boolean, Self;
    UserId,    user,    User,    Self;
    ChannelId, channel, Channel, Self;
    RoleId,    role,    Role,    Self;
);

impl<T: FromCommandOption> FromCommandOption for Option<T> {
    fn try_from(option: ApplicationCommandInteractionDataOption) -> Result<Self, CommandParseError> where Self: Sized {
        Ok(Some(T::try_from(option)?))
    }
}

pub trait DataExt {
    // todo should probably take `Option<GuildId>` or even `InteractionSource` or smth
    fn from_data(data: ApplicationCommandInteractionData, guild: GuildId) -> Result<Self, CommandParseErrorInfo>
        where Self: Sized;
}

impl<T> DataExt for T
    where T: TryFrom<ApplicationCommandInteractionData, Error=(CommandParseError, CommandId)>,
{
    fn from_data(data: ApplicationCommandInteractionData, guild: GuildId) -> Result<Self, CommandParseErrorInfo>
        where Self: Sized {
        Self::try_from(data)
            .map_err(|cpe| cpe.with_info(guild))
    }
}

pub trait OptionChoices {
    fn choices() -> Vec<CommandChoice<&'static str>>;
}

impl<T: OptionChoices> OptionChoices for Option<T> {
    fn choices() -> Vec<CommandChoice<&'static str>> {
        T::choices()
    }
}

pub trait OptionCtor {
    type Data;

    fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption;
}

impl<T: OptionCtor<Data=T>> OptionCtor for Option<T> {
    type Data = T;

    fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption {
        T::option_ctor(cdo)
    }
}

pub trait CommandArgs<Command: SlashCommand> {
    fn args(command: &Command) -> TopLevelOption;
}

// let `()` be used for commands with no options
impl<Command: SlashCommand> CommandArgs<Command> for () {
    fn args(_: &Command) -> TopLevelOption {
        TopLevelOption::Empty
    }
}

impl DataExt for () {
    fn from_data(_: ApplicationCommandInteractionData, _: GuildId) -> Result<Self, CommandParseErrorInfo> where Self: Sized {
        Ok(())
    }
}