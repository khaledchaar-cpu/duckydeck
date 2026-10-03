//! Toggle state of catalog actions (`state = { … }` in the catalog).
//!
//! A state comes from a file (exists = on, or a JSON bool inside it) that the
//! daemon watches with inotify, or from a command that runs at startup and
//! after each press of the action. Nothing is polled.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateSource {
    /// Watched file; `~/` is the home directory.
    pub file: Option<String>,
    /// Status command, run at startup and after a press.
    #[serde(default)]
    pub command: Vec<String>,
    /// Top-level JSON bool key in the file or command output. Without it a
    /// file counts as on when it exists, a command when it succeeds.
    pub json: Option<String>,
    /// While on, the key shows how long since the file was last modified.
    #[serde(default)]
    pub elapsed: bool,
    /// Events that re-run a status command, besides startup and presses.
    #[serde(default)]
    pub refresh: Vec<crate::status::Trigger>,
}

impl StateSource {
    /// Exactly one of `file` and `command`; `elapsed` needs a file.
    pub fn is_valid(&self) -> bool {
        self.file.is_some() != !self.command.is_empty() && (!self.elapsed || self.file.is_some())
    }

    pub fn path(&self, home: &Path) -> Option<PathBuf> {
        let f = self.file.as_deref()?;
        Some(match f.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(f),
        })
    }

    /// State from the file content, `None` if the file is missing.
    pub fn from_file(&self, content: Option<&str>) -> Option<bool> {
        match (&self.json, content) {
            (_, None) => Some(false),
            (None, Some(_)) => Some(true),
            (Some(key), Some(c)) => json_bool(c, key),
        }
    }

    /// State from a finished status command.
    pub fn from_command(&self, success: bool, stdout: &str) -> Option<bool> {
        match &self.json {
            None => Some(success),
            Some(key) if success => json_bool(stdout, key),
            Some(_) => None,
        }
    }
}

fn json_bool(text: &str, key: &str) -> Option<bool> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get(key)?
        .as_bool()
}

/// Known state of one toggle action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggle {
    pub on: bool,
    /// Start time for `elapsed` states that are on.
    pub since: Option<SystemTime>,
}

/// `m:ss`, or `h:mm:ss` from one hour on.
pub fn elapsed_text(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(toml: &str) -> StateSource {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn validity() {
        assert!(src("file = \"~/a\"").is_valid());
        assert!(src("command = [\"x\"]").is_valid());
        assert!(!src("file = \"a\"\ncommand = [\"x\"]").is_valid());
        assert!(!src("json = \"a\"").is_valid());
        assert!(!src("command = [\"x\"]\nelapsed = true").is_valid());
    }

    #[test]
    fn file_states() {
        let exists = src("file = \"~/s/x\"");
        assert_eq!(
            exists.path(Path::new("/home/u")),
            Some("/home/u/s/x".into())
        );
        assert_eq!(exists.from_file(Some("")), Some(true));
        assert_eq!(exists.from_file(None), Some(false));
        let json = src("file = \"/n.json\"\njson = \"dnd\"");
        assert_eq!(
            json.from_file(Some(r#"{"version":3,"dnd":true}"#)),
            Some(true)
        );
        assert_eq!(json.from_file(Some(r#"{"dnd":false}"#)), Some(false));
        assert_eq!(json.from_file(Some("{garbled")), None);
    }

    #[test]
    fn command_states() {
        let c = src("command = [\"x\"]\njson = \"enabled\"");
        assert_eq!(
            c.from_command(true, r#"{"enabled":true,"temperature":4000}"#),
            Some(true)
        );
        assert_eq!(c.from_command(false, ""), None);
        assert_eq!(
            src("command = [\"x\"]").from_command(false, ""),
            Some(false)
        );
    }

    #[test]
    fn elapsed_format() {
        assert_eq!(elapsed_text(Duration::from_secs(5)), "0:05");
        assert_eq!(elapsed_text(Duration::from_secs(754)), "12:34");
        assert_eq!(elapsed_text(Duration::from_secs(3723)), "1:02:03");
    }
}
