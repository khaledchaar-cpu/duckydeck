//! Built-in icons from `assets/icons.toml`, validated and embedded at build
//! time. All are monochrome SVGs using `currentColor`.

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// SVG bytes of the built-in icon `name`, if it exists.
pub fn get(name: &str) -> Option<&'static [u8]> {
    ICONS
        .binary_search_by(|(n, _, _)| (*n).cmp(name))
        .ok()
        .map(|i| ICONS[i].2)
}

/// Names of all built-in icons, sorted.
pub fn names() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(n, _, _)| *n)
}

/// All built-in icons as (name, category, SVG), sorted by name. The
/// category is the folder under `assets/icons/` (`system`, `media`, …).
pub fn all() -> impl Iterator<Item = (&'static str, &'static str, &'static [u8])> {
    ICONS.iter().copied()
}

/// The SVG as a `data:` URL with `currentColor` replaced by `color`
/// (`#rrggbb`), for previews outside the daemon. `None` for other colors.
pub fn data_url(svg: &[u8], color: &str) -> Option<String> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let svg = String::from_utf8_lossy(svg).replace("currentColor", color);
    let mut out = String::from("data:image/svg+xml;utf8,");
    for b in svg.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b' '
            | b'='
            | b'/'
            | b':'
            | b',' => out.push(b as char),
            b'\n' | b'\r' | b'\t' => out.push(' '),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{KeyView, Renderer};
    use crate::theme::Theme;

    #[test]
    fn data_urls() {
        let url = data_url(
            br#"<svg stroke="currentColor"><path d="M1 2"/></svg>"#,
            "#aBc123",
        )
        .unwrap();
        assert_eq!(
            url,
            "data:image/svg+xml;utf8,%3Csvg stroke=%22%23aBc123%22%3E%3Cpath d=%22M1 2%22/%3E%3C/svg%3E"
        );
        assert_eq!(data_url(b"<svg/>", "red"), None);
        assert_eq!(data_url(b"<svg/>", "#12345"), None);
        assert!(all().any(|(n, c, _)| n == "volume" && c == "media"));
    }

    #[test]
    fn lookup() {
        assert!(get("volume").is_some());
        assert!(get("omarchy-menu").is_some());
        assert!(get("does-not-exist").is_none());
    }

    #[test]
    fn covers_minimum_set() {
        // Spot check one per category from docs/spec/assets.md.
        for name in [
            "lock",
            "ocr",
            "play-pause",
            "monitor",
            "vpn",
            "pop-out",
            "generic-app",
            "warning",
        ] {
            assert!(get(name).is_some(), "missing {name}");
        }
    }

    #[test]
    fn every_icon_renders_and_draws_pixels() {
        let theme = Theme::parse("background = \"#121212\"\nforeground = \"#bebebe\"").unwrap();
        let mut renderer = Renderer::new(Vec::new());
        let blank = renderer
            .key(&theme, &KeyView::default())
            .expect("blank key");
        for (name, _, svg) in ICONS {
            let view = KeyView {
                icon: Some(svg),
                ..KeyView::default()
            };
            let img = renderer.key(&theme, &view).expect(name);
            assert_ne!(img, blank, "{name} renders empty");
        }
    }
}

#[cfg(test)]
mod contrast_tests {
    use super::*;
    use crate::render::{KEY_SIZE, KeyView, Renderer};
    use crate::theme::{Color, MIN_CONTRAST, Mode, Role, Theme};

    const STOCK_THEMES: &str = "/usr/share/omarchy/themes";

    fn stock_themes() -> Vec<(String, Theme)> {
        let mut themes: Vec<_> = std::fs::read_dir(STOCK_THEMES)
            .unwrap()
            .filter_map(|e| {
                let dir = e.ok()?.path();
                let theme = Theme::load(&dir.join("colors.toml")).ok()?;
                Some((dir.file_name()?.to_string_lossy().into_owned(), theme))
            })
            .collect();
        themes.sort_by(|a, b| a.0.cmp(&b.0));
        themes
    }

    /// Measured on rendered pixels: the solid `omarchy-menu` glyph's
    /// strongest pixel must reach the minimum contrast against the key
    /// background, for every foreground/background role in every stock theme.
    #[test]
    fn icons_readable_in_all_stock_themes() {
        if !std::path::Path::new(STOCK_THEMES).is_dir() {
            eprintln!("skipped: no Omarchy themes (CI)");
            return;
        }
        let themes = stock_themes();
        assert!(themes.len() >= 10, "stock themes not found");
        assert!(themes.iter().any(|(_, t)| t.mode == Mode::Light));

        let mut renderer = Renderer::new(Vec::new());
        let fgs = [Role::Foreground, Role::Accent, Role::Muted, Role::Red];
        let bgs = [Role::Background, Role::LighterBackground];
        let mut failures = Vec::new();
        for (name, theme) in &themes {
            for bg in bgs {
                for fg in fgs {
                    let view = KeyView {
                        icon: get("omarchy-menu"),
                        fg,
                        bg,
                        ..KeyView::default()
                    };
                    let img = renderer.key(theme, &view).unwrap();
                    let [r, g, b] = img.pixel(0, 70);
                    let back = Color(r, g, b);
                    let best = (0..KEY_SIZE)
                        .flat_map(|y| (0..KEY_SIZE).map(move |x| (x, y)))
                        .map(|(x, y)| {
                            let [r, g, b] = img.pixel(x, y);
                            Color(r, g, b).contrast(back)
                        })
                        .fold(1.0_f32, f32::max);
                    if best < MIN_CONTRAST {
                        failures.push(format!("{name}: {fg:?} on {bg:?} = {best:.2}"));
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "low contrast:\n{}",
            failures.join("\n")
        );
    }
}
