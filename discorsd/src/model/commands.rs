use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet};
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use log::warn;
use tokio::time::{Duration, Instant};

use crate::{BotState, utils};
use crate::cache::Cache;
use crate::commands::{Interaction, SlashCommandRaw};
use crate::errors::*;
use crate::http::{ClientResult, DiscordClient};
use crate::http::interaction::WebhookMessage;
use crate::model::{ids::*, interaction::*};
use crate::model::guild::GuildMember;
use crate::model::message::Message;
use crate::model::user::User;

pub trait Usability: PartialEq {}

pub trait NotUnused: Usability {}

#[allow(clippy::empty_enum)]
#[derive(Debug, PartialEq)]
pub enum Unused {}

impl Usability for Unused {}

#[allow(clippy::empty_enum)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Deferred {}

impl Usability for Deferred {}

impl NotUnused for Deferred {}

#[allow(clippy::empty_enum)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Used {}

impl Usability for Used {}

impl NotUnused for Used {}

#[async_trait]
pub trait FinalizeInteraction {
    async fn finalize<B: Send + Sync + 'static>(self, state: &Arc<BotState<B>>) -> ClientResult<InteractionUse<Used>>;
}

#[allow(clippy::use_self)]
#[async_trait]
impl FinalizeInteraction for InteractionUse<Used> {
    async fn finalize<B: Send + Sync + 'static>(self, _: &Arc<BotState<B>>) -> ClientResult<InteractionUse<Used>> {
        Ok(self)
    }
}

#[allow(clippy::use_self)]
#[async_trait]
impl FinalizeInteraction for InteractionUse<Deferred> {
    async fn finalize<B: Send + Sync + 'static>(self, state: &Arc<BotState<B>>) -> ClientResult<InteractionUse<Used>> {
        self.delete(state).await
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionUse<Usability: self::Usability> {
    /// id of the interaction
    pub id: InteractionId,
    /// the id of the command being invoked
    pub command: CommandId,
    /// the name of the command being invoked
    pub command_name: String,
    /// the channel it was sent from
    pub channel: ChannelId,
    pub source: InteractionSource,
    /// a continuation token for responding to the interaction
    pub token: String,
    _priv: PhantomData<Usability>,
}

impl<Use: Usability> Id for InteractionUse<Use> {
    type Id = InteractionId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl<U: Usability> InteractionUse<U> {
    pub fn guild(&self) -> Option<GuildId> {
        match &self.source {
            InteractionSource::Guild(gs) => Some(gs.id),
            InteractionSource::Dm(_) => None
        }
    }

    pub fn user(&self) -> &User {
        match &self.source {
            InteractionSource::Guild(GuildSource { member, .. }) => &member.user,
            InteractionSource::Dm(DmSource { user }) => user,
        }
    }

    pub fn member(&self) -> Option<&GuildMember> {
        match &self.source {
            InteractionSource::Guild(GuildSource { member, .. }) => Some(member),
            InteractionSource::Dm(_) => None,
        }
    }
}

// its not actually self, you dumb clippy::nursery
#[allow(clippy::use_self)]
impl InteractionUse<Unused> {
    pub fn from(interaction: Interaction) -> (Self, InteractionDataOption) {
        let Interaction { id, kind: _, data: InteractionData { id: command, name, options }, source, channel_id, token } = interaction;
        let this = Self { id, command, command_name: name, channel: channel_id, source, token, _priv: PhantomData };
        (this, options)
    }

    pub async fn respond<Client, Message>(self, client: Client, message: Message) -> ClientResult<InteractionUse<Used>>
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

    pub async fn defer<Client: AsRef<DiscordClient> + Send>(self, client: Client) -> ClientResult<InteractionUse<Deferred>> {
        let client = client.as_ref();
        client.create_interaction_response(
            self.id,
            &self.token,
            InteractionResponse::DeferredChannelMessageWithSource,
        ).await.map(|_| self.into())
    }

    pub async fn delete<B, State>(self, state: State) -> ClientResult<InteractionUse<Used>>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send,
    {
        let client = state.as_ref();
        self.defer(client).await?.delete(&client).await
    }
}

impl InteractionUse<Used> {
    pub async fn edit<B, State, Message>(&mut self, state: State, message: Message) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync,
              Message: Into<InteractionMessage> + Send,
    {
        let state = state.as_ref();
        state.client.edit_interaction_response(
            state.application_id(),
            &self.token,
            message.into(),
        ).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete<B, State>(self, state: State) -> ClientResult<()>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync
    {
        let state = state.as_ref();
        state.client.delete_interaction_response(
            state.application_id(),
            &self.token,
        ).await
    }

    pub async fn followup<B, State, Message>(&self, state: State, message: Message) -> ClientResult<crate::model::message::Message>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync,
              Message: Into<WebhookMessage> + Send,
    {
        let state = state.as_ref();
        state.client.create_followup_message(
            state.application_id(),
            &self.token,
            message.into(),
        ).await
    }
}

#[allow(clippy::use_self)]
impl InteractionUse<Deferred> {
    pub async fn followup<B, State, Message>(&self, state: State, message: Message) -> ClientResult<crate::model::message::Message>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync,
              Message: Into<WebhookMessage> + Send,
    {
        let state = state.as_ref();
        state.client.create_followup_message(
            state.application_id(),
            &self.token,
            message.into(),
        ).await
    }

    pub async fn edit<B, State, Message>(self, state: State, message: Message) -> ClientResult<InteractionUse<Used>>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync,
              Message: Into<InteractionMessage> + Send,
    {
        let state = state.as_ref();
        state.client.edit_interaction_response(
            state.application_id(),
            &self.token,
            message.into(),
        ).await?;
        Ok(self.into())
    }

    pub async fn delete<B, State>(self, state: State) -> ClientResult<InteractionUse<Used>>
        where B: Send + Sync + 'static,
              State: AsRef<BotState<B>> + Send + Sync
    {
        let state = state.as_ref();
        state.client.delete_interaction_response(
            state.application_id(),
            &self.token,
        ).await?;
        Ok(self.into())
    }
}

impl<U: NotUnused + Sync> InteractionUse<U> {
    pub async fn get_message(
        &self,
        cache: &Cache,
        period: Duration,
        timeout: Duration,
    ) -> Option<Message> {
        let start = Instant::now();
        let mut interval = tokio::time::interval(period);
        loop {
            let now = interval.tick().await;
            if let Some(message) = cache.interaction_response(self).await {
                println!("DONE: {:?}", now - start);
                break Some(message);
            }
            log::info!("MISSED ONE = {:?}", now - start);
            if now - start > timeout {
                break None;
            }
        }
    }
}

#[allow(clippy::use_self)]
impl From<InteractionUse<Unused>> for InteractionUse<Used> {
    fn from(InteractionUse { id, command, command_name, channel, source, token, _priv }: InteractionUse<Unused>) -> Self {
        Self { id, command, command_name, channel, source, token, _priv: PhantomData }
    }
}

#[allow(clippy::use_self)]
impl From<InteractionUse<Unused>> for InteractionUse<Deferred> {
    fn from(InteractionUse { id, command, command_name, channel, source, token, _priv }: InteractionUse<Unused>) -> Self {
        Self { id, command, command_name, channel, source, token, _priv: PhantomData }
    }
}

#[allow(clippy::use_self)]
impl From<InteractionUse<Deferred>> for InteractionUse<Used> {
    fn from(InteractionUse { id, command, command_name, channel, source, token, _priv }: InteractionUse<Deferred>) -> Self {
        Self { id, command, command_name, channel, source, token, _priv: PhantomData }
    }
}

// begin magic happy traits that let the proc macros be epic

macro_rules! option_primitives {
    ($($ty:ty, $method:ident, $ctor_fn:ident, $ctor_ty:ty);+ $(;)?) => {
        $(
            #[allow(clippy::use_self)]
            impl<C: SlashCommandRaw> CommandData<C> for $ty {
                type Options = ValueOption;

                fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
                    options.lower.$method().map_err(|e| e.into())
                }

                type VecArg = DataOption;

                fn make_args(_: &C) -> Vec<Self::VecArg> {
                    unreachable!()
                }

                type Choice = Self;
            }

            impl OptionCtor for $ty {
                type Data = $ctor_ty;
                const ARG_NAME: &'static str = stringify!($method);

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

macro_rules! option_integers {
    ($($ty:ty, $parsed_type:ident);+ $(;)?) => {
        $(
            #[allow(clippy::use_self)]
            impl<C: SlashCommandRaw> CommandData<C> for $ty {
                type Options = ValueOption;

                fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
                    use std::convert::TryInto;
                    options.lower.int()
                        .and_then(|i64| i64.try_into().map_err(|_| OptionType {
                            value: OptionValue::Integer(i64),
                            desired: CommandOptionTypeParsed::$parsed_type,
                        }))
                        .map_err(|e| e.into())
                }

                type VecArg = DataOption;

                fn make_args(_: &C) -> Vec<Self::VecArg> {
                    unreachable!()
                }

                type Choice = Self;
            }

            impl OptionCtor for $ty {
                type Data = i64;
                // todo specify positive integer?
                const ARG_NAME: &'static str = "int";

                fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption {
                    DataOption::Integer(cdo)
                }
            }
        )+
    };
}
option_integers!(
    usize, Usize;
    u64, U64;
);

macro_rules! option_ids {
    ($($id:ty, $cotp:ident, $name:literal);+ $(;)?) => {
        $(
            impl<C: SlashCommandRaw> CommandData<C> for $id {
                type Options = ValueOption;

                fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
                    options.lower.string()
                        .and_then(|s| s.parse().map_err(|_| OptionType {
                            value: OptionValue::String(s),
                            desired: CommandOptionTypeParsed::$cotp,
                        }))
                        .map_err(|e| e.into())
                }

                type VecArg = DataOption;

                fn make_args(_: &C) -> Vec<Self::VecArg> {
                    unreachable!()
                }

                type Choice = Self;
            }

            impl OptionCtor for $id {
                type Data = &'static str;
                const ARG_NAME: &'static str = $name;

                fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption {
                    DataOption::String(cdo)
                }
            }
        )+
    };
}
option_ids!(
    MessageId, MessageId, "message";
    GuildId, GuildId, "guild";
);

pub trait OptionCtor {
    type Data;

    /// Get the name of this for generic types that implement [`CommandData`]
    const ARG_NAME: &'static str;

    fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption;
}

impl<T: OptionCtor<Data=T>> OptionCtor for Option<T> {
    type Data = T;

    const ARG_NAME: &'static str = T::ARG_NAME;

    fn option_ctor(cdo: CommandDataOption<Self::Data>) -> DataOption {
        T::option_ctor(cdo)
    }
}

// traits to let enums figure out how to impl CommandData
pub enum Highest {}

pub enum Lowest {}

pub trait VecArgLadder: Sized {
    type Raise: VecArgLadder;
    type Lower: VecArgLadder;
    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption;
    fn make<N, D>(name: N, desc: D, lower_options: Vec<Self::Lower>) -> Self
        where N: Into<Cow<'static, str>>,
              D: Into<Cow<'static, str>>;
}

impl VecArgLadder for Highest {
    type Raise = Self;
    // todo should maybe just be self?
    type Lower = SubCommandGroup;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        unreachable!("should never have a `Highest`")
    }

    fn make<N, D>(_: N, _: D, _: Vec<Self::Lower>) -> Self
        where N: Into<Cow<'static, str>>,
              D: Into<Cow<'static, str>> {
        unreachable!("should never have a `Highest`")
    }
}

impl VecArgLadder for SubCommandGroup {
    type Raise = Highest;
    type Lower = SubCommand;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        TopLevelOption::Groups
    }

    fn make<N, D>(name: N, desc: D, lower_options: Vec<Self::Lower>) -> Self
        where N: Into<Cow<'static, str>>,
              D: Into<Cow<'static, str>> {
        Self { name: name.into(), description: desc.into(), sub_commands: lower_options }
    }
}

impl VecArgLadder for SubCommand {
    type Raise = SubCommandGroup;
    type Lower = DataOption;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        TopLevelOption::Commands
    }

    fn make<N, D>(name: N, desc: D, lower_options: Vec<Self::Lower>) -> Self
        where N: Into<Cow<'static, str>>,
              D: Into<Cow<'static, str>> {
        Self { name: name.into(), description: desc.into(), options: lower_options }
    }
}

impl VecArgLadder for DataOption {
    type Raise = SubCommand;
    type Lower = Lowest;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        TopLevelOption::Data
    }

    fn make<N, D>(_: N, _: D, _: Vec<Self::Lower>) -> Self
        where N: Into<Cow<'static, str>>,
              D: Into<Cow<'static, str>> {
        // Self::String(CommandDataOption::new(name, desc))
        unimplemented!("this should be covered by the proc-macro for structs?")
    }
}

impl VecArgLadder for Lowest {
    // todo should maybe be Self?
    type Raise = DataOption;
    type Lower = Self;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        unreachable!("should never have a `Lowest`")
    }

    fn make<N, D>(_: N, _: D, _: Vec<Self::Lower>) -> Self
        where N: Into<Cow<'static, str>>,
              D: Into<Cow<'static, str>> {
        unreachable!("should never have a `Lowest`")
    }
}

impl VecArgLadder for () {
    type Raise = ();
    type Lower = ();

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        fn ctor(_: Vec<()>) -> TopLevelOption {
            TopLevelOption::Empty
        }
        ctor
    }

    fn make<N, D>(_: N, _: D, _: Vec<Self::Lower>) -> Self
        where N: Into<Cow<'static, str>>,
              D: Into<Cow<'static, str>> {
        unimplemented!()
    }
}

pub trait OptionsLadder: Sized {
    type Raise: OptionsLadder;
    type Lower: OptionsLadder;

    fn from_data_option(data: InteractionDataOption) -> Result<Self, CommandParseError>;
}

impl OptionsLadder for Highest {
    // todo should maybe just be self?
    type Raise = Self;
    type Lower = InteractionDataOption;

    fn from_data_option(_: InteractionDataOption) -> Result<Self, CommandParseError> {
        unreachable!("should never have a `Highest`")
    }
}

impl OptionsLadder for InteractionDataOption {
    type Raise = Highest;
    type Lower = GroupOption;

    fn from_data_option(data: InteractionDataOption) -> Result<Self, CommandParseError> {
        Ok(data)
    }
}

impl OptionsLadder for GroupOption {
    type Raise = InteractionDataOption;
    type Lower = CommandOption;

    fn from_data_option(data: InteractionDataOption) -> Result<Self, CommandParseError> {
        match data {
            InteractionDataOption::Group(group) => Ok(group),
            InteractionDataOption::Command(_) => Err(CommandParseError::BadCommandOccurrence),
            InteractionDataOption::Values(_) => Err(CommandParseError::BadGroupOccurrence),
        }
    }
}

impl OptionsLadder for CommandOption {
    type Raise = GroupOption;
    type Lower = Vec<ValueOption>;

    fn from_data_option(data: InteractionDataOption) -> Result<Self, CommandParseError> {
        match data {
            InteractionDataOption::Group(_) => Err(CommandParseError::BadGroupOccurrence),
            InteractionDataOption::Command(command) => Ok(command),
            InteractionDataOption::Values(_) => Err(CommandParseError::BadGroupOccurrence),
        }
    }
}

impl OptionsLadder for Vec<ValueOption> {
    type Raise = CommandOption;
    type Lower = ValueOption;

    fn from_data_option(data: InteractionDataOption) -> Result<Self, CommandParseError> {
        match data {
            InteractionDataOption::Group(_) => Err(CommandParseError::BadGroupOccurrence),
            InteractionDataOption::Command(_) => Err(CommandParseError::BadCommandOccurrence),
            InteractionDataOption::Values(values) => Ok(values),
        }
    }
}

#[allow(clippy::use_self)]
impl OptionsLadder for ValueOption {
    type Raise = Vec<ValueOption>;
    type Lower = Lowest;

    fn from_data_option(data: InteractionDataOption) -> Result<Self, CommandParseError> {
        match data {
            InteractionDataOption::Group(_) => Err(CommandParseError::BadGroupOccurrence),
            InteractionDataOption::Command(_) => Err(CommandParseError::BadCommandOccurrence),
            InteractionDataOption::Values(mut values) => {
                warn!("This probably shouldn't be happening???");
                warn!("values = {:?}", values);
                Ok(values.remove(0))
            }
        }
    }
}

impl OptionsLadder for Lowest {
    // todo should just be self?
    type Raise = ValueOption;
    type Lower = Self;

    fn from_data_option(_: InteractionDataOption) -> Result<Self, CommandParseError> {
        unreachable!("should never have a `Lowest`")
    }
}

impl OptionsLadder for () {
    type Raise = ();
    type Lower = ();

    fn from_data_option(_: InteractionDataOption) -> Result<Self, CommandParseError> {
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum VarargState {
    Fixed(usize),
    Variable,
    None,
}

impl VarargState {
    pub fn number(self) -> Option<usize> {
        match self {
            Self::Fixed(n) => Some(n),
            Self::Variable => None,
            Self::None => panic!("Tried to get the number of varargs for a non-vararg option"),
        }
    }
}

/// the big boi himself
pub trait CommandData<Command: SlashCommandRaw>: Sized {
    type Options: OptionsLadder + Send;
    /// function to go from (the options in a) `InteractionData` -> Self
    fn from_options(options: Self::Options) -> Result<Self, CommandParseError>;

    type VecArg: VecArgLadder;
    // todo: VecArg *maybe* should have the Vec<> on it, so that this can just return one?
    //  do I ever actually return vec![one] or just do I always panic?
    /// functionality to got from Self -> Command for sending to Discord
    fn make_args(command: &Command) -> Vec<Self::VecArg>;
    type Choice;
    fn make_choices() -> Vec<Self::Choice> {
        Vec::new()
    }
    fn into_command_choice(self) -> CommandChoice<&'static str> {
        unreachable!("todo this whole shabang could be less ugly")
    }
    fn vararg_number() -> VarargState { VarargState::None }
}

// let `()` be used for commands with no options
impl<Command: SlashCommandRaw> CommandData<Command> for () {
    type Options = InteractionDataOption;

    fn from_options(_: Self::Options) -> Result<Self, CommandParseError> {
        Ok(())
    }

    type VecArg = ();

    fn make_args(_: &Command) -> Vec<Self::VecArg> {
        Vec::new()
    }

    type Choice = ();
}

// impl for some containers
impl<C: SlashCommandRaw, T: CommandData<C>> CommandData<C> for Option<T> {
    type Options = T::Options;

    fn from_options(data: Self::Options) -> Result<Self, CommandParseError> {
        // `T::from_data` failing means that the data was the wrong type, not that it was absent
        // Absent data is handled before calling this function
        Ok(Some(T::from_options(data)?))
    }

    type VecArg = T::VecArg;

    fn make_args(command: &C) -> Vec<Self::VecArg> {
        T::make_args(command)
    }

    type Choice = T::Choice;
}

impl<T, C, S> CommandData<C> for HashSet<T, S>
    where
        T: CommandData<C, VecArg=DataOption, Options=ValueOption, Choice=T> + Eq + Hash,
        C: SlashCommandRaw,
        S: BuildHasher + Default,
{
    type Options = Vec<ValueOption>;

    fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
        options.into_iter().map(T::from_options).collect()
    }

    type VecArg = DataOption;

    fn make_args(c: &C) -> Vec<Self::VecArg> {
        T::make_args(c)
    }

    type Choice = T;
    fn make_choices() -> Vec<T> {
        T::make_choices()
    }

    fn vararg_number() -> VarargState {
        VarargState::Variable
    }
}

impl<T, C> CommandData<C> for BTreeSet<T>
    where
        T: CommandData<C, VecArg=DataOption, Options=ValueOption, Choice=T> + Ord,
        C: SlashCommandRaw,
{
    type Options = Vec<ValueOption>;

    fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
        options.into_iter().map(T::from_options).collect()
    }

    type VecArg = DataOption;

    fn make_args(c: &C) -> Vec<Self::VecArg> {
        T::make_args(c)
    }

    type Choice = T;
    fn make_choices() -> Vec<T> {
        T::make_choices()
    }

    fn vararg_number() -> VarargState {
        VarargState::Variable
    }
}

#[allow(clippy::use_self)]
impl<T, C> CommandData<C> for Vec<T>
    where
        T: CommandData<C, VecArg=DataOption, Options=ValueOption, Choice=T>,
        C: SlashCommandRaw,
{
    type Options = Vec<ValueOption>;

    fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
        options.into_iter().map(T::from_options).collect()
    }

    type VecArg = DataOption;

    fn make_args(c: &C) -> Vec<Self::VecArg> {
        T::make_args(c)
    }

    type Choice = T;
    fn make_choices() -> Vec<T> {
        T::make_choices()
    }

    fn vararg_number() -> VarargState {
        VarargState::Variable
    }
}

impl<T, C, const N: usize> CommandData<C> for [T; N]
    where
        T: CommandData<C, VecArg=DataOption, Options=ValueOption, Choice=T>,
        C: SlashCommandRaw,
{
    type Options = Vec<ValueOption>;

    fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
        let iter = options.into_iter().map(T::from_options);
        utils::array_try_from_iter(iter, |i| CommandParseError::MissingOption(
            format!("Didn't have option number {}", i + 1)
        ))
    }

    type VecArg = DataOption;

    fn make_args(command: &C) -> Vec<Self::VecArg> {
        T::make_args(command)
    }

    type Choice = T;
    fn make_choices() -> Vec<T> {
        T::make_choices()
    }

    fn vararg_number() -> VarargState {
        VarargState::Fixed(N)
    }
}