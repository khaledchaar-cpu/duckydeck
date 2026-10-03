//! Per-app volume: playback streams (PipeWire sink inputs via `pactl`),
//! grouped into apps, and the commands that change them.
//!
//! The dial picks one of the apps that are playing (pressed turn) and sets
//! its streams. Omarchy has no route for app volume, so the dial uses
//! `pactl` directly and shows the Omarchy OSD itself.

use std::collections::HashMap;

use serde::Deserialize;

use crate::command::{CommandRunner, CommandSpec};

/// One playback stream of an app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
    pub index: u32,
    /// `application.name`, e.g. "Spotify", "Google Chrome".
    pub name: Option<String>,
    /// `application.process.binary`, e.g. "spotify" (from the client when
    /// the stream lacks it).
    pub binary: Option<String>,
    /// `application.icon_name` and `application.id`.
    pub ids: Vec<String>,
    /// First channel in percent (over-amplified = 100).
    pub volume: u8,
    pub muted: bool,
}

impl Stream {
    /// Name shown for the app: application name, else binary. ALSA
    /// clients are named "PipeWire ALSA [cliamp]" – shown as "cliamp".
    pub fn label(&self) -> Option<&str> {
        let alsa = self.name.as_deref().and_then(|n| {
            n.strip_prefix("PipeWire ALSA [")
                .and_then(|n| n.strip_suffix(']'))
        });
        alsa.or(self.name.as_deref()).or(self.binary.as_deref())
    }

    /// Match of `app` (a label or a configured name) against label,
    /// application name, binary, icon name or id, ignoring case and
    /// treating spaces as dashes ("Google Chrome" = "google-chrome").
    pub fn is_app(&self, app: &str) -> bool {
        let app = normalize(app);
        [self.label(), self.name.as_deref(), self.binary.as_deref()]
            .into_iter()
            .flatten()
            .chain(self.ids.iter().map(String::as_str))
            .any(|n| normalize(n) == app)
    }
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase().replace(' ', "-")
}

/// Streams of `app`.
pub fn matching<'a>(streams: &'a [Stream], app: &str) -> Vec<&'a Stream> {
    streams.iter().filter(|s| s.is_app(app)).collect()
}

/// Apps that are playing, one label each, in stream order.
pub fn apps(streams: &[Stream]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in streams {
        if let Some(l) = s.label()
            && !out.iter().any(|o| normalize(o) == normalize(l))
        {
            out.push(l.to_owned());
        }
    }
    out
}

/// The app `delta` steps from `current` (cyclic); from the first app when
/// `current` is not playing. `None` when nothing plays.
pub fn select(apps: &[String], current: Option<&str>, delta: i32) -> Option<String> {
    let n = i32::try_from(apps.len()).ok().filter(|n| *n > 0)?;
    let pos = current.and_then(|c| apps.iter().position(|a| normalize(a) == normalize(c)));
    let next = match pos {
        // Bounded by `apps.len()`.
        Some(p) => (p as i32 + delta).rem_euclid(n),
        None => 0,
    };
    apps.get(next as usize).cloned()
}

/// Shown level of an app: first stream's volume, muted when all are.
pub fn level(streams: &[&Stream]) -> (Option<u8>, bool) {
    let volume = streams.first().map(|s| s.volume);
    let muted = !streams.is_empty() && streams.iter().all(|s| s.muted);
    (volume, muted)
}
/// Commands that move all streams of an app by `delta` detents. All end
/// on the same level (first stream + delta), clamped to 0–100 %; muted
/// streams are unmuted (like `omarchy audio output volume`).
pub fn twist(streams: &[&Stream], step: u8, delta: i32) -> Vec<CommandSpec> {
    let Some(first) = streams.first() else {
        return Vec::new();
    };
    if delta == 0 {
        return Vec::new();
    }
    let target = (i32::from(first.volume) + i32::from(step) * delta).clamp(0, 100);
    let unmute = streams
        .iter()
        .filter(|s| s.muted)
        .map(|s| pactl("set-sink-input-mute", s.index, "0".into()));
    unmute
        .chain(
            streams
                .iter()
                .map(|s| pactl("set-sink-input-volume", s.index, format!("{target}%"))),
        )
        .collect()
}

/// `omarchy osd` for an app's new level.
pub fn osd(app: &str, volume: u8, muted: bool) -> CommandSpec {
    let icon = if muted || volume == 0 {
        "volume-muted"
    } else {
        "volume-high"
    };
    CommandSpec::omarchy([
        "osd".to_owned(),
        "-i".into(),
        icon.into(),
        "-m".into(),
        app.to_owned(),
        "-p".into(),
        volume.to_string(),
    ])
}

/// Commands that toggle mute: unmute all if all are muted, else mute all.
pub fn press(streams: &[&Stream]) -> Vec<CommandSpec> {
    let (_, muted) = level(streams);
    let to = if muted { "0" } else { "1" };
    streams
        .iter()
        .map(|s| pactl("set-sink-input-mute", s.index, to.into()))
        .collect()
}

fn pactl(cmd: &str, index: u32, value: String) -> CommandSpec {
    CommandSpec::new("pactl").args([cmd.to_owned(), index.to_string(), value])
}

/// Reads all playback streams; empty on error.
pub async fn read_streams(runner: &dyn CommandRunner) -> Vec<Stream> {
    let list = |what: &str| CommandSpec::new("pactl").args(["-f", "json", "list", what]);
    let read = |spec: CommandSpec| async move {
        match runner.run(&spec).await {
            Ok(out) if out.success() => Some(out.stdout),
            res => {
                tracing::debug!(args = ?spec.args, result = ?res.map(|o| o.status), "reading streams failed");
                None
            }
        }
    };
    let Some(inputs) = read(list("sink-inputs")).await else {
        return Vec::new();
    };
    let mut streams = parse_sink_inputs(&inputs);
    if streams.iter().any(|s| s.binary.is_none()) {
        let clients = read(list("clients")).await.unwrap_or_default();
        with_clients(&mut streams, &inputs, &clients);
    }
    streams
}

/// Fills the binary from the streams' clients where the stream itself
/// lacks it (ALSA and some native PipeWire apps).
pub fn with_clients(streams: &mut [Stream], inputs: &str, clients: &str) {
    #[derive(Deserialize)]
    struct Raw {
        index: u32,
        #[serde(default)]
        client: serde_json::Value,
        #[serde(default)]
        properties: serde_json::Map<String, serde_json::Value>,
    }
    let parse = |json: &str| serde_json::from_str::<Vec<Raw>>(json).unwrap_or_default();
    let clients: HashMap<u32, Raw> = parse(clients).into_iter().map(|c| (c.index, c)).collect();
    let owner: HashMap<u32, u32> = parse(inputs)
        .into_iter()
        .filter_map(|i| Some((i.index, as_u32(&i.client)?)))
        .collect();
    for s in streams.iter_mut().filter(|s| s.binary.is_none()) {
        s.binary = owner
            .get(&s.index)
            .and_then(|i| clients.get(i))
            .and_then(|c| c.properties.get("application.process.binary")?.as_str())
            .filter(|b| !b.is_empty())
            .map(str::to_owned);
    }
}

/// `pactl` writes ids as numbers or strings.
fn as_u32(v: &serde_json::Value) -> Option<u32> {
    match v {
        serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

#[derive(Deserialize)]
struct RawInput {
    index: u32,
    #[serde(default)]
    mute: bool,
    #[serde(default)]
    volume: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    properties: serde_json::Map<String, serde_json::Value>,
}

/// Parses `pactl -f json list sink-inputs`.
pub fn parse_sink_inputs(json: &str) -> Vec<Stream> {
    let Ok(raw) = serde_json::from_str::<Vec<RawInput>>(json) else {
        return Vec::new();
    };
    raw.into_iter()
        .map(|r| {
            let prop = |k: &str| {
                r.properties
                    .get(k)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            };
            let volume = r
                .volume
                .values()
                .next()
                .and_then(|c| c.get("value_percent")?.as_str())
                .and_then(|p| p.trim_end_matches('%').trim().parse::<u32>().ok())
                .map_or(0, |v| v.min(100) as u8);
            Stream {
                index: r.index,
                name: prop("application.name"),
                binary: prop("application.process.binary"),
                ids: ["application.icon_name", "application.id"]
                    .into_iter()
                    .filter_map(prop)
                    .collect(),
                volume,
                muted: r.mute,
            }
        })
        .collect()
}

/// Whether a `pactl subscribe` line can change playback streams.
pub fn is_stream_event(line: &str) -> bool {
    line.starts_with("Event '") && line.contains(" on sink-input #")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutput, RecordingRunner};

    const JSON: &str = r#"[
      {"index": 12, "mute": false,
       "volume": {"front-left": {"value": 45875, "value_percent": "70%"},
                  "front-right": {"value": 45875, "value_percent": "70%"}},
       "properties": {"application.name": "Spotify", "application.process.binary": "spotify"}},
      {"index": 13, "mute": true,
       "volume": {"mono": {"value": 98304, "value_percent": "150%"}},
       "properties": {"application.name": "mpv", "application.process.binary": null}},
      {"index": 14, "mute": false,
       "volume": {"front-left": {"value_percent": "40%"}},
       "properties": {"application.name": "Chromium"}},
      {"index": 16, "mute": false,
       "volume": {"front-left": {"value_percent": "50%"}},
       "properties": {"application.name": "Google Chrome", "application.process.binary": "chrome",
                      "application.icon_name": "google-chrome"}},
      {"index": 15, "mute": false,
       "volume": {"front-left": {"value_percent": "90%"}},
       "properties": {"application.name": "Chromium"}}
    ]"#;

    fn by_name<'a>(s: &'a [Stream], app: &str) -> Vec<&'a Stream> {
        matching(s, app)
    }

    fn lines(specs: &[CommandSpec]) -> Vec<String> {
        specs
            .iter()
            .map(|s| format!("{} {}", s.program, s.args.join(" ")))
            .collect()
    }

    #[test]
    fn parses_streams() {
        let s = parse_sink_inputs(JSON);
        assert_eq!(s.len(), 5);
        assert_eq!(s[0].label(), Some("Spotify"));
        assert_eq!(s[0].volume, 70);
        assert_eq!(s[1].binary, None);
        assert_eq!((s[1].volume, s[1].muted), (100, true));
        assert!(parse_sink_inputs("garbage").is_empty());
        assert!(parse_sink_inputs("[]").is_empty());
    }

    #[test]
    fn matches_apps() {
        let s = parse_sink_inputs(JSON);
        assert_eq!(by_name(&s, "spotify").len(), 1);
        assert_eq!(by_name(&s, "SPOTIFY").len(), 1);
        assert_eq!(by_name(&s, "chromium").len(), 2);
        assert!(by_name(&s, "firefox").is_empty());
        // Window class differs from the stream name.
        assert_eq!(by_name(&s, "google-chrome").len(), 1);
        assert_eq!(by_name(&s, "Google Chrome").len(), 1);
        assert_eq!(by_name(&s, "chrome").len(), 1);
        assert_eq!(level(&by_name(&s, "chromium")), (Some(40), false));
        assert_eq!(level(&by_name(&s, "mpv")), (Some(100), true));
        assert_eq!(level(&[]), (None, false));
    }

    #[test]
    fn builds_commands() {
        let s = parse_sink_inputs(JSON);
        let chromium = by_name(&s, "chromium");
        assert_eq!(
            lines(&twist(&chromium, 5, 2)),
            [
                "pactl set-sink-input-volume 14 50%",
                "pactl set-sink-input-volume 15 50%"
            ]
        );
        assert_eq!(
            lines(&twist(&by_name(&s, "spotify"), 5, -20)),
            ["pactl set-sink-input-volume 12 0%"]
        );
        assert_eq!(
            lines(&twist(&by_name(&s, "mpv"), 5, 1)),
            [
                "pactl set-sink-input-mute 13 0",
                "pactl set-sink-input-volume 13 100%"
            ]
        );
        assert!(twist(&chromium, 5, 0).is_empty());
        assert!(twist(&[], 5, 1).is_empty());
        assert_eq!(
            lines(&press(&chromium)),
            [
                "pactl set-sink-input-mute 14 1",
                "pactl set-sink-input-mute 15 1"
            ]
        );
        assert_eq!(
            lines(&press(&by_name(&s, "mpv"))),
            ["pactl set-sink-input-mute 13 0"]
        );
    }

    #[test]
    fn osd_commands() {
        assert_eq!(
            lines(&[osd("Spotify", 70, false)]),
            ["omarchy osd -i volume-high -m Spotify -p 70"]
        );
        assert_eq!(
            lines(&[osd("mpv", 70, true)]),
            ["omarchy osd -i volume-muted -m mpv -p 70"]
        );
    }

    #[test]
    fn alsa_clients() {
        let inputs = r#"[{"index": 23409, "client": "23408", "mute": false,
            "volume": {"front-left": {"value_percent": "80%"}},
            "properties": {"application.name": "PipeWire ALSA [cliamp]"}}]"#;
        let clients = r#"[{"index": 23408, "properties": {
            "application.process.binary": "cliamp", "application.process.id": "1281540"}}]"#;
        let mut s = parse_sink_inputs(inputs);
        assert_eq!(s[0].label(), Some("cliamp"));
        with_clients(&mut s, inputs, clients);
        assert_eq!(s[0].binary.as_deref(), Some("cliamp"));
        assert_eq!(by_name(&s, "cliamp").len(), 1);
    }

    #[test]
    fn selects_apps() {
        let s = parse_sink_inputs(JSON);
        let a = apps(&s);
        assert_eq!(a, ["Spotify", "mpv", "Chromium", "Google Chrome"]);
        assert_eq!(select(&a, None, 1).as_deref(), Some("Spotify"));
        assert_eq!(select(&a, Some("mpv"), 1).as_deref(), Some("Chromium"));
        assert_eq!(
            select(&a, Some("spotify"), -1).as_deref(),
            Some("Google Chrome")
        );
        assert_eq!(select(&a, Some("Google Chrome"), 2).as_deref(), Some("mpv"));
        assert_eq!(select(&a, Some("firefox"), 1).as_deref(), Some("Spotify"));
        assert_eq!(select(&[], Some("mpv"), 1), None);
    }

    #[test]
    fn stream_events() {
        assert!(is_stream_event("Event 'new' on sink-input #12"));
        assert!(is_stream_event("Event 'change' on sink-input #12"));
        assert!(is_stream_event("Event 'remove' on sink-input #12"));
        assert!(!is_stream_event("Event 'change' on sink #865"));
    }

    #[tokio::test]
    async fn reads_streams() {
        let r = RecordingRunner::with_response(CommandOutput {
            status: Some(0),
            stdout: JSON.into(),
            stderr: String::new(),
        });
        assert_eq!(read_streams(&r).await.len(), 5);
        assert_eq!(
            r.command_lines(),
            [
                "pactl -f json list sink-inputs",
                "pactl -f json list clients"
            ]
        );
    }
}
