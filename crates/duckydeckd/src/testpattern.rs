//! Temporary demo screen until profiles exist (M4): keys are drawn by the
//! real renderer with theme colors; pressed keys switch to the lighter
//! background, long-pressed ones turn red. The strip is still a plain level
//! bar per encoder until the strip renderer lands.

use anyhow::{Context, Result};
use duckydeck_core::render::{KeyView, Renderer};
use duckydeck_core::theme::{Role, Theme};
use image::{Rgb, RgbImage};

use crate::device::Deck;
use crate::gesture::{Control, Gesture};
use crate::surface::{KEY_SIZE, STRIP_H};

const DEMO_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v9"/><path d="M6 6a9 9 0 1 0 12 0"/></svg>"#;
const LABELS: [&str; 8] = [
    "Power",
    "",
    "Label",
    "Long label text",
    "Accent",
    "Muted",
    "Red",
    "Icon only",
];
const FALLBACK_THEME: &str =
    "background = \"#121212\"\nforeground = \"#bebebe\"\naccent = \"#e68e0d\"";

pub struct Painter {
    renderer: Renderer,
    theme: Theme,
}

impl Painter {
    pub fn new(font: Option<Vec<u8>>) -> Result<Self> {
        let theme = match Theme::current_path().map(|p| Theme::load(&p)) {
            Some(Ok(t)) => t,
            other => {
                tracing::warn!(error = ?other.map(|r| r.err()), "theme unavailable, using fallback");
                Theme::parse(FALLBACK_THEME).context("fallback theme")?
            }
        };
        Ok(Self {
            renderer: Renderer::new(font.into_iter().collect()),
            theme,
        })
    }

    fn key(&mut self, deck: &mut Deck, i: u8, state: KeyState) -> Result<()> {
        let idx = usize::from(i % 8);
        let fg = match (state, idx) {
            (KeyState::Long, _) => Role::Red,
            (_, 4) => Role::Accent,
            (_, 5) => Role::Muted,
            (_, 6) => Role::Red,
            _ => Role::Foreground,
        };
        let view = KeyView {
            icon: (idx != 1).then_some(DEMO_ICON),
            label: (idx != 7).then_some(LABELS[idx]).filter(|l| !l.is_empty()),
            fg,
            bg: match state {
                KeyState::Up => Role::Background,
                _ => Role::LighterBackground,
            },
        };
        let img = self.renderer.key(&self.theme, &view)?;
        let img = RgbImage::from_raw(img.width, img.height, img.data)
            .context("renderer returned a malformed image")?;
        debug_assert_eq!(img.width(), KEY_SIZE);
        deck.out.set_key(i, &img)
    }

    fn rgb(&self, role: Role) -> Rgb<u8> {
        let c = self.theme.get(role);
        Rgb([c.0, c.1, c.2])
    }
}

#[derive(Clone, Copy)]
enum KeyState {
    Up,
    Down,
    Long,
}

pub fn draw(p: &mut Painter, deck: &mut Deck) -> Result<()> {
    deck.out.set_brightness(60)?;
    for i in 0..8 {
        p.key(deck, i, KeyState::Up)?;
    }
    draw_strip(p, deck)?;
    deck.out.flush()
}

pub fn on_gesture(p: &mut Painter, deck: &mut Deck, g: Gesture) {
    let res = match g {
        Gesture::Down(Control::Key(i)) => p.key(deck, i, KeyState::Down),
        Gesture::Up(Control::Key(i)) => p.key(deck, i, KeyState::Up),
        Gesture::LongPress(Control::Key(i)) => p.key(deck, i, KeyState::Long),
        Gesture::Twist { encoder, delta, .. } => {
            let Some(level) = deck.levels.get_mut(usize::from(encoder)) else {
                return;
            };
            *level = level
                .saturating_add_signed(delta.saturating_mul(5))
                .min(100);
            let level = *level;
            draw_segment(p, deck, encoder, level, None)
        }
        Gesture::Down(Control::Encoder(i)) => {
            draw_segment(p, deck, i, 0, Some(p.rgb(Role::Foreground)))
        }
        Gesture::LongPress(Control::Encoder(i)) => {
            draw_segment(p, deck, i, 0, Some(p.rgb(Role::Muted)))
        }
        Gesture::Up(Control::Encoder(i)) => {
            let level = deck.levels.get(usize::from(i)).copied().unwrap_or(50);
            draw_segment(p, deck, i, level, None)
        }
        // Strip: touched segment turns white (tap) or gray (long press);
        // a swipe fills the whole strip, blue to the left, orange to the right.
        Gesture::StripTap(x, _) => {
            draw_segment(p, deck, segment(x), 0, Some(p.rgb(Role::Foreground)))
        }
        Gesture::StripLongPress(x, _) => {
            draw_segment(p, deck, segment(x), 0, Some(p.rgb(Role::Muted)))
        }
        Gesture::StripSwipe((x0, _), (x1, _)) => {
            let c = p.rgb(if x1 < x0 { Role::Accent } else { Role::Red });
            (0..4).try_for_each(|seg| draw_segment(p, deck, seg, 0, Some(c)))
        }
        _ => return,
    };
    if let Err(e) = res.and_then(|()| deck.out.flush()) {
        tracing::warn!(error = %e, "test pattern update failed");
    }
}

fn segment(x: u16) -> u8 {
    u8::try_from((x / 200).min(3)).unwrap_or(3)
}

pub fn draw_strip(p: &Painter, deck: &mut Deck) -> Result<()> {
    for seg in 0..4u8 {
        draw_segment(p, deck, seg, 50, None)?;
    }
    Ok(())
}

/// One strip segment above an encoder: colored bar showing `level` (0-100),
/// or filled with `solid` while the encoder is held.
fn draw_segment(
    p: &Painter,
    deck: &mut Deck,
    seg: u8,
    level: u8,
    solid: Option<Rgb<u8>>,
) -> Result<()> {
    let fill = u32::from(level) * 2;
    let (bar, bg) = (p.rgb(Role::Accent), p.rgb(Role::LighterBackground));
    let img = RgbImage::from_fn(200, STRIP_H, |x, _| match solid {
        Some(c) => c,
        None if x < fill => bar,
        None => bg,
    });
    deck.out.set_strip(u16::from(seg) * 200, &img)
}
