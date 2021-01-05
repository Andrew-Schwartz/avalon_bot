use std::fmt::{self, Display};
use std::ops::Index;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Rounds(pub usize);

impl Rounds {
    pub fn rounds(&self) -> [Round; 5] {
        MAP[self.0.saturating_sub(5)]
    }
}

impl Display for Rounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !(5..=10).contains(&self.0) { return Ok(()) }
        self.rounds().iter()
            .zip(1..=5)
            .map(|(round, i)| {
                write!(f,
                       "{}{} Round: {} on the quest.",
                       i,
                       match i {
                           1 => "st",
                           2 => "nd",
                           3 => "rd",
                           4 | 5 => "th",
                           _ => unreachable!(),
                       },
                       round.players
                )?;
                if round.fails != 1 {
                    write!(f, " {} fails are needed.", round.fails)?;
                }
                writeln!(f)
            })
            .collect()
    }
}

impl Index<usize> for Rounds {
    type Output = Round;

    // 1 indexed :)
    fn index(&self, index: usize) -> &Self::Output {
        &MAP[self.0.saturating_sub(5)][index - 1]
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct Round {
    pub players: usize,
    pub fails: usize,
}

macro_rules! r {
    ($players:expr, 2) => { Round { players: $players, fails: 2 } };
    ($players:expr) => { Round { players: $players, fails: 1 } };
}

// @formatter:off
const MAP: [[Round; 5]; 6] = [
    // /* 5  */ [r!(2), r!(2), r!(2), r!(2),    r!(2)],
    /* 5  */ [r!(2), r!(3), r!(2), r!(3),    r!(3)],
    /* 6  */ [r!(2), r!(3), r!(4), r!(3),    r!(4)],
    /* 7  */ [r!(2), r!(3), r!(3), r!(4, 2), r!(4)],
    /* 8  */ [r!(3), r!(4), r!(4), r!(5, 2), r!(5)],
    /* 9  */ [r!(3), r!(4), r!(4), r!(5, 2), r!(5)],
    /* 10 */ [r!(3), r!(4), r!(4), r!(5, 2), r!(5)],
];
// @formatter:on
