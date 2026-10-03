//! Shared library for the DuckyDeck daemon and CLI.

pub mod app_audio;
pub mod appicon;
pub mod apps;
pub mod catalog;
pub mod check;
pub mod command;
pub mod compound;
pub mod config;
pub mod context;
pub mod custom_icons;
pub mod dial;
pub mod edit;
pub mod font;
pub mod hypr;
pub mod icon_libraries;
pub mod icons;
pub mod ipc;
pub mod library;
pub mod media;
pub mod nav;

pub use command::{
    CommandError, CommandOutput, CommandRunner, CommandSpec, RecordingRunner, TokioRunner,
};
pub mod render;
pub mod setup;
pub mod status;
pub mod theme;
pub mod toggle;
