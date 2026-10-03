//! Shared library for the DuckyDeck daemon and CLI.

pub mod command;
pub mod font;

pub use command::{
    CommandError, CommandOutput, CommandRunner, CommandSpec, RecordingRunner, TokioRunner,
};
pub mod render;
pub mod theme;
