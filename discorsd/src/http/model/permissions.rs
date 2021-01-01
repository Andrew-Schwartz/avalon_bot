use serde::{Deserialize, Serialize};

use crate::http::model::ids::RoleId;
use crate::http::model::message::Color;
use crate::http::model::Id;

/// Roles represent a set of permissions attached to a group of users. Roles have unique names,
/// colors, and can be "pinned" to the side bar, causing their members to be listed separately.
/// Roles are unique per guild, and can have separate permission profiles for the global context
/// (guild) and channel context. The `@everyone` role has the same ID as the guild it belongs to.
/// Roles without colors (`color == 0`) do not count towards the final computed color in the user
/// list.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Role {
    /// role id
    pub id: RoleId,
    /// role name
    pub name: String,
    /// integer representation of hexadecimal color code
    pub color: Color,
    /// if this role is pinned in the user listing
    pub hoist: bool,
    /// position of this role
    pub position: u32,
    /// permission bit set
    // todo deserialize as u64
    pub permissions: String,
    /// whether this role is managed by an integration
    pub managed: bool,
    /// whether this role is mentionable
    pub mentionable: bool,
}

id_eq!(Role);
impl Id for Role {
    type Id = RoleId;

    fn id(&self) -> Self::Id {
        self.id
    }
}