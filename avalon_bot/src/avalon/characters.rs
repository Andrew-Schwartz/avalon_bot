use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use once_cell::sync::Lazy;
use strum::{EnumCount, EnumIter, EnumString};

use Character::*;
use discorsd::http::model::Color;

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, EnumIter, EnumString, EnumCount)]
pub enum Character {
    LoyalServant,
    MinionOfMordred,
    Assassin,
    Merlin,
    Mordred,
    Morgana,
    Oberon,
    Percival,
}

impl Character {
    pub const fn name(&self) -> &'static str {
        match self {
            LoyalServant => "Loyal Servant",
            MinionOfMordred => "Minion of Mordred",
            Assassin => "Assassin",
            Merlin => "Merlin",
            Mordred => "Mordred",
            Morgana => "Morgana",
            Oberon => "Oberon",
            Percival => "Percival",
        }
    }

    pub const fn loyalty(&self) -> Loyalty {
        match self {
            LoyalServant | Merlin | Percival => Loyalty::Good,
            MinionOfMordred | Assassin | Mordred | Morgana | Oberon => Loyalty::Evil,
        }
    }

    pub const fn abilities(&self) -> &'static str {
        match self {
            LoyalServant => "Sees no one.",
            MinionOfMordred => "Sees other minions of Mordred.",
            Assassin => "Sees other minions of Mordred. \
                         \nAttempts to assassinate Merlin at the end of the game.",
            Merlin => "Sees the Assassin, Morgana, and Minions of Mordred.",
            Mordred => "Sees his minions. Not seen my Merlin.",
            Morgana => "Sees other minions of Mordred. Appears as Merlin to Percival.",
            Oberon => "Sees no one, seen by no one.",
            Percival => "Sees Merlin and Morgana but doesn't know who is who.",
        }
    }

    pub fn sees(&self) -> &HashSet<Character> {
        SEES.get(self).unwrap()
    }

    pub fn image(&self) -> PathBuf {
        PathBuf::from(format!("images/avalon/characters/{}.jpg", self.name()))
    }
}

impl fmt::Display for Character {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Loyalty { Good, Evil }

impl Loyalty {
    pub const fn color(&self) -> Color {
        match self {
            Loyalty::Good => Color::BLUE,
            Loyalty::Evil => Color::RED,
        }
    }

    pub fn image(&self) -> PathBuf {
        PathBuf::from(format!("images/avalon/loyalty{}.jpg", self))
    }
}

impl fmt::Display for Loyalty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Loyalty::Good => "Good",
            Loyalty::Evil => "Evil",
        })
    }
}


static SEES: Lazy<HashMap<Character, HashSet<Character>>> = Lazy::new(|| map! {
    LoyalServant => set!(),
    MinionOfMordred => set!(Assassin, MinionOfMordred, Mordred, Morgana),
    Assassin => set!(MinionOfMordred, Morgana, Mordred),
    Merlin => set!(Assassin, MinionOfMordred, Morgana),
    Mordred => set!(Assassin, MinionOfMordred, Morgana),
    Morgana => set!(Assassin, MinionOfMordred, Mordred),
    Oberon => set!(),
    Percival => set!(Merlin, Morgana),
});