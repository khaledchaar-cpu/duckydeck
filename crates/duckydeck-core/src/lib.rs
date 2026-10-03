//! Shared library for the DuckyDeck daemon and CLI.

pub mod catalog;
pub mod command;
pub mod config;
pub mod font;
pub mod icons;
pub mod nav;

pub use command::{
    CommandError, CommandOutput, CommandRunner, CommandSpec, RecordingRunner, TokioRunner,
};
pub mod render;
pub mod theme;
