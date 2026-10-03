//! User configuration: `config.toml` (global settings) and profiles
//! (`profiles/<name>.toml`, pages and folders of key/dial bindings).
//!
//! Parsing is purely structural; whether an action id exists is checked
//! against the action catalog elsewhere.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::command::{CommandRunner, CommandSpec};

pub const KEYS: usize = 8;
pub const DIALS: usize = 4;
/// Action that opens a folder; its `folder` arg names an entry of `[folders]`.
pub const FOLDER_ACTION: &str = "structure.folder";
/// The default profile, also shipped as an example.
pub const DEFAULT_PROFILE: &str = include_str!("../../../examples/profiles/omarchy.toml");
pub const DEFAULT_PROFILE_ID: &str = "omarchy";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    #[error("{path}: {msg}")]
    Invalid { path: PathBuf, msg: String },
}

/// Global settings from `config.toml`. Every field is optional.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Device brightness in percent.
    pub brightness: u8,
    /// Minutes without input until the deck dims; 0 disables it.
    pub screensaver_minutes: u32,
    /// Profile id (file stem in `profiles/`) used when nothing else matches.
    pub profile: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            brightness: 60,
            screensaver_minutes: 10,
            profile: DEFAULT_PROFILE_ID.to_owned(),
        }
    }
}

impl Config {
    pub fn parse(src: &str, path: &Path) -> Result<Self, ConfigError> {
        let config: Self = parse_toml(src, path)?;
        if config.brightness > 100 {
            return Err(invalid(path, "brightness must be 0-100"));
        }
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    #[serde(default, rename = "match")]
    pub matcher: Option<WindowMatch>,
    pub pages: Vec<Page>,
    #[serde(default)]
    pub folders: BTreeMap<String, Page>,
}

/// Regexes against the active Hyprland window (evaluated in M8).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowMatch {
    pub class: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default)]
    pub keys: Vec<Slot>,
    #[serde(default)]
    pub dials: Vec<Slot>,
}

/// A key or dial position; `{}` in TOML leaves it empty.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(from = "RawSlot")]
pub struct Slot(pub Option<Binding>);

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub action: String,
    pub args: toml::Table,
    pub label: Option<String>,
    pub icon: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSlot {
    action: Option<String>,
    #[serde(default)]
    args: toml::Table,
    label: Option<String>,
    icon: Option<String>,
}

impl From<RawSlot> for Slot {
    fn from(raw: RawSlot) -> Self {
        // A slot without action but with args/label is reported by `validate`.
        Slot(
            Some(Binding {
                action: raw.action.unwrap_or_default(),
                args: raw.args,
                label: raw.label,
                icon: raw.icon,
            })
            .filter(|b| {
                !(b.action.is_empty() && b.args.is_empty() && b.label.is_none() && b.icon.is_none())
            }),
        )
    }
}

impl Profile {
    pub fn parse(src: &str, path: &Path) -> Result<Self, ConfigError> {
        let profile: Self = parse_toml(src, path)?;
        profile.validate().map_err(|msg| invalid(path, &msg))?;
        Ok(profile)
    }

    pub fn default_profile() -> Result<Self, ConfigError> {
        Self::parse(DEFAULT_PROFILE, Path::new("examples/profiles/omarchy.toml"))
    }

    fn validate(&self) -> Result<(), String> {
        if self.pages.is_empty() {
            return Err("profile needs at least one [[pages]]".into());
        }
        let pages = self
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| (format!("page {}", i + 1), p, KEYS));
        // The last folder key is reserved for the automatic back key.
        let folders = self
            .folders
            .iter()
            .map(|(n, p)| (format!("folder {n:?}"), p, KEYS - 1));
        for (what, page, max_keys) in pages.chain(folders) {
            if page.keys.len() > max_keys {
                return Err(format!(
                    "{what}: {} keys, at most {max_keys}",
                    page.keys.len()
                ));
            }
            if page.dials.len() > DIALS {
                return Err(format!(
                    "{what}: {} dials, at most {DIALS}",
                    page.dials.len()
                ));
            }
            for b in page
                .keys
                .iter()
                .chain(&page.dials)
                .filter_map(|s| s.0.as_ref())
            {
                self.check_binding(b).map_err(|e| format!("{what}: {e}"))?;
            }
        }
        Ok(())
    }

    fn check_binding(&self, b: &Binding) -> Result<(), String> {
        if b.action.is_empty() {
            return Err("slot has no action (use {} for an empty slot)".into());
        }
        if b.action == FOLDER_ACTION {
            let Some(name) = b.args.get("folder").and_then(|v| v.as_str()) else {
                return Err(format!("{FOLDER_ACTION} needs args.folder"));
            };
            if !self.folders.contains_key(name) {
                return Err(format!("unknown folder {name:?}"));
            }
        }
        Ok(())
    }
}

/// Everything under the config directory (`~/.config/duckydeck`).
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub config: Config,
    /// Profile id (file stem) → profile; always contains the default profile.
    pub profiles: BTreeMap<String, Profile>,
}

impl Loaded {
    /// Missing files fall back to defaults; any invalid file fails the load,
    /// so the caller can keep the last valid state.
    pub fn load(dir: &Path) -> Result<Self, ConfigError> {
        let config_path = dir.join("config.toml");
        let config = match read_optional(&config_path)? {
            Some(src) => Config::parse(&src, &config_path)?,
            None => Config::default(),
        };
        let mut profiles = BTreeMap::new();
        profiles.insert(DEFAULT_PROFILE_ID.to_owned(), Profile::default_profile()?);
        let profile_dir = dir.join("profiles");
        let entries = match std::fs::read_dir(&profile_dir) {
            Ok(entries) => entries.collect::<Result<Vec<_>, _>>(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
        .map_err(|source| ConfigError::Read {
            path: profile_dir.clone(),
            source,
        })?;
        for entry in entries {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "toml") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let src = read_optional(&path)?.unwrap_or_default();
            profiles.insert(id.to_owned(), Profile::parse(&src, &path)?);
        }
        if !profiles.contains_key(&config.profile) {
            return Err(invalid(
                &config_path,
                &format!("unknown profile {:?}", config.profile),
            ));
        }
        Ok(Self { config, profiles })
    }
}

/// `$XDG_CONFIG_HOME/duckydeck`, falling back to `~/.config/duckydeck`.
pub fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("duckydeck"))
}

/// The active configuration; an invalid reload keeps the last valid state
/// and reports the error as a shell notification.
pub struct Store {
    dir: PathBuf,
    pub current: Loaded,
}

impl Store {
    /// Loads `dir`; on error falls back to the built-in defaults.
    pub fn open(dir: PathBuf, runner: &dyn CommandRunner) -> Result<Self, ConfigError> {
        let current = match Loaded::load(&dir) {
            Ok(loaded) => loaded,
            Err(e) => {
                notify_error(runner, &e);
                Loaded {
                    config: Config::default(),
                    profiles: BTreeMap::from([(
                        DEFAULT_PROFILE_ID.to_owned(),
                        Profile::default_profile()?,
                    )]),
                }
            }
        };
        Ok(Self { dir, current })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Returns whether the active configuration changed.
    pub fn reload(&mut self, runner: &dyn CommandRunner) -> bool {
        match Loaded::load(&self.dir) {
            Ok(loaded) if loaded == self.current => false,
            Ok(loaded) => {
                self.current = loaded;
                true
            }
            Err(e) => {
                notify_error(runner, &e);
                false
            }
        }
    }
}

fn notify_error(runner: &dyn CommandRunner, e: &ConfigError) {
    tracing::warn!(error = %e, "invalid config, keeping last valid one");
    let spec = CommandSpec::omarchy([
        "notification",
        "send",
        "--app-name",
        "DuckyDeck",
        "-u",
        "critical",
    ])
    .args(["DuckyDeck config error".to_owned(), e.to_string()]);
    if let Err(e) = runner.spawn(&spec) {
        tracing::warn!(error = %e, "config error notification failed");
    }
}

fn parse_toml<T: serde::de::DeserializeOwned>(src: &str, path: &Path) -> Result<T, ConfigError> {
    toml::from_str(src).map_err(|e| ConfigError::Parse {
        path: path.to_owned(),
        source: Box::new(e),
    })
}

fn read_optional(path: &Path) -> Result<Option<String>, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Read {
            path: path.to_owned(),
            source,
        }),
    }
}

fn invalid(path: &Path, msg: &str) -> ConfigError {
    ConfigError::Invalid {
        path: path.to_owned(),
        msg: msg.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(src: &str) -> Result<Profile, ConfigError> {
        Profile::parse(src, Path::new("p.toml"))
    }

    fn err(src: &str) -> String {
        match profile(src) {
            Ok(p) => panic!("expected error, got {p:?}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn default_profile_parses() -> anyhow::Result<()> {
        let p = Profile::default_profile()?;
        assert_eq!(p.name, "Omarchy");
        assert_eq!(p.pages[0].keys.len(), KEYS);
        assert_eq!(p.pages[0].dials.len(), DIALS);
        let b = p.pages[0].dials[0]
            .0
            .as_ref()
            .ok_or(anyhow::anyhow!("empty"))?;
        assert_eq!(b.action, "media.volume");
        assert_eq!(b.args.get("step").and_then(|v| v.as_integer()), Some(5));
        Ok(())
    }

    #[test]
    fn empty_slots_and_folders() -> anyhow::Result<()> {
        let p = profile(
            r#"
            name = "T"
            match = { class = "^kitty$" }
            [[pages]]
            keys = [{}, { action = "structure.folder", args = { folder = "power" }, label = "Power" }]
            [folders.power]
            keys = [{ action = "system.lock", icon = "lock" }]
            "#,
        )?;
        assert_eq!(p.pages[0].keys[0], Slot(None));
        assert_eq!(
            p.pages[0].keys[1]
                .0
                .as_ref()
                .and_then(|b| b.label.as_deref()),
            Some("Power")
        );
        assert_eq!(p.matcher.and_then(|m| m.class).as_deref(), Some("^kitty$"));
        assert_eq!(p.folders["power"].keys.len(), 1);
        Ok(())
    }

    #[test]
    fn rejects_invalid_profiles() {
        assert!(err(r#"name = "T""#).contains("pages"));
        assert!(err("name = \"T\"\npages = []").contains("at least one"));
        let nine = [r#"{ action = "a" }"#; 9].join(",");
        assert!(err(&format!("name = \"T\"\n[[pages]]\nkeys = [{nine}]")).contains("at most 8"));
        let five = ["{}"; 5].join(",");
        assert!(err(&format!("name = \"T\"\n[[pages]]\ndials = [{five}]")).contains("at most 4"));
        let eight = ["{}"; 8].join(",");
        assert!(
            err(&format!(
                "name = \"T\"\n[[pages]]\n[folders.f]\nkeys = [{eight}]"
            ))
            .contains("at most 7")
        );
        assert!(err("name = \"T\"\n[[pages]]\nkeys = [{ label = \"x\" }]").contains("no action"));
        assert!(
            err("name = \"T\"\n[[pages]]\nkeys = [{ action = \"structure.folder\", args = { folder = \"x\" } }]")
                .contains("unknown folder")
        );
        assert!(err("name = \"T\"\n[[pages]]\nkeys = [{ acton = \"a\" }]").contains("acton"));
    }

    #[test]
    fn parse_error_reports_line() {
        let e = err("name = \"T\"\n\n[[pages]]\nkeys = [{ action = 1 }]");
        assert!(e.contains("p.toml") && e.contains("line 4"), "{e}");
    }

    #[test]
    fn config_defaults_and_bounds() -> anyhow::Result<()> {
        let p = Path::new("config.toml");
        assert_eq!(Config::parse("", p)?, Config::default());
        assert_eq!(Config::parse("brightness = 30", p)?.brightness, 30);
        assert!(Config::parse("brightness = 101", p).is_err());
        assert!(Config::parse("brightnes = 1", p).is_err());
        Ok(())
    }

    #[test]
    fn load_directory() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("duckydeck-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(Loaded::load(&dir)?.config, Config::default());

        std::fs::create_dir_all(dir.join("profiles"))?;
        std::fs::write(dir.join("config.toml"), "profile = \"work\"")?;
        std::fs::write(dir.join("profiles/work.toml"), "name = \"Work\"\n[[pages]]")?;
        std::fs::write(dir.join("profiles/notes.txt"), "ignored")?;
        let loaded = Loaded::load(&dir)?;
        assert_eq!(
            loaded.profiles.keys().collect::<Vec<_>>(),
            ["omarchy", "work"]
        );

        std::fs::write(dir.join("config.toml"), "profile = \"nope\"")?;
        assert!(Loaded::load(&dir).is_err());
        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }

    #[test]
    fn store_keeps_last_valid_config() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("duckydeck-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let runner = crate::RecordingRunner::new();

        std::fs::write(dir.join("config.toml"), "brightness = 30")?;
        let mut store = Store::open(dir.clone(), &runner)?;
        assert_eq!(store.current.config.brightness, 30);
        assert!(!store.reload(&runner));

        std::fs::write(dir.join("config.toml"), "brightness = 40")?;
        assert!(store.reload(&runner));
        assert_eq!(store.current.config.brightness, 40);
        assert!(runner.calls().is_empty());

        std::fs::write(dir.join("config.toml"), "brightness = 4\nbrightness = 5")?;
        assert!(!store.reload(&runner));
        assert_eq!(store.current.config.brightness, 40);
        let lines = runner.command_lines();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("omarchy notification send --app-name DuckyDeck"));
        assert!(
            lines[0].contains("config.toml") && lines[0].contains("line 2"),
            "{}",
            lines[0]
        );

        std::fs::remove_dir_all(&dir)?;
        Ok(())
    }
}
