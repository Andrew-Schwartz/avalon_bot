#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use command_data_derive::*;

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
                    _: discorsd::commands::InteractionUse<discorsd::commands::Unused>,
                    _: Self::Data
                ) -> Result<discorsd::commands::InteractionUse<discorsd::commands::Used>, discorsd::errors::BotError> {
                    unimplemented!()
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
        #[derive(CommandData)]
        enum GetEditRole {
            #[command(desc = "Get permissions for a role")]
            Get(GetRole),
            #[command(desc = "Edit permissions for a role")]
            Edit(EditRole),
        }
        #[derive(CommandData)]
        struct GetRole {
            #[command(desc = "The role to get")]
            pub role: discorsd::model::ids::RoleId,
            #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
            pub channel: Option<discorsd::model::ids::ChannelId>,
        }
        #[derive(CommandData)]
        struct EditRole {
            #[command(desc = "The role to edit")]
            pub role: discorsd::model::ids::RoleId,
            #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
            pub channel: Option<discorsd::model::ids::ChannelId>,
        }
        // let command = <Data as discorsd::commands::CommandData<Perms>>::make_args(&Perms);
        // println!("command = {:#?}", command);
    }

    // todo handle generics?
//     const CORRECT4GENERIC: &'static str = r#"{
//     "name": "permissions",
//     "description": "Get or edit permissions for a user or a role",
//     "options": [
//         {
//             "name": "user",
//             "description": "Get or edit permissions for a user",
//             "type": 2,
//             "options": [
//                 {
//                     "name": "get",
//                     "description": "Get permissions for a user",
//                     "type": 1,
//                     "options": [
//                         {
//                             "name": "user",
//                             "description": "The id to get",
//                             "type": 6,
//                             "required": true
//                         },
//                         {
//                             "name": "channel",
//                             "description": "The channel permissions to get. If omitted, the guild permissions will be returned",
//                             "type": 7
//                         }
//                     ]
//                 },
//                 {
//                     "name": "edit",
//                     "description": "Edit permissions for a user",
//                     "type": 1,
//                     "options": [
//                         {
//                             "name": "user",
//                             "description": "The id to edit",
//                             "type": 6,
//                             "required": true
//                         },
//                         {
//                             "name": "channel",
//                             "description": "The channel permissions to edit. If omitted, the guild permissions will be edited",
//                             "type": 7
//                         }
//                     ]
//                 }
//             ]
//         },
//         {
//             "name": "role",
//             "description": "Get or edit permissions for a role",
//             "type": 2,
//             "options": [
//                 {
//                     "name": "get",
//                     "description": "Get permissions for a role",
//                     "type": 1,
//                     "options": [
//                         {
//                             "name": "role",
//                             "description": "The id to get",
//                             "type": 8,
//                             "required": true
//                         },
//                         {
//                             "name": "channel",
//                             "description": "The channel permissions to get. If omitted, the guild permissions will be returned",
//                             "type": 7
//                         }
//                     ]
//                 },
//                 {
//                     "name": "edit",
//                     "description": "Edit permissions for a role",
//                     "type": 1,
//                     "options": [
//                         {
//                             "name": "role",
//                             "description": "The id to edit",
//                             "type": 8,
//                             "required": true
//                         },
//                         {
//                             "name": "channel",
//                             "description": "The channel permissions to edit. If omitted, the guild permissions will be edited",
//                             "type": 7
//                         }
//                     ]
//                 }
//             ]
//         }
//     ]
// }"#;
//
//     #[test]
//     fn part4_generic() {
//         use discorsd::model::ids::{Id, RoleId, UserId};
//
//         assert_same_json_value(CORRECT4, Perms);
//         make_slash_command!(Data);
//         #[derive(CommandData)]
//         enum Data {
//             #[command(desc = "Get or edit permissions for a user")]
//             User(GetEditUser),
//             #[command(desc = "Get or edit permissions for a role")]
//             Role(GetEditRole),
//         }
//         #[derive(CommandData)]
//         enum GetEditUser {
//             #[command(desc = "Get permissions for a user")]
//             Get(Get<UserId>),
//             #[command(desc = "Edit permissions for a user")]
//             Edit(Edit<UserId>),
//         }
//         #[derive(CommandData)]
//         enum GetEditRole {
//             #[command(desc = "Get permissions for a role")]
//             Get(Get<RoleId>),
//             #[command(desc = "Edit permissions for a role")]
//             Edit(Edit<RoleId>),
//         }
//         #[derive(CommandData)]
//         struct Get<I: Id> {
//             #[command(desc = "The id to get")]
//             pub role: I,
//             #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
//             pub channel: Option<discorsd::model::ids::ChannelId>,
//         }
//         #[derive(CommandData)]
//         struct Edit<I: Id> {
//             #[command(desc = "The role to edit")]
//             pub role: I,
//             #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
//             pub channel: Option<discorsd::model::ids::ChannelId>,
//         }
//         let command = <Data as discorsd::commands::CommandData<Perms>>::make_args(&Perms);
//         println!("command = {:#?}", command);
//     }
}
