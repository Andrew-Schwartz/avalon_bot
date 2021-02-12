use std::convert::TryFrom;
use std::marker::PhantomData;

use crate::BotState;
use crate::errors::*;
use crate::http::{ClientError, DiscordClient};
use crate::model::{ids::*, interaction::*};
use crate::model::guild::GuildMember;

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
    /// the guild it was sent from
    pub guild: GuildId,
    /// the channel it was sent from
    pub channel: ChannelId,
    /// guild member data for the invoking user
    pub member: GuildMember,
    /// a continuation token for responding to the interaction
    pub token: String,
    _priv: PhantomData<Usability>,
}

// its not actually self, you dumb clippy::nursery
#[allow(clippy::use_self)]
impl InteractionUse<Unused> {
    pub fn from(interaction: Interaction) -> (Self, ApplicationCommandInteractionData) {
        let Interaction { id, kind: _kind, data, guild_id, channel_id, member, token } = interaction;
        let this = Self { id, command: data.id, guild: guild_id, channel: channel_id, member, token, _priv: PhantomData };
        (this, data)
    }

    pub async fn respond<Client, Message>(self, client: Client, message: Message) -> Result<InteractionUse<Used>, ClientError>
        where Client: AsRef<DiscordClient> + Send,
              Message: Into<InteractionMessage> + Send
    {
        let client = client.as_ref();
        client.create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::Message(message.into()),
        ).await.map(|_| self.into())
    }

    pub async fn respond_source<Client, Message>(self, client: Client, message: Message) -> Result<InteractionUse<Used>, ClientError>
        where Client: AsRef<DiscordClient> + Send,
              Message: Into<InteractionMessage> + Send,
    {
        let client = client.as_ref();
        client.create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::MessageWithSource(message.into()),
        ).await.map(|_| self.into())
    }

    pub async fn ack<Client: AsRef<DiscordClient> + Send>(self, client: Client) -> Result<InteractionUse<Used>, ClientError> {
        let client = client.as_ref();
        client.create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::Acknowledge,
        ).await.map(|_| self.into())
    }

    pub async fn ack_source<Client: AsRef<DiscordClient> + Send>(self, client: Client) -> Result<InteractionUse<Used>, ClientError> {
        let client = client.as_ref();
        client.create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::AckWithSource,
        ).await.map(|_| self.into())
    }
}

impl InteractionUse<Used> {
    pub async fn edit<B: Send + Sync + 'static, State: AsRef<BotState<B>> + Send + Sync>(&mut self, state: State, message: InteractionMessage) -> Result<(), ClientError> {
        let state = state.as_ref();
        state.client.edit_interaction_response(
            state.application_id().await,
            &self.token,
            message,
        ).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete<B: Send + Sync + 'static, State: AsRef<BotState<B>> + Send + Sync>(self, state: State) -> Result<(), ClientError> {
        let state = state.as_ref();
        state.client.delete_interaction_response(
            state.application_id().await,
            &self.token,
        ).await
    }
}

#[allow(clippy::use_self)]
impl From<InteractionUse<Unused>> for InteractionUse<Used> {
    fn from(InteractionUse { id, command, guild, channel, member, token, _priv }: InteractionUse<Unused>) -> Self {
        Self { id, command, guild, channel, member, token, _priv: PhantomData }
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