//! Editor writes: changes single slots of a profile file with `toml_edit`,
//! so comments and formatting survive. Every result is parsed before it is
//! written; the daemon picks the file up through its live reload.

use std::path::{Path, PathBuf};

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

use crate::catalog::Catalog;
use crate::check;
use crate::config::{self, DIALS, KEYS, Loaded, Profile};

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("{0}")]
    Invalid(String),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] config::ConfigError),
}

fn invalid(msg: impl Into<String>) -> EditError {
    EditError::Invalid(msg.into())
}

/// A page (1-based) or folder of a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Page(usize),
    Folder(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Key,
    Dial,
}

/// One key or dial (1-based `index`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotRef {
    pub at: Location,
    pub kind: Kind,
    pub index: usize,
}

/// Sets a slot to `binding` (`{action, args?, label?, icon?}` as JSON), or
/// empties it with `None`.
pub fn set(
    src: &str,
    slot: &SlotRef,
    binding: Option<&serde_json::Value>,
) -> Result<String, EditError> {
    let mut doc = parse(src)?;
    let value = match binding {
        Some(b) => Value::InlineTable(binding_table(b)?),
        None => Value::InlineTable(InlineTable::new()),
    };
    put(&mut doc, slot, value)?;
    finish(doc)
}

/// Exchanges two slots, also across pages and folders (drag and drop).
pub fn swap(src: &str, a: &SlotRef, b: &SlotRef) -> Result<String, EditError> {
    if a.kind != b.kind {
        return Err(invalid("keys and dials cannot be swapped"));
    }
    let mut doc = parse(src)?;
    let va = take(&mut doc, a)?;
    let vb = take(&mut doc, b)?;
    put(&mut doc, a, vb)?;
    put(&mut doc, b, va)?;
    finish(doc)
}

/// Applies `change` to the file of profile `id` in config dir `dir`. The
/// built-in default profile gets its own file on the first edit. A change
/// that adds problems (unknown action, missing argument, …) is refused;
/// problems the file already had do not block it.
pub fn apply(
    dir: &Path,
    id: &str,
    catalog: &Catalog,
    change: impl FnOnce(&str) -> Result<String, EditError>,
) -> Result<PathBuf, EditError> {
    let profiles = dir.join("profiles");
    let path = profiles.join(format!("{id}.toml"));
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && id == config::DEFAULT_PROFILE_ID => {
            config::DEFAULT_PROFILE.to_owned()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(invalid(format!("unknown profile {id:?}")));
        }
        Err(source) => return Err(EditError::Io { path, source }),
    };
    let out = change(&src)?;
    let after = Profile::parse(&out, &path)?;
    let added = added_problems(dir, id, Profile::parse(&src, &path).ok(), after, catalog);
    if !added.is_empty() {
        return Err(invalid(added.join("\n")));
    }
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| EditError::Io { path, source }
    };
    std::fs::create_dir_all(&profiles).map_err(io(&profiles))?;
    // Same directory, so the rename is atomic and the watcher sees one change.
    let tmp = profiles.join(format!(".{id}.toml.tmp"));
    std::fs::write(&tmp, out).map_err(io(&tmp))?;
    std::fs::rename(&tmp, &path).map_err(io(&path))?;
    Ok(path)
}

/// Problems with `after` in place of `before` that were not there before.
fn added_problems(
    dir: &Path,
    id: &str,
    before: Option<Profile>,
    after: Profile,
    catalog: &Catalog,
) -> Vec<String> {
    // Other profiles matter for `structure.profile`; a broken config only
    // loses that part of the check.
    let mut loaded = Loaded::load(dir).unwrap_or_else(|_| Loaded {
        config: Default::default(),
        profiles: Default::default(),
    });
    let old = match before {
        Some(p) => {
            loaded.profiles.insert(id.to_owned(), p);
            check::problems(&loaded, catalog)
        }
        None => Vec::new(),
    };
    loaded.profiles.insert(id.to_owned(), after);
    check::problems(&loaded, catalog)
        .into_iter()
        .filter(|p| !old.contains(p))
        .collect()
}

fn parse(src: &str) -> Result<DocumentMut, EditError> {
    src.parse::<DocumentMut>()
        .map_err(|e| invalid(format!("profile is not valid TOML: {e}")))
}

fn finish(doc: DocumentMut) -> Result<String, EditError> {
    let out = doc.to_string();
    Profile::parse(&out, Path::new("profile"))?;
    Ok(out)
}

/// The `keys`/`dials` array of a page or folder, created when missing.
fn slots<'a>(doc: &'a mut DocumentMut, slot: &SlotRef) -> Result<&'a mut Array, EditError> {
    let max = match slot.kind {
        Kind::Key => KEYS,
        Kind::Dial => DIALS,
    };
    if slot.index == 0 || slot.index > max {
        return Err(invalid(format!("slot must be 1-{max}")));
    }
    let table: &mut Table = match &slot.at {
        Location::Page(n) => {
            let pages = doc
                .get_mut("pages")
                .and_then(Item::as_array_of_tables_mut)
                .ok_or_else(|| invalid("profile has no [[pages]]"))?;
            let count = pages.len();
            n.checked_sub(1)
                .and_then(|i| pages.get_mut(i))
                .ok_or_else(|| invalid(format!("page must be 1-{count}")))?
        }
        Location::Folder(f) => doc
            .get_mut("folders")
            .and_then(|t| t.get_mut(f))
            .and_then(Item::as_table_mut)
            .ok_or_else(|| invalid(format!("unknown folder {f:?}")))?,
    };
    let name = match slot.kind {
        Kind::Key => "keys",
        Kind::Dial => "dials",
    };
    let item = table
        .entry(name)
        .or_insert_with(|| Item::Value(Value::Array(Array::new())));
    item.as_array_mut()
        .ok_or_else(|| invalid(format!("`{name}` must be an inline array")))
}

/// Removes a slot's value, leaving `{}` in its place.
fn take(doc: &mut DocumentMut, slot: &SlotRef) -> Result<Value, EditError> {
    let arr = slots(doc, slot)?;
    let i = slot.index - 1;
    if i >= arr.len() {
        return Ok(Value::InlineTable(InlineTable::new()));
    }
    let empty = Value::InlineTable(InlineTable::new());
    Ok(arr.replace(i, empty))
}

fn put(doc: &mut DocumentMut, slot: &SlotRef, mut value: Value) -> Result<(), EditError> {
    let arr = slots(doc, slot)?;
    let i = slot.index - 1;
    while arr.len() <= i {
        arr.push(InlineTable::new());
    }
    // Keep the position's whitespace (one slot per line stays that way).
    let decor = arr.get(i).map(|v| v.decor().clone());
    if let Some(d) = decor {
        *value.decor_mut() = d;
    }
    arr.replace(i, value);
    // Trailing empty slots carry no information.
    while !arr.is_empty() && arr.get(arr.len() - 1).is_some_and(is_empty) {
        arr.remove(arr.len() - 1);
    }
    Ok(())
}

fn is_empty(v: &Value) -> bool {
    v.as_inline_table().is_some_and(InlineTable::is_empty)
}

fn binding_table(b: &serde_json::Value) -> Result<InlineTable, EditError> {
    let obj = b
        .as_object()
        .ok_or_else(|| invalid("binding must be a JSON object"))?;
    let mut t = InlineTable::new();
    for key in ["action", "args", "label", "icon"] {
        match obj.get(key) {
            None | Some(serde_json::Value::Null) => {}
            Some(serde_json::Value::Object(o)) if key == "args" && o.is_empty() => {}
            Some(serde_json::Value::String(s))
                if key != "action" && key != "args" && s.is_empty() => {}
            Some(v) => {
                t.insert(key, json_value(v)?);
            }
        }
    }
    if let Some(k) = obj
        .keys()
        .find(|k| !["action", "args", "label", "icon"].contains(&k.as_str()))
    {
        return Err(invalid(format!("unknown binding field {k:?}")));
    }
    if !t.contains_key("action") {
        return Err(invalid("binding needs an action"));
    }
    Ok(t)
}

fn json_value(v: &serde_json::Value) -> Result<Value, EditError> {
    Ok(match v {
        serde_json::Value::Bool(b) => Value::from(*b),
        serde_json::Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Value::from(i),
            (None, Some(f)) => Value::from(f),
            _ => return Err(invalid(format!("number out of range: {n}"))),
        },
        serde_json::Value::String(s) => Value::from(s.as_str()),
        serde_json::Value::Array(a) => {
            let mut arr = Array::new();
            for x in a {
                arr.push(json_value(x)?);
            }
            Value::Array(arr)
        }
        serde_json::Value::Object(o) => {
            let mut t = InlineTable::new();
            for (k, x) in o {
                t.insert(k, json_value(x)?);
            }
            Value::InlineTable(t)
        }
        serde_json::Value::Null => return Err(invalid("null is not a TOML value")),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const SRC: &str = r#"# My profile
name = "Dev"

[[pages]]
# Top row
keys = [
  { action = "system.lock" },   # lock it
  {},
  { action = "launcher.terminal" },
]
dials = [{ action = "media.volume" }]

[[pages]]
keys = [{ action = "system.menu" }]

[folders.tools]
keys = [{ action = "capture.qr" }]
"#;

    fn key(at: Location, index: usize) -> SlotRef {
        SlotRef {
            at,
            kind: Kind::Key,
            index,
        }
    }

    #[test]
    fn set_keeps_comments_and_layout() {
        let out = set(
            SRC,
            &key(Location::Page(1), 2),
            Some(&json!({ "action": "capture.screenshot", "args": { "mode": "region" }, "label": "" })),
        )
        .unwrap();
        insta::assert_snapshot!(out);
    }

    #[test]
    fn set_pads_and_clear_trims() {
        let out = set(
            SRC,
            &key(Location::Page(2), 4),
            Some(&json!({ "action": "system.lock" })),
        )
        .unwrap();
        assert!(
            out.contains(
                r#"keys = [{ action = "system.menu" }, {}, {}, { action = "system.lock" }]"#
            ),
            "{out}"
        );
        let back = set(&out, &key(Location::Page(2), 4), None).unwrap();
        assert!(
            back.contains(r#"keys = [{ action = "system.menu" }]"#),
            "{back}"
        );
    }

    #[test]
    fn swap_across_folder_and_page() {
        let out = swap(
            SRC,
            &key(Location::Page(1), 1),
            &key(Location::Folder("tools".into()), 1),
        )
        .unwrap();
        let p = Profile::parse(&out, Path::new("t")).unwrap();
        assert_eq!(p.pages[0].keys[0].0.as_ref().unwrap().action, "capture.qr");
        assert_eq!(
            p.folders["tools"].keys[0].0.as_ref().unwrap().action,
            "system.lock"
        );
        assert!(out.contains("# lock it"), "{out}");
    }

    #[test]
    fn creates_dials_and_rejects_bad_input() {
        let dial = SlotRef {
            at: Location::Page(2),
            kind: Kind::Dial,
            index: 1,
        };
        let out = set(SRC, &dial, Some(&json!({ "action": "media.mic" }))).unwrap();
        assert!(
            out.contains(r#"dials = [{ action = "media.mic" }]"#),
            "{out}"
        );
        assert!(set(SRC, &key(Location::Page(3), 1), None).is_err());
        assert!(set(SRC, &key(Location::Page(1), 9), None).is_err());
        assert!(set(SRC, &key(Location::Folder("x".into()), 1), None).is_err());
        assert!(
            set(
                SRC,
                &key(Location::Page(1), 1),
                Some(&json!({ "label": "x" }))
            )
            .is_err()
        );
        assert!(
            set(
                SRC,
                &key(Location::Page(1), 1),
                Some(&json!({ "action": "a", "nope": 1 }))
            )
            .is_err()
        );
        assert!(swap(SRC, &key(Location::Page(1), 1), &dial).is_err());
    }

    #[test]
    fn apply_copies_the_default_profile_and_writes_atomically() {
        let dir = std::env::temp_dir().join(format!("duckydeck-edit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let catalog = Catalog::builtin().unwrap();
        let path = apply(&dir, config::DEFAULT_PROFILE_ID, &catalog, |src| {
            set(
                src,
                &key(Location::Page(1), 1),
                Some(&json!({ "action": "system.lock" })),
            )
        })
        .unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("system.lock"));
        assert_eq!(std::fs::read_dir(dir.join("profiles")).unwrap().count(), 1);
        assert!(apply(&dir, "missing", &catalog, |s| Ok(s.to_owned())).is_err());
        let unknown = apply(&dir, config::DEFAULT_PROFILE_ID, &catalog, |src| {
            set(
                src,
                &key(Location::Page(1), 2),
                Some(&json!({ "action": "nope" })),
            )
        });
        assert!(unknown.unwrap_err().to_string().contains("unknown action"));
        // A change that breaks the profile is not written.
        let err = apply(&dir, config::DEFAULT_PROFILE_ID, &catalog, |_| {
            Ok("name = 1".into())
        });
        assert!(err.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), written);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
