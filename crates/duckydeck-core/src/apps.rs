//! Installed applications from XDG desktop entries, for the `app`
//! parameter of `launcher.app` (desktop-entry id, as `gtk-launch` takes it).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct App {
    /// Desktop-entry id without `.desktop`, e.g. `com.mitchellh.ghostty`.
    pub id: String,
    pub name: String,
    /// `StartupWMClass`: the window class when it differs from the id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wm_class: Option<String>,
}

/// Application directories in lookup order: `$XDG_DATA_HOME`, then `$XDG_DATA_DIRS`.
pub fn dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    home.into_iter()
        .chain(data_dirs.split(':').map(PathBuf::from))
        .map(|d| d.join("applications"))
        .collect()
}

/// Visible applications, sorted by name. An id found in an earlier
/// directory shadows later ones, also when it is hidden there.
pub fn scan(dirs: &[PathBuf]) -> Vec<App> {
    let mut seen = BTreeSet::new();
    let mut apps = Vec::new();
    for dir in dirs {
        let mut files = Vec::new();
        collect(dir, dir, &mut files);
        files.sort();
        for (id, path) in files {
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some((name, wm_class)) = parse(&text) {
                apps.push(App { id, name, wm_class });
            }
        }
    }
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    apps
}

/// Desktop files below `dir` with their id (subdirectories joined by `-`).
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if let Some(rel) = path
            .strip_prefix(root)
            .ok()
            .and_then(|r| r.to_str())
            .and_then(|r| r.strip_suffix(".desktop"))
        {
            out.push((rel.replace('/', "-"), path));
        }
    }
}

/// Name and window class of a shown application entry, `None` if it is
/// hidden or no app.
fn parse(text: &str) -> Option<(String, Option<String>)> {
    let mut in_entry = false;
    let mut name = None;
    let mut wm_class = None;
    let mut is_app = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match (key.trim(), value.trim()) {
            ("Name", v) => name = Some(v.to_owned()),
            ("StartupWMClass", v) if !v.is_empty() => wm_class = Some(v.to_owned()),
            ("Type", v) => is_app = v == "Application",
            ("NoDisplay" | "Hidden", "true") => return None,
            _ => {}
        }
    }
    name.filter(|n| is_app && !n.is_empty())
        .map(|n| (n, wm_class))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    const GHOSTTY: &str = "[Desktop Entry]\nType=Application\nName=Ghostty\nName[de]=Geist\n\n[Desktop Action new]\nName=New Window\n";

    #[test]
    fn parses_shown_apps_only() {
        assert_eq!(parse(GHOSTTY), Some(("Ghostty".into(), None)));
        assert_eq!(
            parse("[Desktop Entry]\nType=Application\nName=X\nStartupWMClass=x-app\n"),
            Some(("X".into(), Some("x-app".into())))
        );
        assert_eq!(
            parse("[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\n"),
            None
        );
        assert_eq!(parse("[Desktop Entry]\nType=Link\nName=X\n"), None);
        assert_eq!(parse("[Desktop Entry]\nType=Application\n"), None);
    }

    #[test]
    fn scans_with_shadowing_and_subdir_ids() {
        let tmp = std::env::temp_dir().join(format!("dd-apps-{}", std::process::id()));
        let (user, system) = (tmp.join("user"), tmp.join("system"));
        write(&system, "com.mitchellh.ghostty.desktop", GHOSTTY);
        write(
            &system,
            "kde/calc.desktop",
            "[Desktop Entry]\nType=Application\nName=calc\n",
        );
        write(
            &system,
            "hidden.desktop",
            "[Desktop Entry]\nType=Application\nName=Hidden\n",
        );
        write(&user, "hidden.desktop", "[Desktop Entry]\nHidden=true\n");
        write(&system, "notes.txt", "x");
        let apps = scan(&[user, system]);
        std::fs::remove_dir_all(&tmp).unwrap();
        assert_eq!(
            apps,
            vec![
                App {
                    id: "kde-calc".into(),
                    name: "calc".into(),
                    wm_class: None,
                },
                App {
                    id: "com.mitchellh.ghostty".into(),
                    name: "Ghostty".into(),
                    wm_class: None,
                },
            ]
        );
    }
}
