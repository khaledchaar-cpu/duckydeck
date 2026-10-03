//! Built-in icons from `assets/icons.toml`, validated and embedded at build
//! time. All are monochrome SVGs using `currentColor`.

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// SVG bytes of the built-in icon `name`, if it exists.
pub fn get(name: &str) -> Option<&'static [u8]> {
    ICONS
        .binary_search_by(|(n, _)| (*n).cmp(name))
        .ok()
        .map(|i| ICONS[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{KeyView, Renderer};
    use crate::theme::Theme;

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
        for (name, svg) in ICONS {
            let view = KeyView {
                icon: Some(svg),
                ..KeyView::default()
            };
            let img = renderer.key(&theme, &view).expect(name);
            assert_ne!(img, blank, "{name} renders empty");
        }
    }
}
