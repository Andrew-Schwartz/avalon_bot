#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use command_data_derive::*;
    use discorsd::BotState;

    struct TestBot;

    macro_rules! make_slash_command {
        ($data:ty) => {
            #[derive(Debug, Clone)]
            struct Perms;
            #[discorsd::async_trait]
            impl discorsd::commands::SlashCommand for Perms {
                type Bot = TestBot;
                type Data = $data;
                type Use = discorsd::commands::Used;
                const NAME: &'static str = "permissions";

                fn description(&self) -> std::borrow::Cow<'static, str> {
                    "Get or edit permissions for a user or a role".into()
                }

                async fn run(
                    &self,
                    _: std::sync::Arc<discorsd::BotState<TestBot>>,
                    interaction: discorsd::commands::InteractionUse<discorsd::commands::Unused>,
                    data: Self::Data
                ) -> Result<discorsd::commands::InteractionUse<discorsd::commands::Used>, discorsd::errors::BotError> {
                    // Ok to let the test know we succeeded
                    println!("data = {:?}", data);
                    Ok(interaction.into())
                }
            }
        };
    }

    fn assert_same_json_value(correct: &str, modeled: impl discorsd::commands::SlashCommandRaw) {
        use serde_json::Value;

        let correct: Value = serde_json::from_str(correct).unwrap();
        let modeled = serde_json::to_string_pretty(&modeled.command()).unwrap();
        // println!("modeled = {}", modeled);
        let modeled: Value = serde_json::from_str(&modeled).unwrap();

        assert_eq!(correct, modeled);
    }

    async fn assert_perms_parsing<C: discorsd::commands::SlashCommandRaw<Bot=TestBot>>(command: &C) {
        async fn test_run_data<C: discorsd::commands::SlashCommandRaw<Bot=TestBot>>(command: &C, data: discorsd::commands::ApplicationCommandInteractionData) {
            use std::convert::TryInto;
            use chrono::Utc;
            use discorsd::model::ids::*;
            use discorsd::commands::*;

            let data = data.clone().try_into().unwrap();
            let interaction = Interaction {
                id: InteractionId(1234),
                kind: InteractionType::ApplicationCommand,
                data,
                source: InteractionSource::Guild(GuildSource {
                    id: GuildId(1234),
                    member: discorsd::model::guild::GuildMember {
                        user: discorsd::model::user::User {
                            id: UserId(1234),
                            username: "".to_string(),
                            discriminator: "".to_string(),
                            avatar: None,
                            bot: None,
                            system: None,
                            mfa_enabled: None,
                            locale: None,
                            verified: None,
                            email: None,
                            flags: None,
                            premium_type: None,
                            public_flags: None,
                        },
                        nick: None,
                        roles: Default::default(),
                        joined_at: Utc::now(),
                        premium_since: None,
                        deaf: false,
                        mute: false,
                        pending: false,
                    },
                }),
                channel_id: ChannelId(1234),
                token: "".to_string(),
            };
            let (iu, data) = InteractionUse::from(interaction);
            let state = BotState::testing_state(TestBot);
            command.run(state, iu, data).await.unwrap();
        }

        use discorsd::commands::{
            ApplicationCommandInteractionData as ACID,
            ApplicationCommandInteractionDataOption as ACIDO,
            ApplicationCommandInteractionDataValue as ACIDV,
            OptionValue,
        };
        use discorsd::model::ids::*;
        let permissions_user_get_user = ACID {
            id: CommandId(1234),
            name: "permissions".to_string(),
            options: vec![
                ACIDO {
                    name: "user".to_string(),
                    value: ACIDV::Options {
                        options: vec![
                            ACIDO {
                                name: "get".to_string(),
                                value: ACIDV::Options {
                                    options: vec![
                                        ACIDO {
                                            name: "user".to_string(),
                                            value: ACIDV::Value { value: OptionValue::String("1234".into()) },
                                        }
                                    ]
                                },
                            }
                        ]
                    },
                }],
            resolved: None,
        };
        test_run_data(command, permissions_user_get_user).await;
        let permissions_user_get_user_channel = ACID {
            id: CommandId(1234),
            name: "permissions".to_string(),
            options: vec![
                ACIDO {
                    name: "user".to_string(),
                    value: ACIDV::Options {
                        options: vec![
                            ACIDO {
                                name: "get".to_string(),
                                value: ACIDV::Options {
                                    options: vec![
                                        ACIDO {
                                            name: "user".to_string(),
                                            value: ACIDV::Value { value: OptionValue::String("1234".into()) },
                                        },
                                        ACIDO {
                                            name: "channel".to_string(),
                                            value: ACIDV::Value { value: OptionValue::String("4321".into()) },
                                        }
                                    ]
                                },
                            }
                        ]
                    },
                }],
            resolved: None,
        };
        test_run_data(command, permissions_user_get_user_channel).await;
    }


    const CORRECT4: &'static str = r#"{
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

    #[tokio::test]
    async fn part4() {
        assert_same_json_value(CORRECT4, Perms);
        make_slash_command!(Data);
        #[derive(CommandData, Debug)]
        enum Data {
            #[command(desc = "Get or edit permissions for a user")]
            User(GetEditUser),
            #[command(desc = "Get or edit permissions for a role")]
            Role(GetEditRole),
        }
        #[derive(CommandData, Debug)]
        enum GetEditUser {
            #[command(desc = "Get permissions for a user")]
            Get {
                #[command(desc = "The user to get")]
                user: discorsd::model::ids::UserId,
                #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
                channel: Option<discorsd::model::ids::ChannelId>,
            },
            #[command(desc = "Edit permissions for a user")]
            Edit {
                #[command(desc = "The user to edit")]
                user: discorsd::model::ids::UserId,
                #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
                channel: Option<discorsd::model::ids::ChannelId>,
            },
        }
        #[derive(CommandData, Debug)]
        enum GetEditRole {
            #[command(desc = "Get permissions for a role")]
            Get(GetRole),
            #[command(desc = "Edit permissions for a role")]
            Edit(EditRole),
        }
        #[derive(CommandData, Debug)]
        struct GetRole {
            #[command(desc = "The role to get")]
            pub role: discorsd::model::ids::RoleId,
            #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
            pub channel: Option<discorsd::model::ids::ChannelId>,
        }
        #[derive(CommandData, Debug)]
        struct EditRole {
            #[command(desc = "The role to edit")]
            pub role: discorsd::model::ids::RoleId,
            #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
            pub channel: Option<discorsd::model::ids::ChannelId>,
        }

        assert_perms_parsing(&Perms).await;
    }

    #[test]
    fn generic() {
        const CORRECT: &str = r#"{
  "name": "permissions",
  "description": "Get or edit permissions for a user or a role",
  "options": [
    {
      "type": 1,
      "name": "role",
      "description": "role",
      "options": [
        {
          "type": 8,
          "name": "role",
          "description": "The role to get",
          "required": true
        },
        {
          "type": 7,
          "name": "channel",
          "description": "The channel permissions to get. If omitted, the guild permissions will be returned"
        }
      ]
    },
    {
      "type": 1,
      "name": "user",
      "description": "user",
      "options": [
        {
          "type": 6,
          "name": "user",
          "description": "The user to get",
          "required": true
        },
        {
          "type": 7,
          "name": "channel",
          "description": "The channel permissions to get. If omitted, the guild permissions will be returned"
        }
      ]
    }
  ]
}"#;

        use discorsd::model::ids::*;

        make_slash_command!(Data);

        // todo make a good error if this is a struct or an enum with inline structs
        #[derive(CommandData, Debug)]
        #[command(command = "Perms")]
        enum Data {
            Role(IdInChannel<RoleId>),
            User(IdInChannel<UserId>),
        }

        #[derive(CommandData, Debug)]
        #[command(command = "Perms")]
        struct IdInChannel<I: Id> {
            #[command(rename = "<I>", desc = "The <I> to get")]
            id: I,
            #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
            channel: Option<ChannelId>,
        }

        assert_same_json_value(CORRECT, Perms);
    }

    #[tokio::test]
    async fn part4_generic() {
        use discorsd::model::ids::{Id, RoleId, UserId};

        assert_same_json_value(CORRECT4, Perms);
        make_slash_command!(Data);

        #[derive(CommandData, Debug)]
        enum Data {
            #[command(desc = "Get or edit permissions for a user")]
            User(GetEdit<UserId>),
            #[command(desc = "Get or edit permissions for a role")]
            Role(GetEdit<RoleId>),
        }
        #[derive(CommandData, Debug)]
        enum GetEdit<I: Id> {
            #[command(desc = "Get permissions for a <I>")]
            Get(Get<I>),
            #[command(desc = "Edit permissions for a <I>")]
            Edit(Edit<I>),
        }
        #[derive(CommandData, Debug)]
        struct Get<I: Id> {
            #[command(rename = "<I>", desc = "The <I> to get")]
            pub id: I,
            #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
            pub channel: Option<discorsd::model::ids::ChannelId>,
        }
        #[derive(CommandData, Debug)]
        struct Edit<I: Id> {
            #[command(rename = "<I>", desc = "The <I> to edit")]
            pub id: I,
            #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
            pub channel: Option<discorsd::model::ids::ChannelId>,
        }

        assert_perms_parsing(&Perms).await;
    }
}