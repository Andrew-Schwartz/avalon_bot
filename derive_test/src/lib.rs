#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use command_data_derive::*;
    use discorsd::async_trait;
    use discorsd::BotState;
    use discorsd::commands::*;
    use discorsd::errors::BotError;
    use discorsd::model::ids::{ChannelId, RoleId, UserId};

    struct TestBot;

    macro_rules! make_slash_command {
        ($data:ty) => {
            #[derive(Debug, Clone)]
            struct Perms;
            #[async_trait]
            impl SlashCommandData for Perms {
                type Bot = TestBot;
                type Data = $data;
                type Use = Used;
                const NAME: &'static str = "permissions";

                fn description(&self) -> Cow<'static, str> {
                    "Get or edit permissions for a user or a role".into()
                }

                async fn run(&self, _: Arc<BotState<TestBot>>, _: InteractionUse<Unused>, _: Self::Data) -> Result<InteractionUse<Used>, BotError> {
                    unimplemented!()
                }
            }
        };
    }

    fn assert_same_json_value(correct: &str, modeled: impl SlashCommand) {
        use serde_json::Value;

        let correct: Value = serde_json::from_str(correct).unwrap();
        let modeled = serde_json::to_string_pretty(&modeled.command()).unwrap();
        println!("modeled = {}", modeled);
        let modeled: Value = serde_json::from_str(&modeled).unwrap();

        assert_eq!(correct, modeled);
    }

    const CORRECT1: &'static str = r#"{
    "name": "permissions",
    "description": "Get or edit permissions for a user or a role",
    "options": []
}"#;

    #[test]
    fn part1() {
        make_slash_command!(());
        assert_same_json_value(CORRECT1, Perms);
        let command = <()>::make_args(&Perms);
        println!("command = {:?}", command);
    }

    const CORRECT2: &'static str = r#"{
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

    // #[test]
    // fn part2_derive() {
    //     make_slash_command!(Data);
    //     // #[derive(CommandData)]
    //     // #[command(rename_all = "lowercase")]
    //     enum Data {
    //         // #[command(desc = "Get or edit permissions for a user")]
    //         User,
    //         // #[command(desc = "Get or edit permissions for a role")]
    //         Role,
    //     }
    //     // impl TryFrom
    //     impl CommandArgs<Perms> for Data {
    //         fn args(command: &Perms) -> TopLevelOption {
    //             TopLevelOption::Groups(vec![
    //                 SubCommandGroup {
    //                     name: "user",
    //                     description: "Get or edit permissions for a user",
    //                     sub_commands: vec![]
    //                 },
    //                 // role
    //             ])
    //         }
    //     }
    //     assert_same_json_value(CORRECT2, Perms.command());
    // }

    const CORRECT3: &'static str = r#"{
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

    // #[test]
    // fn part3() {
    //     make_slash_command!(Data);
    //     // #[derive(CommandData)]
    //     enum Data {
    //         // #[command(desc = "Get or edit permissions for a user")]
    //         User(GetEditUser),
    //         // #[command(desc = "Get or edit permissions for a role")]
    //         Role(GetEditRole),
    //     }
    //     // #[derive(CommandData)]
    //     enum GetEditUser {
    //         // #[command(desc = "Get permissions for a user")]
    //         Get,
    //         // #[command(desc = "Edit permissions for a user")]
    //         Edit,
    //     }
    //     // #[derive(CommandData)]
    //     enum GetEditRole {
    //         // #[command(desc = "Get permissions for a role")]
    //         Get,
    //         // #[command(desc = "Edit permissions for a role")]
    //         Edit,
    //     }
    //     impl CommandArgs<Perms> for Data {
    //         fn args(command: &Perms) -> TopLevelOption {
    //             TopLevelOption::Groups(vec![
    //                 SubCommandGroup {
    //                     name: "user",
    //                     description: "Get or edit permissions for a user",
    //                     sub_commands: vec![
    //                         SubCommand {
    //                             name: "get",
    //                             description: "Get permissions for a user",
    //                             options: vec![]
    //                         },
    //                         // edit
    //                     ]
    //                 },
    //                 // role
    //             ])
    //         }
    //     }
    // }

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

    #[test]
    fn part4() {
        assert_same_json_value(CORRECT4, Perms);
        make_slash_command!(Data);
        #[derive(CommandData)]
        enum Data {
            #[command(desc = "Get or edit permissions for a user")]
            User(GetEditUser),
            #[command(desc = "Get or edit permissions for a role")]
            Role(GetEditRole),
        }
        #[derive(CommandData)]
        enum GetEditUser {
            #[command(desc = "Get permissions for a user")]
            Get {
                #[command(desc = "The user to get")]
                user: UserId,
                #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
                channel: Option<ChannelId>,
            },
            #[command(desc = "Edit permissions for a user")]
            Edit {
                #[command(desc = "The user to edit")]
                user: UserId,
                #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
                channel: Option<ChannelId>,
            },
        }
        #[derive(CommandData)]
        enum GetEditRole {
            #[command(desc = "Get permissions for a role")]
            Get(GetRole),
            #[command(desc = "Edit permissions for a role")]
            Edit(EditRole),
        }
        #[derive(CommandData)]
        struct GetRole {
            #[command(desc = "The role to get", required)]
            pub role: RoleId,
            #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
            pub channel: Option<ChannelId>,
        }
        #[derive(CommandData)]
        struct EditRole {
            #[command(desc = "The role to edit", required)]
            pub role: RoleId,
            #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
            pub channel: Option<ChannelId>,
        }
        let command = Data::make_args(&Perms);
        println!("command = {:#?}", command);
    }
}
