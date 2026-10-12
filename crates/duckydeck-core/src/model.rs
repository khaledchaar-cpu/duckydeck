//! Supported Stream Deck models and their layout. Profiles name the model
//! they are made for (`device = "plus_xl"`); everything else derives the
//! number of keys, dials and the strip width from here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    /// Stream Deck +: 8 keys, 4 dials, 800 px strip.
    #[default]
    Plus,
    /// Stream Deck + XL: 36 keys, 6 dials, 1200 px strip.
    PlusXl,
}

/// One strip segment per dial.
pub const SEGMENT_W: u32 = 200;

impl Model {
    pub const ALL: [Self; 2] = [Self::Plus, Self::PlusXl];

    pub const fn keys(self) -> usize {
        match self {
            Self::Plus => 8,
            Self::PlusXl => 36,
        }
    }

    /// Keys per row.
    pub const fn columns(self) -> usize {
        match self {
            Self::Plus => 4,
            Self::PlusXl => 9,
        }
    }

    pub const fn dials(self) -> usize {
        match self {
            Self::Plus => 4,
            Self::PlusXl => 6,
        }
    }

    pub const fn strip_width(self) -> u32 {
        self.dials() as u32 * SEGMENT_W
    }

    /// Value of `device` in a profile.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Plus => "plus",
            Self::PlusXl => "plus_xl",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Plus => "Stream Deck +",
            Self::PlusXl => "Stream Deck + XL",
        }
    }

    /// Id of the built-in default profile for this model.
    pub const fn default_profile(self) -> &'static str {
        match self {
            Self::Plus => "omarchy",
            Self::PlusXl => "omarchy-xl",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        assert_eq!(Model::Plus.strip_width(), 800);
        assert_eq!(Model::PlusXl.strip_width(), 1200);
        for m in Model::ALL {
            assert_eq!(m.keys() % m.columns(), 0);
            assert_eq!(Model::from_id(m.id()), Some(m));
        }
    }
}
