use std::borrow::Cow;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use itertools::Itertools;
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::SerializeSeq;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::http::channel::RichEmbed;
use crate::http::model::{AllowedMentions, GuildMember, MessageFlags};
use crate::http::model::ids::*;

/*
(\w+)\??\**\t\??(.+)\t(.+)

/// $3
\tpub $1: $2,
*/

#[derive(Serialize, Debug, Clone)]
pub struct Command {
    name: &'static str,
    description: Cow<'static, str>,
    options: TopLevelOption,
}

impl Command {
    pub fn new<D: Into<Cow<'static, str>>>(name: &'static str, description: D, options: TopLevelOption) -> Self {
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "command names must only contain letters, numbers, and `_`; name = `{:?}`",
            name
        );
        assert!(
            3 <= name.len() && name.len() <= 32,
            "command names must be 3-32 characters long ({} is {} characters)",
            name, name.len()
        );
        let description = description.into();
        let dlen = description.chars().count();
        assert!(
            1 <= dlen && dlen <= 100,
            "command descriptions must be 1-100 characters long ({} is {} characters)",
            description, dlen
        );
        Self { name, description, options }
    }

    pub fn options(self) -> TopLevelOption {
        self.options
    }
}

impl Hash for Command {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[derive(Debug, Clone)]
pub enum TopLevelOption {
    Commands(Vec<SubCommand>),
    Groups(Vec<SubCommandGroup>),
    Data(Vec<DataOption>),
    Empty,
}

impl TopLevelOption {
    pub fn empty() -> Self { Self::Empty }

    // todo other ctors, doc for TLO saying to use the functions
    //  (maybe TLO should be private, then these are different functions on Command? but then edit...)
    pub fn options(options: Vec<DataOption>) -> Self {
        assert!(
            options.iter()
                .filter(|o| o.default()).count() <= 1,
            "only one option can be default"
        );
        assert!(
            !options.iter()
                .skip_while(|o| o.required())
                .any(|o| o.required()),
            "all required options must be at front of list"
        );
        assert_eq!(
            options.iter()
                .map(|o| o.name())
                .unique()
                .count(),
            options.len(),
            "must not repeat option names"
        );

        Self::Data(options)
    }
}

impl Serialize for TopLevelOption {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TopLevelOption::Commands(subs) => subs.serialize(s),
            TopLevelOption::Groups(groups) => groups.serialize(s),
            TopLevelOption::Data(opts) => opts.serialize(s),
            TopLevelOption::Empty => s.serialize_seq(Some(0))?.end(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubCommand {
    /// 1-32 character name
    pub name: &'static str,
    /// 1-100 character description
    pub description: &'static str,
    /// the parameters to this subcommand
    pub options: Vec<DataOption>,
}

impl Serialize for SubCommand {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        SerializeOption {
            kind: ApplicationCommandOptionType::SubCommand,
            name: self.name.into(),
            description: self.description.into(),
            default: false,
            required: false,
            choices: vec![],
            options: Some(&self.options),
        }.serialize(s)
    }
}

#[derive(Debug, Clone)]
pub struct SubCommandGroup {
    /// 1-32 character name
    pub name: &'static str,
    /// 1-100 character description
    pub description: &'static str,
    /// the subcommands in this subcommand group
    pub sub_commands: Vec<SubCommand>,
}

impl Serialize for SubCommandGroup {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        SerializeOption {
            kind: ApplicationCommandOptionType::SubCommandGroup,
            name: self.name.into(),
            description: self.description.into(),
            default: false,
            required: false,
            choices: vec![],
            options: Some(&self.sub_commands),
        }.serialize(s)
    }
}

#[derive(Debug, Clone)]
pub enum DataOption {
    String(CommandDataOption<&'static str>),
    Integer(CommandDataOption<i64>),
    Boolean(CommandDataOption<bool>),
    User(CommandDataOption<UserId>),
    Channel(CommandDataOption<ChannelId>),
    Role(CommandDataOption<RoleId>),
}

impl Serialize for DataOption {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use ApplicationCommandOptionType::*;
        match self {
            DataOption::String(opt) => opt.serializable(String),
            DataOption::Integer(opt) => opt.serializable(Integer),
            DataOption::Boolean(opt) => opt.serializable(Boolean),
            DataOption::User(opt) => opt.serializable(User),
            DataOption::Channel(opt) => opt.serializable(Channel),
            DataOption::Role(opt) => opt.serializable(Role),
        }.serialize(s)
    }
}

// impl DataOption {
//     pub fn string(
//         name: &'static str,
//         description: &'static str,
//     ) -> Self {
//         Self::String(CommandDataOption::new(name, description))
//     }
//
//     pub fn int(
//         name: &'static str,
//         description: &'static str,
//     ) -> Self {
//         Self::Integer(CommandDataOption::new(name, description))
//     }
// }

impl DataOption {
    pub fn name(&self) -> &str {
        match self {
            DataOption::String(o) => o.name.as_ref(),
            DataOption::Integer(o) => o.name.as_ref(),
            DataOption::Boolean(o) => o.name.as_ref(),
            DataOption::User(o) => o.name.as_ref(),
            DataOption::Channel(o) => o.name.as_ref(),
            DataOption::Role(o) => o.name.as_ref(),
        }
    }
    pub fn description(&self) -> &str {
        match self {
            DataOption::String(o) => o.description.as_ref(),
            DataOption::Integer(o) => o.description.as_ref(),
            DataOption::Boolean(o) => o.description.as_ref(),
            DataOption::User(o) => o.description.as_ref(),
            DataOption::Channel(o) => o.description.as_ref(),
            DataOption::Role(o) => o.description.as_ref(),
        }
    }
    pub fn default(&self) -> bool {
        match self {
            DataOption::String(o) => o.default,
            DataOption::Integer(o) => o.default,
            DataOption::Boolean(o) => o.default,
            DataOption::User(o) => o.default,
            DataOption::Channel(o) => o.default,
            DataOption::Role(o) => o.default,
        }
    }
    pub fn required(&self) -> bool {
        match self {
            DataOption::String(o) => o.required,
            DataOption::Integer(o) => o.required,
            DataOption::Boolean(o) => o.required,
            DataOption::User(o) => o.required,
            DataOption::Channel(o) => o.required,
            DataOption::Role(o) => o.required,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandDataOption<T> {
    /// 1-32 character name
    name: Cow<'static, str>,
    /// 1-100 character description
    description: Cow<'static, str>,
    /// the first required option for the user to complete--only one option can be default
    default: bool,
    /// if the parameter is required or optional--default false
    required: bool,
    /// choices for string and int types for the user to pick from
    choices: Vec<CommandChoice<T>>,
}

impl<T> CommandDataOption<T> {
    pub fn new<N: Into<Cow<'static, str>>, D: Into<Cow<'static, str>>>(name: N, description: D) -> Self {
        let name = name.into();
        let description = description.into();
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "command names must only contain letters, numbers, and _, name = {:?}",
                name
        );
        assert!(1 <= name.len() && name.len() <= 32, "command names must be 1-32 characters, name = {:?}", name);
        let dlen = description.chars().count();
        assert!(1 <= dlen && dlen <= 100, "command descriptions must be 1-100 characters, description = {:?}", description);

        Self {
            name,
            description,
            default: false,
            required: false,
            choices: vec![],
        }
    }

    pub fn default(mut self) -> Self {
        self.default = true;
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

impl CommandDataOption<&'static str> {
    pub fn new_str(name: &'static str, description: &'static str) -> Self {
        Self::new(name, description)
    }

    pub fn choices(mut self, choices: Vec<CommandChoice<&'static str>>) -> Self {
        self.choices = choices;
        self
    }
}

impl CommandDataOption<i64> {
    pub fn new_int(name: &'static str, description: &'static str) -> Self {
        Self::new(name, description)
    }

    pub fn choices(mut self, choices: Vec<CommandChoice<i64>>) -> Self {
        self.choices = choices;
        self
    }
}

impl<T> CommandDataOption<T>
    where ApplicationCommandOptionChoice: From<CommandChoice<T>>,
          CommandChoice<T>: Copy,
{
    fn serializable(&self, kind: ApplicationCommandOptionType) -> SerializeOption<DataOption> {
        // have to convert `CommandChoice<T>` to `ApplicationCommandOptionChoice` to get rid of the
        // generic type. todo is there a better way to do this? (could make choices: Option<String>?)
        let choices = self.choices
            .iter()
            .copied()
            .map(ApplicationCommandOptionChoice::from)
            .collect();
        SerializeOption {
            kind,
            name: Cow::clone(&self.name),
            description: Cow::clone(&self.description),
            default: self.default,
            required: self.required,
            choices,
            options: None,
        }
    }
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct CommandChoice<T> {
    /// 1-100 character choice name
    name: &'static str,
    /// value of the choice
    value: T,
}

impl<T> CommandChoice<T> {
    pub fn new(name: &'static str, value: T) -> Self {
        let nlen = name.chars().count();
        assert!(1 <= nlen && nlen <= 100, "command names must be 1-100 characters, name = {:?}", name);

        Self { name, value }
    }
}

impl CommandChoice<&'static str> {
    pub fn new_str(name_value: &'static str) -> Self {
        Self::new(name_value, name_value)
    }
}

impl<'a> From<CommandChoice<&'a str>> for ApplicationCommandOptionChoice {
    fn from(choice: CommandChoice<&'a str>) -> Self {
        Self { name: choice.name.to_string(), value: OptionValue::String(choice.value.to_string()) }
    }
}

impl From<CommandChoice<i64>> for ApplicationCommandOptionChoice {
    fn from(choice: CommandChoice<i64>) -> Self {
        Self { name: choice.name.to_string(), value: OptionValue::Integer(choice.value) }
    }
}

impl From<CommandChoice<bool>> for ApplicationCommandOptionChoice {
    fn from(choice: CommandChoice<bool>) -> Self {
        Self { name: choice.name.to_string(), value: OptionValue::Bool(choice.value) }
    }
}

impl From<CommandChoice<UserId>> for ApplicationCommandOptionChoice {
    fn from(choice: CommandChoice<UserId>) -> Self {
        Self { name: choice.name.to_string(), value: OptionValue::String(choice.value.to_string()) }
    }
}

impl From<CommandChoice<ChannelId>> for ApplicationCommandOptionChoice {
    fn from(choice: CommandChoice<ChannelId>) -> Self {
        Self { name: choice.name.to_string(), value: OptionValue::String(choice.value.to_string()) }
    }
}

impl From<CommandChoice<RoleId>> for ApplicationCommandOptionChoice {
    fn from(choice: CommandChoice<RoleId>) -> Self {
        Self { name: choice.name.to_string(), value: OptionValue::String(choice.value.to_string()) }
    }
}

#[derive(Serialize)]
struct SerializeOption<'a, O: Debug> {
    #[serde(rename = "type")]
    pub kind: ApplicationCommandOptionType,
    pub name: Cow<'static, str>,
    pub description: Cow<'static, str>,
    #[serde(skip_serializing_if = "crate::serde_utils::BoolExt::is_false")]
    pub default: bool,
    #[serde(skip_serializing_if = "crate::serde_utils::BoolExt::is_false")]
    pub required: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ApplicationCommandOptionChoice>,
    #[serde(skip_serializing_if = "skip_options")]
    pub options: Option<&'a Vec<O>>,
}

fn skip_options<O: Debug>(options: &Option<&Vec<O>>) -> bool {
    options.filter(|vec| !vec.is_empty()).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_same_json_value(correct: impl AsRef<str>, modeled: impl Serialize) {
        use serde_json::Value;

        let correct: Value = serde_json::from_str(correct.as_ref()).unwrap();
        let modeled = serde_json::to_string_pretty(&modeled).unwrap();
        println!("modeled = {}", modeled);
        let modeled: Value = serde_json::from_str(&modeled).unwrap();

        assert_eq!(correct, modeled);
    }

    #[test]
    fn part1() {
        const CORRECT: &'static str = r#"{
    "name": "permissions",
    "description": "Get or edit permissions for a user or a role",
    "options": []
}"#;
        let command2 = Command::new(
            "permissions",
            "Get or edit permissions for a user or a role",
            TopLevelOption::Empty,
        );

        assert_same_json_value(CORRECT, command2);
    }

    #[test]
    fn part2() {
        const CORRECT: &'static str = r#"{
    "name": "permissions",
    "description": "Get or edit permissions for a user or a role",
    "options": [
        {
            "name": "user",
            "description": "Get or edit permissions for a user",
            "type": 2
        },
        {
            "name": "role",
            "description": "Get or edit permissions for a role",
            "type": 2
        }
    ]
}"#;

        let command = Command::new(
            "permissions",
            "Get or edit permissions for a user or a role",
            TopLevelOption::Groups(vec![
                SubCommandGroup {
                    name: "user",
                    description: "Get or edit permissions for a user",
                    sub_commands: vec![],
                },
                SubCommandGroup {
                    name: "role",
                    description: "Get or edit permissions for a role",
                    sub_commands: vec![],
                }
            ]),
        );

        assert_same_json_value(CORRECT, command);
    }

    #[test]
    fn part3() {
        const CORRECT: &'static str = r#"{
    "name": "permissions",
    "description": "Get or edit permissions for a user or a role",
    "options": [
        {
            "name": "user",
            "description": "Get or edit permissions for a user",
            "type": 2,
            "options": [
                {
                    "name": "get",
                    "description": "Get permissions for a user",
                    "type": 1
                },
                {
                    "name": "edit",
                    "description": "Edit permissions for a user",
                    "type": 1
                }
            ]
        },
        {
            "name": "role",
            "description": "Get or edit permissions for a role",
            "type": 2,
            "options": [
                {
                    "name": "get",
                    "description": "Get permissions for a role",
                    "type": 1
                },
                {
                    "name": "edit",
                    "description": "Edit permissions for a role",
                    "type": 1
                }
            ]
        }
    ]
}"#;

        let command = Command::new(
            "permissions",
            "Get or edit permissions for a user or a role",
            TopLevelOption::Groups(vec![
                SubCommandGroup {
                    name: "user",
                    description: "Get or edit permissions for a user",
                    sub_commands: vec![
                        SubCommand {
                            name: "get",
                            description: "Get permissions for a user",
                            options: vec![],
                        },
                        SubCommand {
                            name: "edit",
                            description: "Edit permissions for a user",
                            options: vec![],
                        }
                    ],
                },
                SubCommandGroup {
                    name: "role",
                    description: "Get or edit permissions for a role",
                    sub_commands: vec![
                        SubCommand {
                            name: "get",
                            description: "Get permissions for a role",
                            options: vec![],
                        },
                        SubCommand {
                            name: "edit",
                            description: "Edit permissions for a role",
                            options: vec![],
                        }
                    ],
                }
            ]),
        );

        assert_same_json_value(CORRECT, command)
    }

    #[test]
    fn part4() {
        const CORRECT: &'static str = r#"{
    "name": "permissions",
    "description": "Get or edit permissions for a user or a role",
    "options": [
        {
            "name": "user",
            "description": "Get or edit permissions for a user",
            "type": 2,
            "options": [
                {
                    "name": "get",
                    "description": "Get permissions for a user",
                    "type": 1,
                    "options": [
                        {
                            "name": "user",
                            "description": "The user to get",
                            "type": 6,
                            "required": true
                        },
                        {
                            "name": "channel",
                            "description": "The channel permissions to get. If omitted, the guild permissions will be returned",
                            "type": 7
                        }
                    ]
                },
                {
                    "name": "edit",
                    "description": "Edit permissions for a user",
                    "type": 1,
                    "options": [
                        {
                            "name": "user",
                            "description": "The user to edit",
                            "type": 6,
                            "required": true
                        },
                        {
                            "name": "channel",
                            "description": "The channel permissions to edit. If omitted, the guild permissions will be edited",
                            "type": 7
                        }
                    ]
                }
            ]
        },
        {
            "name": "role",
            "description": "Get or edit permissions for a role",
            "type": 2,
            "options": [
                {
                    "name": "get",
                    "description": "Get permissions for a role",
                    "type": 1,
                    "options": [
                        {
                            "name": "role",
                            "description": "The role to get",
                            "type": 8,
                            "required": true
                        },
                        {
                            "name": "channel",
                            "description": "The channel permissions to get. If omitted, the guild permissions will be returned",
                            "type": 7
                        }
                    ]
                },
                {
                    "name": "edit",
                    "description": "Edit permissions for a role",
                    "type": 1,
                    "options": [
                        {
                            "name": "role",
                            "description": "The role to edit",
                            "type": 8,
                            "required": true
                        },
                        {
                            "name": "channel",
                            "description": "The channel permissions to edit. If omitted, the guild permissions will be edited",
                            "type": 7
                        }
                    ]
                }
            ]
        }
    ]
}"#;

        let command = Command::new(
            "permissions",
            "Get or edit permissions for a user or a role",
            TopLevelOption::Groups(vec![
                SubCommandGroup {
                    name: "user",
                    description: "Get or edit permissions for a user",
                    sub_commands: vec![
                        SubCommand {
                            name: "get",
                            description: "Get permissions for a user",
                            options: vec![
                                DataOption::User(CommandDataOption::new(
                                    "user",
                                    "The user to get",
                                ).required()),
                                DataOption::Channel(CommandDataOption::new(
                                    "channel",
                                    "The channel permissions to get. If omitted, the guild permissions will be returned",
                                ))
                            ],
                        },
                        SubCommand {
                            name: "edit",
                            description: "Edit permissions for a user",
                            options: vec![
                                DataOption::User(CommandDataOption::new(
                                    "user",
                                    "The user to edit",
                                ).required()),
                                DataOption::Channel(CommandDataOption::new(
                                    "channel",
                                    "The channel permissions to edit. If omitted, the guild permissions will be edited",
                                ))
                            ],
                        }
                    ],
                },
                SubCommandGroup {
                    name: "role",
                    description: "Get or edit permissions for a role",
                    sub_commands: vec![
                        SubCommand {
                            name: "get",
                            description: "Get permissions for a role",
                            options: vec![
                                DataOption::Role(CommandDataOption::new(
                                    "role",
                                    "The role to get",
                                ).required()),
                                DataOption::Channel(CommandDataOption::new(
                                    "channel",
                                    "The channel permissions to get. If omitted, the guild permissions will be returned",
                                ))
                            ],
                        },
                        SubCommand {
                            name: "edit",
                            description: "Edit permissions for a role",
                            options: vec![
                                DataOption::Role(CommandDataOption::new(
                                    "role",
                                    "The role to edit",
                                ).required()),
                                DataOption::Channel(CommandDataOption::new(
                                    "channel",
                                    "The channel permissions to edit. If omitted, the guild permissions will be edited",
                                ))
                            ],
                        }
                    ],
                }
            ]),
        );

        if let TopLevelOption::Groups(groups) = &command.options {
            groups.iter()
                .flat_map(|g| &g.sub_commands)
                .flat_map(|c| &c.options)
                .for_each(|opt| {
                    match opt {
                        DataOption::String(opt) => assert!(matches!(&opt.name, Cow::Borrowed(_))),
                        DataOption::Integer(opt) => assert!(matches!(&opt.name, Cow::Borrowed(_))),
                        DataOption::Boolean(opt) => assert!(matches!(&opt.name, Cow::Borrowed(_))),
                        DataOption::User(opt) => assert!(matches!(&opt.name, Cow::Borrowed(_))),
                        DataOption::Channel(opt) => assert!(matches!(&opt.name, Cow::Borrowed(_))),
                        DataOption::Role(opt) => assert!(matches!(&opt.name, Cow::Borrowed(_))),
                    }
                });
        }

        assert_same_json_value(CORRECT, command);
    }
}

// ^ noice model ^
// ----------------------------------------------------
// v  raw model  v

/// An application command is the base "command" model that belongs to an application.
/// This is what you are creating when you POST a new command.
///
/// A command, or each individual subcommand, can have a maximum of 10 options
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApplicationCommand {
    /// unique id of the command
    pub id: CommandId,
    /// unique id of the parent application
    pub application_id: ApplicationId,
    /// 3-32 character name
    pub name: String,
    /// 1-100 character description
    pub description: String,
    /// the parameters for the command
    #[serde(default)]
    pub options: Vec<ApplicationCommandOption>,
}

/// You can specify a maximum of 10 choices per option.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApplicationCommandOption {
    /// value of ApplicationCommandOptionType
    #[serde(rename = "type")]
    pub kind: ApplicationCommandOptionType,
    /// 1-32 character name
    pub name: String,
    /// 1-100 character description
    pub description: String,
    /// the first required option for the user to complete--only one option can be default
    #[serde(default, skip_serializing_if = "crate::serde_utils::BoolExt::is_false")]
    pub default: bool,
    /// if the parameter is required or optional--default false
    #[serde(default, skip_serializing_if = "crate::serde_utils::BoolExt::is_false")]
    pub required: bool,
    /// choices for string and int types for the user to pick from
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ApplicationCommandOptionChoice>,
    /// if the option is a subcommand or subcommand group type, this nested options will be the parameters
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ApplicationCommandOption>,
}

// honestly this would probably be best as a generic I think?
#[derive(Deserialize_repr, Serialize_repr, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ApplicationCommandOptionType {
    SubCommand = 1,
    SubCommandGroup = 2,
    String = 3,
    Integer = 4,
    Boolean = 5,
    User = 6,
    Channel = 7,
    Role = 8,
}

/// If you specify `choices` for an option, they are the **only** valid values for a user to pick
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApplicationCommandOptionChoice {
    /// 1-100 character choice name
    pub name: String,
    /// value of the choice
    pub value: OptionValue,
}

// todo maybe the None version should be in here?
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum OptionValue {
    String(String),
    Integer(i64),
    Bool(bool),
}

impl OptionValue {
    pub fn unwrap_string(self) -> String {
        if let Self::String(s) = self {
            s
        } else {
            panic!("expected a string")
        }
    }

    pub fn unwrap_int(self) -> i64 {
        if let Self::Integer(i) = self {
            i
        } else {
            panic!("expected an integer")
        }
    }

    pub fn unwrap_bool(self) -> bool {
        if let Self::Bool(b) = self {
            b
        } else {
            panic!("expected a boolean")
        }
    }

    pub fn unwrap_user(self) -> UserId {
        self.unwrap_string().parse().unwrap()
    }

    pub fn unwrap_channel(self) -> ChannelId {
        self.unwrap_string().parse().unwrap()
    }

    pub fn unwrap_role(self) -> RoleId {
        self.unwrap_string().parse().unwrap()
    }
}

/// An interaction is the base "thing" that is sent when a user invokes a command, and is the same
/// for Slash Commands and other future interaction types.
// ooh spooky ^
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Interaction {
    /// id of the interaction
    pub id: InteractionId,
    /// the type of interaction
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// the command data payload
    ///
    /// This is always present on ApplicationCommand interaction types.
    /// It is optional for future-proofing against new interaction types (according to docs, but I'm
    /// cool and can just change it to be optional then :). Also will probably just be a tagged enum)
    pub data: ApplicationCommandInteractionData,
    /// the guild it was sent from
    pub guild_id: GuildId,
    /// the channel it was sent from
    pub channel_id: ChannelId,
    /// guild member data for the invoking user
    pub member: GuildMember,
    /// a continuation token for responding to the interaction
    pub token: String,
    // /// read-only property, always 1
    // pub version: u8,
}

#[derive(Deserialize_repr, Serialize_repr, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum InteractionType {
    Ping = 1,
    ApplicationCommand = 2,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApplicationCommandInteractionData {
    /// the ID of the invoked command
    pub id: CommandId,
    /// the name of the invoked command
    pub name: String,
    /// the params + values from the user
    #[serde(default)]
    pub options: Vec<ApplicationCommandInteractionDataOption>,
}

/// All options have names, and an option can either be a parameter and input value--in which case
/// `value` will be set--or it can denote a subcommand or group--in which case it will contain a
/// top-level key and another array of `options`.
///
/// `value` and `options` are mutually exclusive.
// todo make value/options be an enum
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApplicationCommandInteractionDataOption {
    /// the name of the parameter
    pub name: String,
    /// the value of the pair
    pub value: Option<OptionValue>,
    /// present if this option is a group or subcommand
    #[serde(default)]
    pub options: Vec<ApplicationCommandInteractionDataOption>,
}

/// After receiving an interaction, you must respond to acknowledge it. This may be a `pong` for a
/// `ping`, a message, or simply an acknowledgement that you have received it and will handle the
/// command async.
///
/// Interaction responses may choose to "eat" the user's command input if you do not wish to have
/// their slash command show up as message in chat. This may be helpful for slash commands, or
/// commands whose responses are asynchronous or ephemeral messages.
#[derive(Debug, Clone)]
pub enum InteractionResponse {
    /// ACK a `Ping`
    Pong,
    /// ACK a command without sending a message, eating the user's input
    Acknowledge,
    /// respond with a message, showing the user's input
    Message(InteractionMessage),
    /// ACK a command without sending a message, showing the user's input
    MessageWithSource(InteractionMessage),
    /// respond with a message, eating the user's input
    AckWithSource,
}

impl Serialize for InteractionResponse {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Shim<'a> {
            #[serde(rename = "type")]
            kind: u8,
            data: Option<&'a InteractionMessage>,
        }

        let shim = match self {
            InteractionResponse::Pong => Shim { kind: 1, data: None },
            InteractionResponse::Acknowledge => Shim { kind: 2, data: None },
            InteractionResponse::Message(m) => Shim { kind: 3, data: Some(m) },
            InteractionResponse::MessageWithSource(m) => Shim { kind: 4, data: Some(m) },
            InteractionResponse::AckWithSource => Shim { kind: 5, data: None },
        };

        shim.serialize(s)
    }
}

/// Not all message fields are currently supported.
#[derive(Serialize, Debug, Clone)]
pub struct InteractionMessage {
    /// is the response TTS
    pub tts: bool,
    /// message content
    content: String,
    /// supports up to 10 embeds
    embeds: Vec<RichEmbed>,
    /// allowed mentions object
    pub allowed_mentions: Option<AllowedMentions>,
    /// (undocumented) flags, probalbly for setting EPHEMERAL
    flags: MessageFlags,
}

pub fn message<F: FnOnce(&mut InteractionMessage)>(builder: F) -> InteractionMessage {
    InteractionMessage::build(builder)
}

impl InteractionMessage {
    pub fn new(content: String) -> Self {
        Self {
            tts: false,
            content,
            embeds: vec![],
            allowed_mentions: None,
            flags: MessageFlags::empty(),
        }
    }

    pub fn build_with<F: FnOnce(&mut Self)>(mut with: Self, builder: F) -> Self {
        builder(&mut with);
        with
    }

    pub fn build<F: FnOnce(&mut Self)>(builder: F) -> Self {
        let mut message = Self::new(String::new());
        builder(&mut message);
        message
    }

    pub fn embeds<F: FnMut(usize, &mut RichEmbed)>(&mut self, n: usize, mut builder: F) {
        if self.embeds.len() + n > 10 {
            panic!("can't send more than 10 embeds");
        } else {
            self.embeds.extend(
                (0..n).map(|i| RichEmbed::build(|e| builder(i, e)))
            );
        }
    }

    /// add an embed to the [IntegrationMessage](IntegrationMessage)
    ///
    /// panics if this message already has 10 or more embeds
    pub fn embed<F: FnOnce(&mut RichEmbed)>(&mut self, builder: F) {
        if self.embeds.len() >= 10 {
            panic!("can't send more than 10 embeds");
        } else {
            self.embeds.push(RichEmbed::build(builder));
        }
    }

    /// add an embed to the [IntegrationMessage](IntegrationMessage)
    ///
    /// panics if this message already has 10 or more embeds
    pub fn embed_with<F: FnOnce(&mut RichEmbed)>(&mut self, embed: RichEmbed, builder: F) {
        if self.embeds.len() >= 10 {
            panic!("can't send more than 10 embeds");
        } else {
            self.embeds.push(RichEmbed::build_with(embed, builder));
        }
    }

    /// add an embed to the [IntegrationMessage](IntegrationMessage)
    ///
    /// Returns `Err(builder)` if this message already has 10 or more embeds
    pub fn try_embed<F: FnOnce(&mut RichEmbed)>(&mut self, builder: F) -> Result<(), F> {
        if self.embeds.len() >= 10 {
            Err(builder)
        } else {
            self.embeds.push(RichEmbed::build(builder));
            Ok(())
        }
    }

    pub fn content<S: ToString>(&mut self, content: S) {
        self.content = content.to_string();
    }

    pub fn ephemeral(&mut self) {
        self.flags.set(MessageFlags::EPHEMERAL, true);
    }

    pub fn with_source(self) -> InteractionResponse {
        self.into_response(true)
    }

    pub fn without_source(self) -> InteractionResponse {
        self.into_response(false)
    }

    pub fn into_response(self, with_source: bool) -> InteractionResponse {
        use InteractionResponse::*;
        (if with_source { MessageWithSource } else { Message })(self)
    }
}