//! Declarative command actions from `actions/catalog.toml`.
//!
//! Each entry is "run a command + show an icon" and is executed by
//! [`Entry::command`]. Actions with real logic are Rust code elsewhere.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Deserialize;

use crate::command::CommandSpec;

/// The built-in catalog, embedded at build time.
pub const CATALOG: &str = include_str!("../../../actions/catalog.toml");

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid catalog: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("action `{0}` has an empty `run`")]
    EmptyRun(String),
    #[error("action `{action}` needs argument `{arg}`")]
    MissingArg { action: String, arg: String },
    #[error("action `{action}`: argument `{arg}` must be a string, number or bool")]
    BadArg { action: String, arg: String },
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
    /// Icon shown while the state is unknown.
    pub fn default_name(&self) -> &str {
        match self {
            Self::Single(n) => n,
            Self::Toggle { on, .. } => on,
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
    pub run: Vec<String>,
    /// Omarchy route that must exist; default: derived from `run`.
    pub requires: Option<String>,
    pub confirm: Option<Confirm>,
    /// Placeholder values used when the binding does not set them.
    #[serde(default)]
    pub defaults: toml::Table,
}

impl Entry {
    /// Builds the command, filling `{name}` placeholders from `args`, then `defaults`.
    pub fn command(&self, id: &str, args: &toml::Table) -> Result<CommandSpec, CatalogError> {
        let mut parts = self.run.iter().map(|a| self.fill(id, a, args));
        let program = parts
            .next()
            .ok_or_else(|| CatalogError::EmptyRun(id.to_owned()))??;
        Ok(CommandSpec::new(program).args(parts.collect::<Result<Vec<_>, _>>()?))
    }

    fn fill(&self, id: &str, arg: &str, args: &toml::Table) -> Result<String, CatalogError> {
        let mut out = String::new();
        let mut rest = arg;
        while let Some(start) = rest.find('{') {
            let Some(len) = rest[start..].find('}') else {
                break;
            };
            let name = &rest[start + 1..start + len];
            let value = args
                .get(name)
                .or_else(|| self.defaults.get(name))
                .ok_or_else(|| CatalogError::MissingArg {
                    action: id.to_owned(),
                    arg: name.to_owned(),
                })?;
            out.push_str(&rest[..start]);
            out.push_str(&scalar(value).ok_or_else(|| CatalogError::BadArg {
                action: id.to_owned(),
                arg: name.to_owned(),
            })?);
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
        if let Some((id, _)) = entries.iter().find(|(_, e)| e.run.is_empty()) {
            return Err(CatalogError::EmptyRun(id.clone()));
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
            // Defaults complete every placeholder; the call goes out verbatim.
            let spec = e.command(id, &toml::Table::new()).unwrap();
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
        let spec = e.command("capture.screenshot", &args).unwrap();
        assert_eq!(spec.program, "omarchy");
        assert_eq!(spec.args, ["capture", "screenshot", "region; rm -rf ~"]);
        let spec = e
            .command("capture.screenshot", &toml::Table::new())
            .unwrap();
        assert_eq!(spec.args[2], "smart");
    }

    #[test]
    fn missing_and_bad_args() {
        let c = Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"lock\"\nrun = [\"a\", \"--n={n}\"]")
            .unwrap();
        let e = c.get("x.y").unwrap();
        assert!(matches!(
            e.command("x.y", &toml::Table::new()),
            Err(CatalogError::MissingArg { .. })
        ));
        let mut args = toml::Table::new();
        args.insert("n".into(), 3.into());
        assert_eq!(e.command("x.y", &args).unwrap().args, ["--n=3"]);
        args.insert("n".into(), toml::Value::Array(vec![]));
        assert!(matches!(
            e.command("x.y", &args),
            Err(CatalogError::BadArg { .. })
        ));
    }

    #[test]
    fn rejects_unknown_fields_and_empty_run() {
        assert!(
            Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"a\"\nrun = [\"a\"]\nfoo = 1").is_err()
        );
        assert!(Catalog::parse("[x.y]\nlabel = \"Y\"\nicon = \"a\"\nrun = []").is_err());
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
