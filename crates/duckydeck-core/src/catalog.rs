//! Declarative command actions from `actions/catalog.toml`.
//!
//! Each entry is "run a command (or a Hyprland dispatch) + show an icon" and
//! is executed through [`Entry::exec`]. Actions with real logic are Rust code
//! elsewhere.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Deserialize;

use crate::command::CommandSpec;
use crate::toggle::StateSource;

/// The built-in catalog, embedded at build time.
pub const CATALOG: &str = include_str!("../../../actions/catalog.toml");

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid catalog: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("action `{0}` needs exactly one of `run` and `dispatch`")]
    EmptyRun(String),
    #[error("action `{action}` needs argument `{arg}`")]
    MissingArg { action: String, arg: String },
    #[error("action `{action}`: argument `{arg}` must be a string, number or bool")]
    BadArg { action: String, arg: String },
    #[error("action `{action}`: argument `{arg}` may only contain letters, digits and `_+-:`")]
    UnsafeArg { action: String, arg: String },
    #[error(
        "action `{0}`: `state` needs exactly one of `file` and `command`; `elapsed` needs `file`"
    )]
    BadState(String),
    #[error("action `{0}`: `state` needs an `{{ on, off }}` icon")]
    StateIcon(String),
    #[error("invalid output of `omarchy commands --json`: {0}")]
    Routes(#[from] serde_json::Error),
}

/// One icon, or one per toggle state.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Icon {
    Single(String),
    Toggle { on: String, off: String },
}

impl Icon {
    /// Icon for a toggle state; `off` while the state is unknown.
    pub fn name(&self, on: Option<bool>) -> &str {
        match self {
            Self::Single(n) => n,
            Self::Toggle { on: n, .. } if on == Some(true) => n,
            Self::Toggle { off, .. } => off,
        }
    }

    pub fn names(&self) -> Vec<&str> {
        match self {
            Self::Single(n) => vec![n],
            Self::Toggle { on, off } => vec![on, off],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confirm {
    /// Runs only on a long press; a tap does nothing.
    LongPress,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub label: String,
    pub icon: Icon,
    #[serde(default)]
    pub run: Vec<String>,
    /// Hyprland dispatcher in Lua (`hl.dsp.…`), sent over the IPC socket.
    pub dispatch: Option<String>,
    /// Placeholders that take a whole dispatcher expression (`hl.dsp.…`)
    /// from the profile instead of a value inside a Lua string.
    #[serde(default)]
    pub raw: Vec<String>,
    /// Omarchy route that must exist; default: derived from `run`.
    pub requires: Option<String>,
    pub confirm: Option<Confirm>,
    /// Placeholder values used when the binding does not set them.
    #[serde(default)]
    pub defaults: toml::Table,
    /// Where the toggle state comes from; needs an `{ on, off }` icon.
    pub state: Option<StateSource>,
}

impl Entry {
    /// What the action does, filling `{name}` placeholders from `args`, then
    /// `defaults`. Dispatch values are restricted so they stay inside their
    /// Lua string.
    pub fn exec(&self, id: &str, args: &toml::Table) -> Result<Exec, CatalogError> {
        if let Some(d) = &self.dispatch {
            return Ok(Exec::Dispatch(self.fill(id, d, args, true)?));
        }
        let mut parts = self.run.iter().map(|a| self.fill(id, a, args, false));
        let program = parts
            .next()
            .ok_or_else(|| CatalogError::EmptyRun(id.to_owned()))??;
        Ok(Exec::Command(
            CommandSpec::new(program).args(parts.collect::<Result<Vec<_>, _>>()?),
        ))
    }

    fn fill(
        &self,
        id: &str,
        arg: &str,
        args: &toml::Table,
        lua: bool,
    ) -> Result<String, CatalogError> {
        let mut out = String::new();
        let mut rest = arg;
        while let Some(start) = rest.find('{') {
            let Some(len) = rest[start..].find('}') else {
                break;
            };
            let name = &rest[start + 1..start + len];
            // Lua tables (`{ mode = "x" }`) are not placeholders.
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.push_str(&rest[..=start]);
                rest = &rest[start + 1..];
                continue;
            }
            let value = args
                .get(name)
                .or_else(|| self.defaults.get(name))
                .ok_or_else(|| CatalogError::MissingArg {
                    action: id.to_owned(),
                    arg: name.to_owned(),
                })?;
            let value = scalar(value).ok_or_else(|| CatalogError::BadArg {
                action: id.to_owned(),
                arg: name.to_owned(),
            })?;
            let unsafe_value = if self.raw.iter().any(|r| r == name) {
                !value.starts_with("hl.dsp.") || value.contains(['\n', '\r'])
            } else {
                lua && !value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_+-:".contains(c))
            };
            if unsafe_value {
                return Err(CatalogError::UnsafeArg {
                    action: id.to_owned(),
                    arg: name.to_owned(),
                });
            }
            out.push_str(&rest[..start]);
            out.push_str(&value);
            rest = &rest[start + len + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    /// The omarchy route this entry needs, if any. Without an explicit
    /// `requires`, the longest prefix of `run` found in `routes`, or the
    /// first three words when none matches.
    pub fn route(&self, routes: &HashSet<String>) -> Option<String> {
        if let Some(r) = &self.requires {
            return Some(r.clone());
        }
        if self.run.first().map(String::as_str) != Some("omarchy") {
            return None;
        }
        let words: Vec<&str> = self
            .run
            .iter()
            .take_while(|w| !w.contains('{') && !w.starts_with('-'))
            .map(String::as_str)
            .collect();
        (2..=words.len())
            .rev()
            .map(|n| words[..n].join(" "))
            .find(|r| routes.contains(r))
            .or_else(|| Some(words[..words.len().min(3)].join(" ")))
    }
}

/// How an entry is executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exec {
    Command(CommandSpec),
    /// Lua dispatcher expression for Hyprland.
    Dispatch(String),
}

#[derive(Debug, Clone)]
pub struct Catalog {
    entries: BTreeMap<String, Entry>,
}

impl Catalog {
    pub fn parse(src: &str) -> Result<Self, CatalogError> {
        let groups: BTreeMap<String, BTreeMap<String, Entry>> = toml::from_str(src)?;
        let entries: BTreeMap<String, Entry> = groups
            .into_iter()
            .flat_map(|(g, es)| es.into_iter().map(move |(n, e)| (format!("{g}.{n}"), e)))
            .collect();
        if let Some((id, _)) = entries
            .iter()
            .find(|(_, e)| e.run.is_empty() == e.dispatch.is_none())
        {
            return Err(CatalogError::EmptyRun(id.clone()));
        }
        for (id, e) in &entries {
            let Some(st) = &e.state else { continue };
            if !st.is_valid() {
                return Err(CatalogError::BadState(id.clone()));
            }
            if !matches!(e.icon, Icon::Toggle { .. }) {
                return Err(CatalogError::StateIcon(id.clone()));
            }
        }
        Ok(Self { entries })
    }

    pub fn builtin() -> Result<Self, CatalogError> {
        Self::parse(CATALOG)
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Entry)> {
        self.entries.iter()
    }

    /// Ids of entries whose omarchy route is missing from `routes`.
    pub fn unavailable(&self, routes: &HashSet<String>) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|(_, e)| e.route(routes).is_some_and(|r| !routes.contains(&r)))
            .map(|(id, _)| id.clone())
            .collect()
    }
}

/// All routes (including aliases) from `omarchy commands --json`.
pub fn parse_routes(json: &str) -> Result<HashSet<String>, CatalogError> {
    #[derive(Deserialize)]
    struct Commands {
        commands: Vec<Command>,
    }
    #[derive(Deserialize)]
    struct Command {
        route: String,
        #[serde(default)]
        routes: Vec<String>,
    }
    let c: Commands = serde_json::from_str(json)?;
    Ok(c.commands
        .into_iter()
        .flat_map(|c| std::iter::once(c.route).chain(c.routes))
        .collect())
}

fn scalar(v: &toml::Value) -> Option<String> {
    match v {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Float(f) => Some(f.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandRunner, RecordingRunner};
    use crate::icons;

    /// Routes from the generated `docs/omarchy-reference.md`.
    fn reference_routes() -> HashSet<String> {
        include_str!("../../../docs/omarchy-reference.md")
            .lines()
            .filter_map(|l| l.strip_prefix("| `")?.split('`').next())
            .filter(|r| r.starts_with("omarchy "))
            .map(str::to_owned)
            .collect()
    }

    fn command(e: &Entry, id: &str, args: &toml::Table) -> Result<CommandSpec, CatalogError> {
        match e.exec(id, args)? {
            Exec::Command(spec) => Ok(spec),
            Exec::Dispatch(d) => panic!("{id} is a dispatch: {d}"),
        }
    }

    #[tokio::test]
    async fn builtin_catalog_is_valid() {
        let catalog = Catalog::builtin().unwrap();
        let routes = reference_routes();
        assert!(routes.contains("omarchy system lock"));
        for (id, e) in catalog.iter() {
            for name in e.icon.names() {
                assert!(icons::get(name).is_some(), "{id}: unknown icon `{name}`");
            }
            if let Some(r) = e.route(&routes) {
                assert!(routes.contains(&r), "{id}: unknown route `{r}`");
            }
            // Raw dispatches have no sensible default; see their own test.
            if !e.raw.is_empty() {
                continue;
            }
            // Defaults complete every placeholder; the call goes out verbatim.
            let spec = match e.exec(id, &toml::Table::new()).unwrap() {
                Exec::Command(spec) => spec,
                Exec::Dispatch(d) => {
                    assert!(d.starts_with("hl.dsp."), "{id}: {d}");
                    continue;
                }
            };
            assert!(!spec.args.iter().any(|a| a.contains('{')), "{id}");
            let runner = RecordingRunner::new();
            runner.spawn(&spec).unwrap();
            let line = runner.command_lines().remove(0);
            assert!(line.starts_with(&e.run[0]), "{id}: {line}");
        }
        assert!(catalog.unavailable(&routes).is_empty());
    }

    #[test]
    fn placeholders_stay_single_arguments() {
        let c = Catalog::builtin().unwrap();
        let e = c.get("capture.screenshot").unwrap();
        let mut args = toml::Table::new();
        args.insert("mode".into(), "region; rm -rf ~".into());
        let spec = command(e, "capture.screenshot", &args).unwrap();
        assert_eq!(spec.program, "omarchy");
        assert_eq!(spec.args, ["capture", "screenshot", "region; rm -rf ~"]);
        let spec = command(e, "capture.screenshot", &toml::Table::new()).unwrap();
        assert_eq!(spec.args[2], "smart");
    }

    #[test]
    fn missing_and_bad_args() {
        let c = Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"lock\"\nrun = [\"a\", \"--n={n}\"]")
            .unwrap();
        let e = c.get("x.y").unwrap();
        assert!(matches!(
            command(e, "x.y", &toml::Table::new()),
            Err(CatalogError::MissingArg { .. })
        ));
        let mut args = toml::Table::new();
        args.insert("n".into(), 3.into());
        assert_eq!(command(e, "x.y", &args).unwrap().args, ["--n=3"]);
        args.insert("n".into(), toml::Value::Array(vec![]));
        assert!(matches!(
            command(e, "x.y", &args),
            Err(CatalogError::BadArg { .. })
        ));
    }

    #[test]
    fn rejects_unknown_fields_and_empty_run() {
        assert!(
            Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"a\"\nrun = [\"a\"]\nfoo = 1").is_err()
        );
        assert!(Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"a\"\nrun = []").is_err());
        assert!(Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"a\"").is_err());
        assert!(
            Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"a\"\nrun = [\"a\"]\ndispatch = \"b\"")
                .is_err()
        );
    }

    #[test]
    fn state_needs_one_source_and_toggle_icon() {
        let entry = |extra: &str| format!("[x.y]\nlabel = \"Y\"\nrun = [\"a\"]\n{extra}");
        let toggle = "icon = { on = \"a\", off = \"b\" }";
        assert!(Catalog::parse(&entry(&format!("{toggle}\nstate = {{ file = \"/x\" }}"))).is_ok());
        assert!(matches!(
            Catalog::parse(&entry(&format!("{toggle}\nstate = {{ json = \"k\" }}"))),
            Err(CatalogError::BadState(_))
        ));
        assert!(matches!(
            Catalog::parse(&entry("icon = \"a\"\nstate = { file = \"/x\" }")),
            Err(CatalogError::StateIcon(_))
        ));
    }

    #[test]
    fn dispatch_args_stay_inside_lua_strings() {
        let c = Catalog::builtin().unwrap();
        let e = c.get("window.workspace").unwrap();
        let mut args = toml::Table::new();
        args.insert("n".into(), 3.into());
        assert_eq!(
            e.exec("window.workspace", &args).unwrap(),
            Exec::Dispatch(r#"hl.dsp.focus({ workspace = "3" })"#.into())
        );
        args.insert("n".into(), r#"1" }) os.exit() --"#.into());
        assert!(matches!(
            e.exec("window.workspace", &args),
            Err(CatalogError::UnsafeArg { .. })
        ));
    }

    #[test]
    fn raw_dispatch_takes_whole_expression() {
        let c = Catalog::builtin().unwrap();
        let e = c.get("window.dispatch").unwrap();
        let mut args = toml::Table::new();
        let expr = r#"hl.dsp.window.move({ monitor = "+1" })"#;
        args.insert("expr".into(), expr.into());
        assert_eq!(
            e.exec("window.dispatch", &args).unwrap(),
            Exec::Dispatch(expr.into())
        );
        for bad in ["os.exit()", "hl.dsp.window.close()\nos.exit()"] {
            args.insert("expr".into(), bad.into());
            assert!(matches!(
                e.exec("window.dispatch", &args),
                Err(CatalogError::UnsafeArg { .. })
            ));
        }
    }

    #[test]
    fn route_check() {
        let c = Catalog::parse(
            "[a.b]\nlabel = \"B\"\nicon = \"x\"\nrun = [\"omarchy\", \"gone\", \"away\"]\n\
             [a.c]\nlabel = \"C\"\nicon = \"x\"\nrun = [\"systemctl\", \"suspend\"]\n\
             [a.d]\nlabel = \"D\"\nicon = \"x\"\nrun = [\"omarchy\", \"capture\", \"screenshot\", \"region\"]",
        )
        .unwrap();
        let routes = parse_routes(
            r#"{"ok":true,"commands":[{"route":"omarchy capture screenshot","routes":["omarchy capture screenshot"]}]}"#,
        )
        .unwrap();
        assert_eq!(
            c.unavailable(&routes).into_iter().collect::<Vec<_>>(),
            ["a.b"]
        );
    }
}
