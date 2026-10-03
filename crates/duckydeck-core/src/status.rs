//! Status text of catalog actions (`text = { … }` in the catalog).
//!
//! The text replaces the key label, e.g. the current audio output on the
//! "switch output" key. It comes from a command (first stdout line, or a
//! top-level JSON string) or from built-in logic. It is read at startup,
//! after each press of the action and on the events named in `refresh`.
//! Nothing is polled.

use serde::Deserialize;

use crate::{CommandRunner, CommandSpec};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSource {
    /// Status command.
    #[serde(default)]
    pub command: Vec<String>,
    /// Top-level JSON string key in the command output; without it the
    /// first line of stdout.
    pub json: Option<String>,
    /// Built-in source instead of a command.
    pub builtin: Option<Builtin>,
    /// Events that re-read the text, besides startup and presses.
    #[serde(default)]
    pub refresh: Vec<Trigger>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Builtin {
    /// Short name of the default audio sink.
    AudioOutput,
}

/// Event that re-reads a status text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trigger {
    /// A sink, source or the server changed (`pactl subscribe`).
    Audio,
    /// An MPRIS player appeared, left or changed its playback status.
    Media,
}

impl TextSource {
    /// Exactly one of `command` and `builtin`; `json` needs a command.
    pub fn is_valid(&self) -> bool {
        self.builtin.is_some() != !self.command.is_empty()
            && (self.json.is_none() || !self.command.is_empty())
    }

    /// Text from a finished status command; `None` = nothing to show.
    pub fn from_command(&self, success: bool, stdout: &str) -> Option<String> {
        if !success {
            return None;
        }
        let text = match &self.json {
            None => stdout.lines().next()?.to_owned(),
            Some(key) => serde_json::from_str::<serde_json::Value>(stdout)
                .ok()?
                .get(key)?
                .as_str()?
                .to_owned(),
        };
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    /// Reads the current text.
    pub async fn read(&self, runner: &dyn CommandRunner) -> Option<String> {
        if let Some(Builtin::AudioOutput) = self.builtin {
            return audio_output(runner).await;
        }
        let (program, args) = self.command.split_first()?;
        let spec = CommandSpec::new(program.clone()).args(args.to_vec());
        match runner.run(&spec).await {
            Ok(out) => self.from_command(out.success(), &out.stdout),
            Err(e) => {
                tracing::debug!(command = ?self.command, error = %e, "status text failed");
                None
            }
        }
    }
}

/// Short name of the default sink: node nick (e.g. "USB-C Audio" or the
/// monitor name for HDMI), else the device description.
pub async fn audio_output(runner: &dyn CommandRunner) -> Option<String> {
    let out = runner
        .run(&CommandSpec::new("pactl").args(["get-default-sink"]))
        .await
        .ok()
        .filter(|o| o.success())?;
    let name = out.stdout.trim().to_owned();
    let list = runner
        .run(&CommandSpec::new("pactl").args(["-f", "json", "list", "sinks"]))
        .await
        .ok()
        .filter(|o| o.success())?;
    sink_name(&list.stdout, &name)
}

/// Short name of sink `name` in `pactl -f json list sinks` output.
pub fn sink_name(json: &str, name: &str) -> Option<String> {
    let sinks: serde_json::Value = serde_json::from_str(json).ok()?;
    let sink = sinks
        .as_array()?
        .iter()
        .find(|s| s.get("name").and_then(|n| n.as_str()) == Some(name))?;
    let props = sink.get("properties");
    ["node.nick", "device.description"]
        .iter()
        .filter_map(|k| props?.get(*k)?.as_str())
        .chain(sink.get("description").and_then(|d| d.as_str()))
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::RecordingRunner;

    fn src(toml: &str) -> TextSource {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn validity() {
        assert!(src("command = [\"x\"]").is_valid());
        assert!(src("builtin = \"audio-output\"\nrefresh = [\"audio\"]").is_valid());
        assert!(!src("builtin = \"audio-output\"\ncommand = [\"x\"]").is_valid());
        assert!(!src("json = \"k\"").is_valid());
        assert!(!src("builtin = \"audio-output\"\njson = \"k\"").is_valid());
    }

    #[test]
    fn command_text() {
        let line = src("command = [\"x\"]");
        assert_eq!(line.from_command(true, " 2/3 \nmore"), Some("2/3".into()));
        assert_eq!(line.from_command(true, ""), None);
        assert_eq!(line.from_command(false, "x"), None);
        let json = src("command = [\"x\"]\njson = \"identity\"");
        assert_eq!(
            json.from_command(true, r#"{"identity":"Spotify","playing":true}"#),
            Some("Spotify".into())
        );
        assert_eq!(json.from_command(true, r#"{"identity":""}"#), None);
        assert_eq!(json.from_command(true, "{garbled"), None);
    }

    const SINKS: &str = r#"[
      {"name":"usb","description":"USB-C Audio Analog Stereo",
       "properties":{"node.nick":"USB-C Audio","device.description":"USB-C Audio"}},
      {"name":"hdmi","description":"Radeon HD Audio Digital Stereo (HDMI) [PL3493WQ]",
       "properties":{"node.nick":"PL3493WQ"}},
      {"name":"bare","description":"Bare Sink","properties":{}}
    ]"#;

    #[test]
    fn sink_names() {
        assert_eq!(sink_name(SINKS, "usb"), Some("USB-C Audio".into()));
        assert_eq!(sink_name(SINKS, "hdmi"), Some("PL3493WQ".into()));
        assert_eq!(sink_name(SINKS, "bare"), Some("Bare Sink".into()));
        assert_eq!(sink_name(SINKS, "gone"), None);
        assert_eq!(sink_name("nope", "usb"), None);
    }

    #[tokio::test]
    async fn audio_output_asks_pactl() {
        let runner = RecordingRunner::new();
        assert_eq!(audio_output(&runner).await, None);
        assert_eq!(runner.command_lines()[0], "pactl get-default-sink");
    }
}
