//! Shared library for the DuckyDeck daemon and CLI.

pub mod catalog;
pub mod command;
pub mod config;
pub mod dial;
pub mod font;
pub mod hypr;
pub mod icons;
pub mod ipc;
pub mod media;
pub mod nav;

pub use command::{
    CommandError, CommandOutput, CommandRunner, CommandSpec, RecordingRunner, TokioRunner,
};
pub mod render;
pub mod theme;
pub mod toggle;
