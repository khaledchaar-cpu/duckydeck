//! User script actions: every executable in `<config dir>/scripts/` becomes
//! `script.<stem>`. The daemon talks to scripts in JSON lines: events on
//! stdin, key updates on stdout (see `docs/spec/actions.md`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::library::Slot;

/// Action id prefix of script actions.
pub const PREFIX: &str = "script.";
/// Header lines are only looked for this far into a script.
const HEADER_LINES: usize = 20;
/// Larger files are not scanned for a header (binaries).
const HEADER_MAX_BYTES: u64 = 256 * 1024;
pub const DEFAULT_ICON: &str = "script";

/// `<config dir>/scripts`.
pub fn dir() -> Option<PathBuf> {
    crate::config::dir().map(|d| d.join("scripts"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// `script.<stem>`.
    pub id: String,
    pub path: PathBuf,
    pub label: String,
    pub icon: String,
    pub slot: Slot,
    pub persistent: bool,
}

/// Scripts found in `dir`, keyed by action id. Two files with the same stem
/// (`a.sh`, `a.py`): the first in name order wins.
pub fn discover(dir: &Path) -> BTreeMap<String, Script> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| Some(e.ok()?.path())).collect();
    paths.sort();
    let mut out = BTreeMap::new();
    for path in paths {
        if let Some(s) = load(&path) {
            out.entry(s.id.clone()).or_insert(s);
        }
    }
    out
}

fn load(path: &Path) -> Option<Script> {
    use std::os::unix::fs::PermissionsExt;
    let name = path.file_name()?.to_str()?;
    if name.starts_with('.') {
        return None;
    }
    let stem = name.split_once('.').map_or(name, |(s, _)| s);
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return None;
    }
    // Follows symlinks: a link to a script elsewhere is fine.
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
        return None;
    }
    let header = if meta.len() <= HEADER_MAX_BYTES {
        std::fs::read(path)
            .map(|d| String::from_utf8_lossy(&d).into_owned())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let mut s = Script {
        id: format!("{PREFIX}{stem}"),
        path: path.to_owned(),
        label: stem.to_owned(),
        icon: DEFAULT_ICON.to_owned(),
        slot: Slot::Key,
        persistent: false,
    };
    parse_header(&header, &mut s);
    Some(s)
}

/// Applies `# duckydeck-<field>: <value>` lines; unknown fields are ignored.
fn parse_header(src: &str, s: &mut Script) {
    for line in src.lines().take(HEADER_LINES) {
        let Some(rest) = line.trim_start().strip_prefix('#') else {
            continue;
        };
        let Some((field, value)) = rest
            .trim_start()
            .strip_prefix("duckydeck-")
            .and_then(|r| r.split_once(':'))
        else {
            continue;
        };
        let value = value.trim();
        match (field.trim(), value) {
            ("label", v) if !v.is_empty() => s.label = v.to_owned(),
            ("icon", v) if !v.is_empty() => s.icon = v.to_owned(),
            ("slot", "dial") => s.slot = Slot::Dial,
            ("slot", "key") => s.slot = Slot::Key,
            ("persistent", v) => s.persistent = v == "true",
            (f, v) => tracing::debug!(field = f, value = v, "unknown script header field"),
        }
    }
}

/// What happened, sent to the script as one JSON line.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Init { args: serde_json::Value },
    Press { args: serde_json::Value },
    LongPress { args: serde_json::Value },
    Twist { delta: i32, args: serde_json::Value },
}

impl Event {
    /// One line, newline included.
    pub fn line(&self) -> String {
        let mut s = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned());
        s.push('\n');
        s
    }
}

/// Binding args as a JSON object.
pub fn args_json(args: &toml::Table) -> serde_json::Value {
    serde_json::to_value(args).unwrap_or_else(|_| serde_json::Value::Object(Default::default()))
}

/// What a script shows; `None` = the header value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub state: bool,
    pub value: Option<u8>,
}

/// One line from a script. A missing field keeps the old value, `null`
/// resets it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    #[serde(default, deserialize_with = "present")]
    label: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    icon: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    state: Option<Option<bool>>,
    #[serde(default, deserialize_with = "present")]
    value: Option<Option<f64>>,
}

/// Tells a present `null` (`Some(None)`) from a missing field (`None`).
fn present<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

impl State {
    /// Applies one stdout line; returns the parse error for the log.
    pub fn apply(&mut self, line: &str) -> Result<(), String> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }
        let u: Update = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if let Some(l) = u.label {
            self.label = l;
        }
        if let Some(i) = u.icon {
            self.icon = i;
        }
        if let Some(s) = u.state {
            self.state = s.unwrap_or(false);
        }
        if let Some(v) = u.value {
            // Clamped and rounded; `as` saturates and maps NaN to 0.
            self.value = v.map(|v| v.clamp(0.0, 100.0).round() as u8);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dd-script-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(dir: &Path, name: &str, src: &str, exec: bool) {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(name);
        std::fs::write(&p, src).unwrap();
        let mode = if exec { 0o755 } else { 0o644 };
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn discovers_executables_with_header() {
        let d = tmp("discover");
        write(
            &d,
            "mail.sh",
            "#!/bin/sh\n# duckydeck-label: Mail\n# duckydeck-icon: mail\n# duckydeck-persistent: true\n",
            true,
        );
        write(&d, "knob", "#!/bin/sh\n# duckydeck-slot: dial\n", true);
        write(&d, "notes.txt", "x", false);
        write(&d, ".hidden", "x", true);
        write(&d, "Bad Name", "x", true);
        write(&d, "mail.zsh", "x", true);
        let s = discover(&d);
        assert_eq!(s.keys().collect::<Vec<_>>(), ["script.knob", "script.mail"]);
        let mail = &s["script.mail"];
        assert_eq!(
            (
                mail.label.as_str(),
                mail.icon.as_str(),
                mail.slot,
                mail.persistent
            ),
            ("Mail", "mail", Slot::Key, true)
        );
        assert!(mail.path.ends_with("mail.sh"));
        let knob = &s["script.knob"];
        assert_eq!(
            (
                knob.label.as_str(),
                knob.icon.as_str(),
                knob.slot,
                knob.persistent
            ),
            ("knob", DEFAULT_ICON, Slot::Dial, false)
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn missing_dir_is_empty() {
        assert!(discover(Path::new("/nonexistent/duckydeck-scripts")).is_empty());
    }

    #[test]
    fn events_are_json_lines() {
        let args = args_json(&toml::toml! { account = "work" });
        assert_eq!(
            Event::Twist {
                delta: -2,
                args: args.clone()
            }
            .line(),
            "{\"event\":\"twist\",\"delta\":-2,\"args\":{\"account\":\"work\"}}\n"
        );
        assert_eq!(
            Event::LongPress {
                args: args_json(&toml::Table::new())
            }
            .line(),
            "{\"event\":\"long_press\",\"args\":{}}\n"
        );
    }

    #[test]
    fn updates_merge_and_reset() {
        let mut s = State::default();
        s.apply(r#"{"label":"3 Mails","state":true,"value":140}"#)
            .unwrap();
        assert_eq!(
            s,
            State {
                label: Some("3 Mails".into()),
                icon: None,
                state: true,
                value: Some(100)
            }
        );
        s.apply(r#"{"icon":"mail"}"#).unwrap();
        assert_eq!(s.label.as_deref(), Some("3 Mails"));
        s.apply(r#"{"label":null,"state":null,"value":42.4}"#)
            .unwrap();
        assert_eq!(
            s,
            State {
                label: None,
                icon: Some("mail".into()),
                state: false,
                value: Some(42)
            }
        );
        s.apply("   ").unwrap();
        assert!(s.apply("not json").is_err());
        assert!(s.apply(r#"{"colour":"red"}"#).is_err());
    }
}
