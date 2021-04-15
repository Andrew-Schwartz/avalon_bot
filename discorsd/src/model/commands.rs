use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use log::warn;

use crate::{BotState, utils};
use crate::commands::{Interaction, SlashCommand};
use crate::errors::*;
use crate::http::{ClientResult, DiscordClient};
use crate::http::interaction::WebhookMessage;
use crate::model::{ids::*, interaction::*};
use crate::model::guild::GuildMember;
use crate::model::user::User;

pub trait Usability {}

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

// todo is PartialEq implied?
impl<Use: Usability + PartialEq> Id for InteractionUse<Use> {
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
            state.application_id().await,
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
            state.application_id().await,
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
            state.application_id().await,
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
            state.application_id().await,
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
            state.application_id().await,
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
            state.application_id().await,
            &self.token,
        ).await?;
        Ok(self.into())
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
            impl<C: SlashCommand> CommandData<C> for $ty {
                type Options = ValueOption;

                fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
                    options.lower.$method().map_err(|e| e.into())
                }

                type VecArg = ();

                fn make_args(_: &C) -> Vec<Self::VecArg> {
                    unimplemented!()
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

// traits to let enums figure out how to impl CommandData

pub enum Highest {}

pub enum Lowest {}

pub trait VecArgLadder: Sized {
    type Raise: VecArgLadder;
    type Lower: VecArgLadder;
    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption;
    fn make(name: &'static str, desc: &'static str, lower_options: Vec<Self::Lower>) -> Self;
}

impl VecArgLadder for Highest {
    type Raise = Self;
    // todo should maybe just be self?
    type Lower = SubCommandGroup;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        unreachable!("should never have a `Highest`")
    }

    fn make(_: &'static str, _: &'static str, _: Vec<Self::Lower>) -> Self {
        unreachable!("should never have a `Highest`")
    }
}

impl VecArgLadder for SubCommandGroup {
    type Raise = Highest;
    type Lower = SubCommand;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        TopLevelOption::Groups
    }

    fn make(name: &'static str, desc: &'static str, lower_options: Vec<Self::Lower>) -> Self {
        Self { name, description: desc, sub_commands: lower_options }
    }
}

impl VecArgLadder for SubCommand {
    type Raise = SubCommandGroup;
    type Lower = DataOption;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        TopLevelOption::Commands
    }

    fn make(name: &'static str, desc: &'static str, lower_options: Vec<Self::Lower>) -> Self {
        Self { name, description: desc, options: lower_options }
    }
}

impl VecArgLadder for DataOption {
    type Raise = SubCommand;
    type Lower = Lowest;

    fn tlo_ctor() -> fn(Vec<Self>) -> TopLevelOption {
        TopLevelOption::Data
    }

    fn make(_name: &'static str, _desc: &'static str, _: Vec<Self::Lower>) -> Self {
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

    fn make(_: &'static str, _: &'static str, _: Vec<Self::Lower>) -> Self {
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

    fn make(_: &'static str, _: &'static str, _: Vec<Self::Lower>) -> Self {
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

// the big boi himself
pub trait CommandData<Command: SlashCommand>: Sized {
    // function to go from InteractionData -> Self
    type Options: OptionsLadder + Send;
    // has to return the name on a Err because it's moved
    fn from_options(options: Self::Options) -> Result<Self, CommandParseError>;

    // functionality to got from Self -> Command for sending to Discord
    type VecArg: VecArgLadder;
    fn make_args(command: &Command) -> Vec<Self::VecArg>;
    #[allow(unused_variables)]
    fn make_choices(command: &Command) -> Vec<CommandChoice<&'static str>> {
        Vec::new()
    }
    // todo fn that returns Option<usize> for # of varargs (ex hashmap is None, [T;N] is N)
    //  maybe some other return type to differentiate between not-vararg and unknown #
    //  that means the `from_options` for the vararg types would have to be prepared before sending
    //  it into that function to have the right number (N or the runtime chosen value)
    //  update: that fn isn't ever called for varargs! should fix that and add this
}

// let `()` be used for commands with no options
impl<Command: SlashCommand> CommandData<Command> for () {
    type Options = InteractionDataOption;

    fn from_options(_: Self::Options) -> Result<Self, CommandParseError> {
        Ok(())
    }

    type VecArg = ();

    fn make_args(_: &Command) -> Vec<Self::VecArg> {
        Vec::new()
    }
}

// impl for some containers
impl<C: SlashCommand, T: CommandData<C>> CommandData<C> for Option<T> {
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
}

impl<T, C, S> CommandData<C> for HashSet<T, S>
    where
        T: CommandData<C, VecArg=DataOption, Options=ValueOption> + Eq + Hash,
        C: SlashCommand,
        S: BuildHasher + Default,
{
    type Options = Vec<ValueOption>;

    fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
        options.into_iter().map(T::from_options).collect()
    }

    type VecArg = DataOption;

    fn make_args(c: &C) -> Vec<Self::VecArg> {
        // Vec::new()
        T::make_args(c)
    }

    fn make_choices(c: &C) -> Vec<CommandChoice<&'static str>> {
        T::make_choices(c)
    }
}

#[allow(clippy::use_self)]
impl<T, C> CommandData<C> for Vec<T>
    where
        T: CommandData<C, VecArg=DataOption, Options=ValueOption>,
        C: SlashCommand,
{
    type Options = Vec<ValueOption>;

    // todo lol this is never even called!
    fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
        println!("options = {:?}", options);
        options.into_iter().map(T::from_options).collect()
    }

    type VecArg = DataOption;

    fn make_args(command: &C) -> Vec<Self::VecArg> {
        T::make_args(command)
    }

    fn make_choices(command: &C) -> Vec<CommandChoice<&'static str>> {
        T::make_choices(command)
    }
}

impl<T, C, const N: usize> CommandData<C> for [T; N]
    where
        T: CommandData<C, VecArg=DataOption, Options=ValueOption>,
        C: SlashCommand,
{
    type Options = Vec<ValueOption>;

    fn from_options(options: Self::Options) -> Result<Self, CommandParseError> {
        let iter = options.into_iter().map(T::from_options);
        utils::array_try_from_iter(iter, |i| CommandParseError::MissingOption(
            format!("Didn't have option number {}", i)
        ))
    }

    type VecArg = DataOption;

    fn make_args(command: &C) -> Vec<Self::VecArg> {
        T::make_args(command)
    }

    fn make_choices(command: &C) -> Vec<CommandChoice<&'static str>> {
        T::make_choices(command)
    }
}