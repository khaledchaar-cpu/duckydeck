//! Temporary demo screen until profiles exist (M4): keys are drawn by the
//! real renderer with theme colors; pressed keys switch to the lighter
//! background, long-pressed ones turn red. Strip segments show a fake level
//! per encoder (twist to change it).

use anyhow::{Context, Result};
use duckydeck_core::render::{KeyView, Renderer, SegmentView};
use duckydeck_core::theme::{Role, Theme};
use image::RgbImage;

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

    /// Strip segment above encoder `seg`: icon, level in percent, bar.
    fn segment(&mut self, deck: &mut Deck, seg: u8, state: KeyState) -> Result<()> {
        let level = deck.levels.get(usize::from(seg)).copied().unwrap_or(50);
        let text = format!("{level}%");
        let view = SegmentView {
            icon: Some(DEMO_ICON),
            text: Some(&text),
            level: Some(f32::from(level) / 100.0),
            fg: match state {
                KeyState::Long => Role::Red,
                _ => Role::Accent,
            },
            bg: match state {
                KeyState::Up => Role::Background,
                _ => Role::LighterBackground,
            },
        };
        let img = self.renderer.segment(&self.theme, &view)?;
        let img = RgbImage::from_raw(img.width, img.height, img.data)
            .context("renderer returned a malformed image")?;
        debug_assert_eq!(img.height(), STRIP_H);
        deck.out.set_strip(u16::from(seg) * 200, &img)
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
            p.segment(deck, encoder, KeyState::Up)
        }
        Gesture::Down(Control::Encoder(i)) => p.segment(deck, i, KeyState::Down),
        Gesture::LongPress(Control::Encoder(i)) => p.segment(deck, i, KeyState::Long),
        Gesture::Up(Control::Encoder(i)) => p.segment(deck, i, KeyState::Up),
        // Strip: touched segment lights up (tap) or turns red (long press);
        // a swipe highlights all segments.
        Gesture::StripTap(x, _) => p.segment(deck, segment(x), KeyState::Down),
        Gesture::StripLongPress(x, _) => p.segment(deck, segment(x), KeyState::Long),
        Gesture::StripSwipe(..) => (0..4).try_for_each(|seg| p.segment(deck, seg, KeyState::Down)),
        _ => return,
    };
    if let Err(e) = res.and_then(|()| deck.out.flush()) {
        tracing::warn!(error = %e, "test pattern update failed");
    }
}

fn segment(x: u16) -> u8 {
    u8::try_from((x / 200).min(3)).unwrap_or(3)
}

pub fn draw_strip(p: &mut Painter, deck: &mut Deck) -> Result<()> {
    (0..4u8).try_for_each(|seg| p.segment(deck, seg, KeyState::Up))
}
