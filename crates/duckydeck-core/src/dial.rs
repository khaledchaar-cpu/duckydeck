//! Dial actions with state: output volume, microphone, display brightness.
//!
//! Turning and pressing map to `omarchy` routes (which also show the OSD);
//! the shown level is read back from `pactl` and `omarchy brightness display`.

use crate::command::{CommandError, CommandRunner, CommandSpec};
use crate::config::Binding;

const DEFAULT_STEP: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dial {
    /// Turn = `omarchy audio output volume ±N`, press = mute toggle.
    Volume { step: u8 },
    /// Turn does nothing, press = `omarchy audio input mute`.
    Mic,
    /// Turn = `omarchy brightness display ±N%`, press does nothing.
    Brightness { step: u8 },
}

impl Dial {
    /// The dial action behind a binding; `None` for other actions.
    pub fn from_binding(b: &Binding) -> Option<Self> {
        let step = || {
            let s = b
                .args
                .get("step")
                .and_then(|v| v.as_integer())
                .unwrap_or(DEFAULT_STEP);
            u8::try_from(s.clamp(1, 100)).unwrap_or(5)
        };
        match b.action.as_str() {
            "media.volume" => Some(Self::Volume { step: step() }),
            "media.mic" => Some(Self::Mic),
            "display.brightness" => Some(Self::Brightness { step: step() }),
            _ => None,
        }
    }

    /// Command for `delta` detents (negative = counter-clockwise).
    pub fn twist(self, delta: i32) -> Option<CommandSpec> {
        if delta == 0 {
            return None;
        }
        let sign = if delta > 0 { '+' } else { '-' };
        match self {
            Self::Volume { step } => {
                let n = u32::from(step) * delta.unsigned_abs();
                Some(CommandSpec::omarchy([
                    "audio".into(),
                    "output".into(),
                    "volume".into(),
                    format!("{sign}{n}"),
                ]))
            }
            Self::Mic => None,
            Self::Brightness { step } => {
                let n = (u32::from(step) * delta.unsigned_abs()).min(100);
                let arg = if delta > 0 {
                    format!("+{n}%")
                } else {
                    format!("{n}%-")
                };
                Some(CommandSpec::omarchy([
                    "brightness".into(),
                    "display".into(),
                    arg,
                ]))
            }
        }
    }

    pub fn press(self) -> Option<CommandSpec> {
        match self {
            Self::Volume { .. } => Some(CommandSpec::omarchy([
                "audio",
                "output",
                "volume",
                "mute-toggle",
            ])),
            Self::Mic => Some(CommandSpec::omarchy(["audio", "input", "mute"])),
            Self::Brightness { .. } => None,
        }
    }
}

/// Last known levels; `None` = unknown (not shown).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Levels {
    pub volume: Option<u8>,
    pub muted: Option<bool>,
    pub mic_volume: Option<u8>,
    pub mic_muted: Option<bool>,
    pub brightness: Option<u8>,
}

impl Levels {
    /// What a dial shows: level in percent and whether it is muted.
    pub fn of(&self, dial: Dial) -> (Option<u8>, bool) {
        match dial {
            Dial::Volume { .. } => (self.volume, self.muted == Some(true)),
            Dial::Mic => (self.mic_volume, self.mic_muted == Some(true)),
            Dial::Brightness { .. } => (self.brightness, false),
        }
    }
}

/// Reads output and input volume/mute of the default devices.
pub async fn read_audio(runner: &dyn CommandRunner, levels: &mut Levels) {
    let pactl = |args: [&str; 2]| CommandSpec::new("pactl").args(args);
    levels.volume = stdout(runner, &pactl(["get-sink-volume", "@DEFAULT_SINK@"]))
        .await
        .and_then(|s| parse_volume(&s));
    levels.muted = stdout(runner, &pactl(["get-sink-mute", "@DEFAULT_SINK@"]))
        .await
        .and_then(|s| parse_mute(&s));
    levels.mic_volume = stdout(runner, &pactl(["get-source-volume", "@DEFAULT_SOURCE@"]))
        .await
        .and_then(|s| parse_volume(&s));
    levels.mic_muted = stdout(runner, &pactl(["get-source-mute", "@DEFAULT_SOURCE@"]))
        .await
        .and_then(|s| parse_mute(&s));
}

/// Brightness of the focused display in percent.
pub async fn read_brightness(runner: &dyn CommandRunner) -> Option<u8> {
    let spec = CommandSpec::omarchy(["brightness", "display"]);
    stdout(runner, &spec)
        .await
        .and_then(|s| s.trim().parse::<u8>().ok())
}

async fn stdout(runner: &dyn CommandRunner, spec: &CommandSpec) -> Option<String> {
    match runner.run(spec).await {
        Ok(out) if out.success() => Some(out.stdout),
        res => {
            let res: Result<_, &CommandError> = res.as_ref().map(|o| o.status);
            tracing::debug!(program = %spec.program, args = ?spec.args, result = ?res, "level read failed");
            None
        }
    }
}

/// First channel's percentage of `pactl get-*-volume`.
pub fn parse_volume(s: &str) -> Option<u8> {
    let pct = s.split('%').next()?;
    let n = pct.rsplit(|c: char| !c.is_ascii_digit()).next()?;
    // Over-amplified volumes are shown full.
    n.parse::<u32>().ok().map(|v| v.min(100) as u8)
}

/// `Mute: yes` / `Mute: no` of `pactl get-*-mute`.
pub fn parse_mute(s: &str) -> Option<bool> {
    match s.trim().strip_prefix("Mute:")?.trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// Whether a `pactl subscribe` line can change the default devices' levels.
pub fn is_audio_event(line: &str) -> bool {
    line.starts_with("Event 'change' on ")
        && [" sink #", " source #", " server #"]
            .iter()
            .any(|k| line.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutput, RecordingRunner};

    fn binding(action: &str, args: &str) -> Binding {
        Binding {
            action: action.into(),
            args: toml::from_str(args).unwrap(),
            label: None,
            icon: None,
        }
    }

    fn line(spec: Option<CommandSpec>) -> String {
        let s = spec.unwrap();
        std::iter::once(s.program)
            .chain(s.args)
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn maps_bindings() {
        assert_eq!(
            Dial::from_binding(&binding("media.volume", "step = 2")),
            Some(Dial::Volume { step: 2 })
        );
        assert_eq!(
            Dial::from_binding(&binding("display.brightness", "")),
            Some(Dial::Brightness { step: 5 })
        );
        assert_eq!(
            Dial::from_binding(&binding("media.mic", "")),
            Some(Dial::Mic)
        );
        assert_eq!(Dial::from_binding(&binding("system.lock", "")), None);
    }

    #[test]
    fn builds_commands() {
        let v = Dial::Volume { step: 5 };
        assert_eq!(line(v.twist(2)), "omarchy audio output volume +10");
        assert_eq!(line(v.twist(-1)), "omarchy audio output volume -5");
        assert_eq!(line(v.press()), "omarchy audio output volume mute-toggle");
        assert!(v.twist(0).is_none());
        let b = Dial::Brightness { step: 5 };
        assert_eq!(line(b.twist(1)), "omarchy brightness display +5%");
        assert_eq!(line(b.twist(-3)), "omarchy brightness display 15%-");
        assert!(b.press().is_none());
        assert_eq!(line(Dial::Mic.press()), "omarchy audio input mute");
        assert!(Dial::Mic.twist(1).is_none());
    }

    #[test]
    fn parses_pactl() {
        let v = "Volume: front-left: 44462 /  68% / -10.11 dB,   front-right: 44462 /  68% / -10.11 dB\n        balance 0.00\n";
        assert_eq!(parse_volume(v), Some(68));
        assert_eq!(
            parse_volume("Volume: mono: 98304 / 150% / 10 dB"),
            Some(100)
        );
        assert_eq!(parse_volume("garbage"), None);
        assert_eq!(parse_mute("Mute: yes\n"), Some(true));
        assert_eq!(parse_mute("Mute: no"), Some(false));
        assert_eq!(parse_mute(""), None);
        assert!(is_audio_event("Event 'change' on sink #865"));
        assert!(is_audio_event("Event 'change' on server #-1"));
        assert!(!is_audio_event("Event 'change' on client #8607"));
        assert!(!is_audio_event("Event 'new' on sink-input #12"));
    }

    #[tokio::test]
    async fn reads_brightness() {
        let r = RecordingRunner::with_response(CommandOutput {
            status: Some(0),
            stdout: "42\n".into(),
            stderr: String::new(),
        });
        assert_eq!(read_brightness(&r).await, Some(42));
        assert_eq!(r.command_lines(), ["omarchy brightness display"]);
    }
}
