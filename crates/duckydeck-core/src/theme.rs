//! Colors of the active Omarchy theme (`colors.toml`), the only color source.
//!
//! Stock themes define every token; user themes often only set `accent`,
//! `background` and `foreground`, so missing tokens are derived from those.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parse colors.toml: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid color {value:?} for {key}")]
    Color { key: &'static str, value: String },
    #[error("missing {0} in colors.toml")]
    Missing(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.trim().strip_prefix('#')?;
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        let ch = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Self(ch(0)?, ch(2)?, ch(4)?))
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// Linear blend: `t = 0` is `self`, `t = 1` is `other`.
    pub fn mix(self, other: Self, t: f32) -> Self {
        let m = |a: u8, b: u8| {
            let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t.clamp(0.0, 1.0);
            // Clamped to 0..=255 above, so the cast cannot truncate.
            v.round() as u8
        };
        Self(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }

    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let lin = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.0) + 0.7152 * lin(self.1) + 0.0722 * lin(self.2)
    }

    /// WCAG contrast ratio (1.0 ..= 21.0).
    pub fn contrast(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// `self` if it reaches `min` contrast on `bg`, otherwise the closest
    /// blend towards white or black (whichever contrasts more) that does.
    pub fn readable_on(self, bg: Self, min: f32) -> Self {
        if self.contrast(bg) >= min {
            return self;
        }
        let white = Self(255, 255, 255);
        let black = Self(0, 0, 0);
        let target = if white.contrast(bg) >= black.contrast(bg) {
            white
        } else {
            black
        };
        (1..=20)
            .map(|i| self.mix(target, i as f32 / 20.0))
            .find(|c| c.contrast(bg) >= min)
            .unwrap_or(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

/// Minimum contrast for icons and labels on keys.
pub const MIN_CONTRAST: f32 = 4.5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub mode: Mode,
    pub accent: Color,
    pub background: Color,
    pub lighter_background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub red: Color,
}

#[derive(Deserialize)]
struct Raw {
    mode: Option<String>,
    accent: Option<String>,
    background: Option<String>,
    lighter_background: Option<String>,
    foreground: Option<String>,
    muted: Option<String>,
    red: Option<String>,
}

impl Theme {
    /// `$XDG_STATE_HOME/omarchy/current/theme/colors.toml` (as the shell reads it).
    pub fn current_path() -> Option<PathBuf> {
        let state = std::env::var_os("XDG_STATE_HOME")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
        Some(state.join("omarchy/current/theme/colors.toml"))
    }

    pub fn load(path: &Path) -> Result<Self, ThemeError> {
        let text = std::fs::read_to_string(path).map_err(|source| ThemeError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, ThemeError> {
        let raw: Raw = toml::from_str(text)?;
        let color = |key: &'static str, v: &Option<String>| -> Result<Option<Color>, ThemeError> {
            v.as_deref()
                .map(|s| {
                    Color::parse(s).ok_or_else(|| ThemeError::Color {
                        key,
                        value: s.to_owned(),
                    })
                })
                .transpose()
        };
        let need =
            |key: &'static str, v: &Option<String>| color(key, v)?.ok_or(ThemeError::Missing(key));

        let background = need("background", &raw.background)?;
        let foreground = need("foreground", &raw.foreground)?;
        let accent = color("accent", &raw.accent)?.unwrap_or(foreground);
        let mode = match raw.mode.as_deref() {
            Some("light") => Mode::Light,
            Some(_) => Mode::Dark,
            None if background.luminance() > 0.5 => Mode::Light,
            None => Mode::Dark,
        };
        Ok(Self {
            mode,
            accent,
            background,
            lighter_background: color("lighter_background", &raw.lighter_background)?
                .unwrap_or_else(|| background.mix(foreground, 0.08)),
            foreground,
            muted: color("muted", &raw.muted)?.unwrap_or_else(|| background.mix(foreground, 0.5)),
            red: color("red", &raw.red)?.unwrap_or(Color(0xd3, 0x5f, 0x5f)),
        })
    }

    pub fn get(&self, role: Role) -> Color {
        match role {
            Role::Accent => self.accent,
            Role::Background => self.background,
            Role::LighterBackground => self.lighter_background,
            Role::Foreground => self.foreground,
            Role::Muted => self.muted,
            Role::Red => self.red,
        }
    }
}

/// Theme token used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Accent,
    Background,
    LighterBackground,
    Foreground,
    Muted,
    Red,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATTE_BLACK: &str = r##"
mode = "dark"
accent = "#e68e0d"
muted = "#333333"
background = "#121212"
lighter_background = "#1e1e1e"
foreground = "#bebebe"
red = "#D35F5F"
"##;

    #[test]
    fn parses_stock_theme() -> Result<(), ThemeError> {
        let t = Theme::parse(MATTE_BLACK)?;
        assert_eq!(t.mode, Mode::Dark);
        assert_eq!(t.accent, Color(0xe6, 0x8e, 0x0d));
        assert_eq!(t.muted, Color(0x33, 0x33, 0x33));
        assert_eq!(t.red, Color(0xd3, 0x5f, 0x5f));
        Ok(())
    }

    #[test]
    fn derives_missing_tokens() -> Result<(), ThemeError> {
        let t =
            Theme::parse("background = \"#ffffff\"\nforeground = \"#000000\"\ncolor0 = \"#000\"")?;
        assert_eq!(t.mode, Mode::Light);
        assert_eq!(t.accent, t.foreground);
        assert_eq!(t.muted, Color(0x80, 0x80, 0x80));
        Ok(())
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            Theme::parse("foreground = \"#fff\""),
            Err(ThemeError::Missing("background"))
        ));
        assert!(matches!(
            Theme::parse("background = \"red\"\nforeground = \"#000000\""),
            Err(ThemeError::Color { .. })
        ));
    }

    #[test]
    fn contrast_matches_wcag() {
        let (w, b) = (Color(255, 255, 255), Color(0, 0, 0));
        assert!((w.contrast(b) - 21.0).abs() < 0.01);
        assert!((w.contrast(w) - 1.0).abs() < 0.01);
    }

    #[test]
    fn readable_on_fixes_low_contrast() {
        let bg = Color(0x12, 0x12, 0x12);
        let muted = Color(0x33, 0x33, 0x33);
        assert!(muted.contrast(bg) < MIN_CONTRAST);
        let fixed = muted.readable_on(bg, MIN_CONTRAST);
        assert!(fixed.contrast(bg) >= MIN_CONTRAST);
        let light = Color(0xee, 0xee, 0xee);
        assert!(
            light
                .readable_on(Color(0xff, 0xff, 0xff), MIN_CONTRAST)
                .contrast(Color(255, 255, 255))
                >= MIN_CONTRAST
        );
        assert_eq!(
            Color(0xbe, 0xbe, 0xbe).readable_on(bg, MIN_CONTRAST),
            Color(0xbe, 0xbe, 0xbe)
        );
    }
}
