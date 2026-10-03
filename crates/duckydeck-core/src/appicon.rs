//! App icons for `launcher.app` keys: the desktop entry's `Icon=` looked up
//! in the icon theme of the Omarchy theme (`icons.theme`), its `Inherits`
//! chain, `hicolor` and `pixmaps`. Simplified XDG lookup: the largest icon
//! wins, scalable before raster.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::theme::Theme;

/// Smallest raster size worth showing on a 96 px key.
const MIN_SIZE: u32 = 32;

/// SVG or PNG bytes of the icon of desktop entry `app`.
pub fn load(app: &str) -> Option<Vec<u8>> {
    let name = crate::apps::icon_name(&crate::apps::dirs(), app)?;
    let path = if Path::new(&name).is_absolute() {
        PathBuf::from(name)
    } else {
        find(&bases(), current_theme().as_deref(), &name)?
    };
    std::fs::read(path).ok()
}

/// Icon theme name from `icons.theme` next to the current `colors.toml`.
fn current_theme() -> Option<String> {
    let path = Theme::current_path()?.with_file_name("icons.theme");
    let name = std::fs::read_to_string(path).ok()?.trim().to_owned();
    (!name.is_empty()).then_some(name)
}

/// Icon base directories: `$XDG_DATA_HOME/icons`, `~/.icons`, `$XDG_DATA_DIRS/icons`.
fn bases() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    data_home
        .map(|d| d.join("icons"))
        .into_iter()
        .chain(home.map(|h| h.join(".icons")))
        .chain(data_dirs.split(':').map(|d| PathBuf::from(d).join("icons")))
        .collect()
}

/// The best file for icon `name` in `theme` (or its parents), then
/// `hicolor`, then `pixmaps` beside the base directories.
fn find(bases: &[PathBuf], theme: Option<&str>, name: &str) -> Option<PathBuf> {
    let mut queue: VecDeque<String> = theme.map(str::to_owned).into_iter().collect();
    let mut seen = Vec::new();
    while let Some(t) = queue.pop_front() {
        if seen.contains(&t) {
            continue;
        }
        if let Some(path) = best_in(bases, &t, name) {
            return Some(path);
        }
        queue.extend(inherits(bases, &t));
        seen.push(t);
    }
    if !seen.iter().any(|t| t == "hicolor")
        && let Some(path) = best_in(bases, "hicolor", name)
    {
        return Some(path);
    }
    bases.iter().find_map(|b| {
        let pixmaps = b.parent()?.join("pixmaps");
        ["svg", "png"]
            .iter()
            .map(|ext| pixmaps.join(format!("{name}.{ext}")))
            .find(|p| p.is_file())
    })
}

/// Largest `<size>/apps/<name>.{svg,png}` of `theme` across the base dirs.
fn best_in(bases: &[PathBuf], theme: &str, name: &str) -> Option<PathBuf> {
    let mut best: Option<(u32, PathBuf)> = None;
    for dir in bases.iter().map(|b| b.join(theme)) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let sub = entry.file_name();
            let Some(score) = sub.to_str().and_then(size_score) else {
                continue;
            };
            if best.as_ref().is_some_and(|(s, _)| *s >= score) {
                continue;
            }
            if let Some(path) = ["svg", "png"]
                .iter()
                .map(|ext| entry.path().join("apps").join(format!("{name}.{ext}")))
                .find(|p| p.is_file())
            {
                best = Some((score, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Ranks a theme subdirectory: `scalable` first, else the pixel size
/// (`48x48@2x` counts 96); symbolic and tiny sizes are skipped.
fn size_score(dir: &str) -> Option<u32> {
    if dir == "scalable" {
        return Some(u32::MAX);
    }
    let (size, scale) = match dir.split_once('@') {
        Some((size, scale)) => (size, scale.trim_end_matches('x').parse().ok()?),
        None => (dir, 1),
    };
    let (w, h) = size.split_once('x')?;
    let px = w.parse::<u32>().ok().filter(|w| h.parse() == Ok(*w))? * scale;
    (px >= MIN_SIZE).then_some(px)
}

/// `Inherits=` of the theme's `index.theme`.
fn inherits(bases: &[PathBuf], theme: &str) -> Vec<String> {
    bases
        .iter()
        .find_map(|b| std::fs::read_to_string(b.join(theme).join("index.theme")).ok())
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.trim().strip_prefix("Inherits=").map(str::to_owned))
        })
        .map(|v| {
            v.split(',')
                .map(|t| t.trim().to_owned())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn scores_directories() {
        assert_eq!(size_score("scalable"), Some(u32::MAX));
        assert_eq!(size_score("256x256"), Some(256));
        assert_eq!(size_score("48x48@2x"), Some(96));
        assert_eq!(size_score("128x128@2"), Some(256));
        assert_eq!(size_score("16x16"), None);
        assert_eq!(size_score("symbolic"), None);
        assert_eq!(size_score("scalable-max-32"), None);
    }

    #[test]
    fn follows_inherits_then_hicolor_then_pixmaps() {
        let tmp = std::env::temp_dir().join(format!("dd-appicon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let base = tmp.join("share/icons");
        touch(
            &base.join("Child/index.theme"),
            "[Icon Theme]\nInherits=Parent,hicolor\n",
        );
        touch(&base.join("Parent/24x24/apps/a.png"), "");
        touch(&base.join("Parent/256x256/apps/a.png"), "");
        touch(&base.join("Child/16x16/apps/a.png"), "");
        touch(&base.join("hicolor/scalable/apps/b.svg"), "");
        touch(&tmp.join("share/pixmaps/c.png"), "");
        let bases = [base.clone()];
        assert_eq!(
            find(&bases, Some("Child"), "a"),
            Some(base.join("Parent/256x256/apps/a.png"))
        );
        assert_eq!(
            find(&bases, Some("Child"), "b"),
            Some(base.join("hicolor/scalable/apps/b.svg"))
        );
        assert_eq!(
            find(&bases, None, "c"),
            Some(tmp.join("share/pixmaps/c.png"))
        );
        assert_eq!(find(&bases, Some("Child"), "d"), None);
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
