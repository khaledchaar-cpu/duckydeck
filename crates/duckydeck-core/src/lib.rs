//! Shared library for the DuckyDeck daemon and CLI.

pub mod command;

pub use command::{
    CommandError, CommandOutput, CommandRunner, CommandSpec, RecordingRunner, TokioRunner,
};
