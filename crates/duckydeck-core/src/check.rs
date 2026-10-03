//! `duckydeck check`: validates the action ids and args of every binding
//! against the catalog and the built-in Rust actions. Syntax and structure
//! are already checked by [`Loaded::load`](crate::config::Loaded::load).

use crate::catalog::Catalog;
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
            check_page(catalog, page, &mut |slot, msg| {
                out.push(format!("profile {id:?}, {what}, {slot}: {msg}"));
            });
        }
    }
    out
}

fn check_page(catalog: &Catalog, page: &Page, report: &mut impl FnMut(String, String)) {
    for (i, b) in bindings(&page.keys) {
        if let Err(msg) = check_key(catalog, b) {
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

fn check_key(catalog: &Catalog, b: &Binding) -> Result<(), String> {
    let a = b.action.as_str();
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
            keys = [{}, { action = "nope" }, { action = "media.volume" }, { action = "system.lock" }]
            dials = [{ action = "media.volume" }, { action = "system.lock" }]
            "#,
        );
        assert_eq!(
            problems(&l, &catalog),
            [
                r#"profile "t", page 1, key 2: unknown action "nope""#,
                r#"profile "t", page 1, key 3: "media.volume" cannot be used on a key"#,
                r#"profile "t", page 1, dial 2: "system.lock" cannot be used on a dial"#,
            ]
        );
    }
}
