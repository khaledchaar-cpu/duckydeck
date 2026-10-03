//! Every action the editor offers: catalog entries plus the built-in Rust
//! actions, with label, icon, slot kind and parameters.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::catalog::{Catalog, Confirm};

/// Where an action can be placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Key,
    Dial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    /// Used when the binding does not set it; `None` = required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<toml::Value>,
    /// Optional parameters may be left out even without a default.
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    /// First part of the id: `system`, `capture`, `media`, …
    pub group: String,
    pub label: String,
    pub icon: String,
    pub slot: Slot,
    pub params: Vec<Param>,
    pub long_press: bool,
    /// `false` if the omarchy route is missing on this system.
    pub available: bool,
}

/// Built-in Rust actions: id, label, icon, slot, params (name, default, optional).
type Builtin = (
    &'static str,
    &'static str,
    &'static str,
    Slot,
    &'static [(&'static str, Option<i64>, bool)],
);

const BUILTINS: &[Builtin] = &[
    (
        "media.play_pause",
        "Play/Pause",
        "play",
        Slot::Key,
        &[("player", None, true)],
    ),
    (
        "media.next",
        "Next",
        "next",
        Slot::Key,
        &[("player", None, true)],
    ),
    (
        "media.previous",
        "Previous",
        "previous",
        Slot::Key,
        &[("player", None, true)],
    ),
    (
        "media.volume",
        "Volume",
        "volume",
        Slot::Dial,
        &[("step", Some(5), false)],
    ),
    ("media.mic", "Microphone", "mic", Slot::Dial, &[]),
    (
        "display.brightness",
        "Brightness",
        "brightness",
        Slot::Dial,
        &[("step", Some(5), false)],
    ),
    (
        "window.workspace_scroll",
        "Workspaces",
        "workspace-scroll",
        Slot::Dial,
        &[],
    ),
    (
        "structure.folder",
        "Folder",
        "folder",
        Slot::Key,
        &[("folder", None, false)],
    ),
    ("structure.back", "Back", "back", Slot::Key, &[]),
    (
        "structure.page",
        "Page",
        "page-next",
        Slot::Key,
        &[("n", None, true), ("to", None, true)],
    ),
    (
        "structure.profile",
        "Profile",
        "profile",
        Slot::Key,
        &[("profile", None, false)],
    ),
    (
        "structure.multi",
        "Multi action",
        "multi-action",
        Slot::Key,
        &[("steps", None, false)],
    ),
    (
        "structure.toggle",
        "Toggle",
        "toggle",
        Slot::Key,
        &[("states", None, false)],
    ),
];

/// All actions, sorted by id. `unavailable` comes from the daemon's route check.
pub fn items(catalog: &Catalog, unavailable: &BTreeSet<String>) -> Vec<Item> {
    let catalog_items = catalog.iter().map(|(id, e)| Item {
        id: id.clone(),
        group: group(id),
        label: e.label.clone(),
        icon: e.icon.name(Some(false)).to_owned(),
        slot: Slot::Key,
        params: e
            .placeholders()
            .into_iter()
            .map(|name| Param {
                default: e.defaults.get(&name).cloned(),
                name,
                optional: false,
            })
            .collect(),
        long_press: e.confirm == Some(Confirm::LongPress),
        available: !unavailable.contains(id),
    });
    let builtins = BUILTINS
        .iter()
        .map(|&(id, label, icon, slot, params)| Item {
            id: id.to_owned(),
            group: group(id),
            label: label.to_owned(),
            icon: icon.to_owned(),
            slot,
            params: params
                .iter()
                .map(|&(name, default, optional)| Param {
                    name: name.to_owned(),
                    default: default.map(toml::Value::Integer),
                    optional,
                })
                .collect(),
            long_press: false,
            available: true,
        });
    let mut all: Vec<Item> = catalog_items.chain(builtins).collect();
    all.sort_by(|a, b| a.id.cmp(&b.id));
    all
}

fn group(id: &str) -> String {
    id.split('.').next().unwrap_or(id).to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::config::{Loaded, Profile};
    use crate::icons;

    /// Every item with its defaults (and a sample for required params) is
    /// accepted by `duckydeck check` on the slot it claims.
    #[test]
    fn items_pass_the_check() {
        let catalog = Catalog::builtin().unwrap();
        let items = items(&catalog, &BTreeSet::new());
        let mut keys = Vec::new();
        let mut dials = Vec::new();
        for it in &items {
            assert!(
                icons::get(&it.icon).is_some(),
                "{}: icon {}",
                it.id,
                it.icon
            );
            let args: Vec<String> = it
                .params
                .iter()
                .filter(|p| p.default.is_none() && !p.optional)
                .map(|p| format!("{} = {}", p.name, sample(&it.id, &p.name)))
                .collect();
            let b = format!(
                "{{ action = {:?}, args = {{ {} }} }}",
                it.id,
                args.join(", ")
            );
            match it.slot {
                Slot::Key => keys.push(b),
                Slot::Dial => dials.push(b),
            }
        }
        let mut src = String::from("name = \"T\"\n[folders.f]\n");
        for (i, chunk) in keys.chunks(8).enumerate() {
            let dial = dials
                .get(i)
                .map_or(String::new(), |d| format!("dials = [{d}]"));
            src.push_str(&format!(
                "[[pages]]\nkeys = [{}]\n{dial}\n",
                chunk.join(", ")
            ));
        }
        for d in dials.iter().skip(keys.len().div_ceil(8)) {
            src.push_str(&format!("[[pages]]\ndials = [{d}]\n"));
        }
        let mut l = Loaded {
            config: Default::default(),
            profiles: Default::default(),
        };
        l.profiles.insert(
            "t".into(),
            Profile::parse(&src, Path::new("t.toml")).unwrap(),
        );
        assert_eq!(crate::check::problems(&l, &catalog), Vec::<String>::new());
        assert!(items.iter().any(|i| i.id == "system.lock"));
        assert!(
            items
                .iter()
                .any(|i| i.id == "media.volume" && i.slot == Slot::Dial)
        );
    }

    fn sample(id: &str, param: &str) -> &'static str {
        match (id, param) {
            (_, "folder") => "\"f\"",
            (_, "profile") => "\"t\"",
            (_, "steps") => "[{ action = \"system.lock\" }]",
            (_, "states") => "[{ action = \"system.lock\" }, { action = \"system.dnd\" }]",
            ("launcher.app", "app") => "\"omacalc\"",
            ("window.dispatch", "expr") => "'hl.dsp.focus({ workspace = \"1\" })'",
            _ => "\"1\"",
        }
    }

    #[test]
    fn unavailable_is_marked() {
        let catalog = Catalog::builtin().unwrap();
        let off = BTreeSet::from(["system.lock".to_owned()]);
        let items = items(&catalog, &off);
        let lock = items.iter().find(|i| i.id == "system.lock").unwrap();
        assert!(!lock.available);
        assert_eq!(lock.group, "system");
    }
}
