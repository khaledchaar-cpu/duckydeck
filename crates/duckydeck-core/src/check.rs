//! `duckydeck check`: validates the action ids and args of every binding
//! against the catalog and the built-in Rust actions. Syntax and structure
//! are already checked by [`Loaded::load`](crate::config::Loaded::load).

use crate::catalog::Catalog;
use crate::command::{CommandRunner, CommandSpec};
use crate::compound::{self, MULTI_ACTION, PROFILE_ACTION, Step, TOGGLE_ACTION};
use crate::config::{Binding, FOLDER_ACTION, Loaded, Page};
use crate::dial::Dial;
use crate::hypr::SCROLL_ACTION;
use crate::media::MediaKey;
use crate::nav::{BACK_ACTION, PAGE_ACTION};

/// One problem per line, e.g. `profile "dev", page 2, key 3: unknown action "x"`.
pub fn problems(loaded: &Loaded, catalog: &Catalog) -> Vec<String> {
    let mut out = Vec::new();
    for (id, profile) in &loaded.profiles {
        let pages = profile
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| (format!("page {}", i + 1), p));
        let folders = profile
            .folders
            .iter()
            .map(|(n, p)| (format!("folder {n:?}"), p));
        for (what, page) in pages.chain(folders) {
            check_page(loaded, catalog, page, &mut |slot, msg| {
                out.push(format!("profile {id:?}, {what}, {slot}: {msg}"));
            });
        }
    }
    out
}

/// Daemon side of the check: logs every problem and sends one shell
/// notification. The config is still used; broken slots just do nothing.
pub fn notify_problems(loaded: &Loaded, catalog: &Catalog, runner: &dyn CommandRunner) {
    let problems = problems(loaded, catalog);
    if problems.is_empty() {
        return;
    }
    for p in &problems {
        tracing::warn!(problem = %p, "config problem");
    }
    let mut body = problems[0].clone();
    if problems.len() > 1 {
        body.push_str(&format!(
            " (+{} more, see `duckydeck check`)",
            problems.len() - 1
        ));
    }
    let spec = CommandSpec::omarchy(["notification", "send", "--app-name", "DuckyDeck"])
        .args(["DuckyDeck config problem".to_owned(), body]);
    if let Err(e) = runner.spawn(&spec) {
        tracing::warn!(error = %e, "config problem notification failed");
    }
}

fn check_page(
    loaded: &Loaded,
    catalog: &Catalog,
    page: &Page,
    report: &mut impl FnMut(String, String),
) {
    for (i, b) in bindings(&page.keys) {
        if let Err(msg) = check_key(loaded, catalog, b) {
            report(format!("key {i}"), msg);
        }
    }
    for (i, b) in bindings(&page.dials) {
        if Dial::from_binding(b).is_none() && b.action != SCROLL_ACTION {
            report(format!("dial {i}"), not_a(b, "dial", catalog));
        }
    }
}

/// 1-based positions of the non-empty slots.
fn bindings(slots: &[crate::config::Slot]) -> impl Iterator<Item = (usize, &Binding)> {
    slots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.0.as_ref().map(|b| (i + 1, b)))
}

fn check_key(loaded: &Loaded, catalog: &Catalog, b: &Binding) -> Result<(), String> {
    let a = b.action.as_str();
    // Structure is validated by the parser.
    match a {
        PROFILE_ACTION => {
            let p = compound::profile(b)?;
            return match loaded.profiles.contains_key(p) {
                true => Ok(()),
                false => Err(format!("unknown profile {p:?}")),
            };
        }
        MULTI_ACTION => {
            for s in compound::steps(b)? {
                if let Step::Run(n) = s {
                    check_key(loaded, catalog, &n)?;
                }
            }
            return Ok(());
        }
        TOGGLE_ACTION => {
            for n in compound::states(b)? {
                check_key(loaded, catalog, &n)?;
            }
            return Ok(());
        }
        _ => {}
    }
    if MediaKey::from_binding(b).is_some() || [FOLDER_ACTION, BACK_ACTION, PAGE_ACTION].contains(&a)
    {
        return Ok(());
    }
    match catalog.get(a) {
        Some(entry) => entry
            .exec(a, &b.args)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        None => Err(not_a(b, "key", catalog)),
    }
}

fn not_a(b: &Binding, slot: &str, catalog: &Catalog) -> String {
    let known = catalog.get(&b.action).is_some()
        || Dial::from_binding(b).is_some()
        || b.action == SCROLL_ACTION
        || MediaKey::from_binding(b).is_some()
        || b.action.starts_with("structure.");
    if known {
        format!("{:?} cannot be used on a {slot}", b.action)
    } else {
        format!("unknown action {:?}", b.action)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::config::Profile;

    fn loaded(src: &str) -> Loaded {
        let mut l = Loaded {
            config: Default::default(),
            profiles: Default::default(),
        };
        let p = Profile::parse(src, Path::new("t.toml"));
        l.profiles.insert("t".into(), p.unwrap());
        l
    }

    #[test]
    fn default_profile_is_clean() {
        let catalog = Catalog::builtin().unwrap();
        let mut l = loaded("name = \"x\"\n[[pages]]");
        l.profiles
            .insert("omarchy".into(), Profile::default_profile().unwrap());
        assert_eq!(problems(&l, &catalog), Vec::<String>::new());
    }

    #[test]
    fn reports_unknown_and_misplaced_actions() {
        let catalog = Catalog::builtin().unwrap();
        let l = loaded(
            r#"
            name = "T"
            [[pages]]
            keys = [{}, { action = "nope" }, { action = "media.volume" }, { action = "system.lock" },
              { action = "structure.profile", args = { profile = "gone" } },
              { action = "structure.multi", args = { steps = [{ action = "system.lock" }, { action = "nope2" }] } }]
            dials = [{ action = "media.volume" }, { action = "system.lock" }]
            "#,
        );
        assert_eq!(
            problems(&l, &catalog),
            [
                r#"profile "t", page 1, key 2: unknown action "nope""#,
                r#"profile "t", page 1, key 3: "media.volume" cannot be used on a key"#,
                r#"profile "t", page 1, key 5: unknown profile "gone""#,
                r#"profile "t", page 1, key 6: unknown action "nope2""#,
                r#"profile "t", page 1, dial 2: "system.lock" cannot be used on a dial"#,
            ]
        );
    }

    #[test]
    fn notifies_once_with_count() {
        let catalog = Catalog::builtin().unwrap();
        let l = loaded(
            r#"
            name = "T"
            [[pages]]
            keys = [{ action = "nope" }, { action = "nope2" }]
            "#,
        );
        let runner = crate::command::RecordingRunner::new();
        notify_problems(&l, &catalog, &runner);
        assert_eq!(
            runner.command_lines(),
            [
                r#"omarchy notification send --app-name DuckyDeck DuckyDeck config problem profile "t", page 1, key 1: unknown action "nope" (+1 more, see `duckydeck check`)"#
            ]
        );
        let clean = loaded("name = \"x\"\n[[pages]]");
        let runner = crate::command::RecordingRunner::new();
        notify_problems(&clean, &catalog, &runner);
        assert!(runner.command_lines().is_empty());
    }
}
