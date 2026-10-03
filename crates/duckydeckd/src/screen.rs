//! Draws the active profile and turns gestures into navigation.
//!
//! Until the action catalog exists (M5), label and icon come from the
//! binding or are derived from the action id, and regular actions are
//! only logged.

use anyhow::{Context, Result};
use duckydeck_core::CommandRunner;
use duckydeck_core::config::{Binding, FOLDER_ACTION, Store};
use duckydeck_core::icons;
use duckydeck_core::nav::{BACK_ACTION, Nav, PAGE_ACTION, Press};
use duckydeck_core::render::{KeyView, Renderer, SegmentView};
use duckydeck_core::theme::{Role, Theme};
use image::RgbImage;

use crate::device::Deck;
use crate::gesture::{Control, Gesture};
use crate::surface::{KEY_SIZE, STRIP_H};

const FALLBACK_THEME: &str =
    "background = \"#121212\"\nforeground = \"#bebebe\"\naccent = \"#e68e0d\"";
/// Minimum horizontal travel for a strip swipe to change the page.
const SWIPE_MIN: u16 = 100;

pub struct Screen {
    renderer: Renderer,
    theme: Theme,
    store: Store,
    nav: Nav,
}

impl Screen {
    pub fn new(font: Option<Vec<u8>>, store: Store) -> Result<Self> {
        let theme = match load_theme() {
            Some(t) => t,
            None => Theme::parse(FALLBACK_THEME).context("fallback theme")?,
        };
        let nav = Nav::new(&store.current);
        Ok(Self {
            renderer: Renderer::new(font.into_iter().collect()),
            theme,
            store,
            nav,
        })
    }

    /// Keeps the previous theme if the new one cannot be read.
    pub fn reload_theme(&mut self) {
        if let Some(t) = load_theme() {
            self.theme = t;
        }
    }

    /// Returns whether the deck needs a redraw.
    pub fn reload_config(&mut self, runner: &dyn CommandRunner) -> bool {
        if !self.store.reload(runner) {
            return false;
        }
        self.nav.reconcile(&self.store.current);
        let c = &self.store.current;
        tracing::info!(
            profile = %self.nav.profile,
            profiles = c.profiles.len(),
            "config reloaded"
        );
        true
    }

    pub fn draw(&mut self, deck: &mut Deck) -> Result<()> {
        deck.out
            .set_brightness(self.store.current.config.brightness)?;
        for i in 0..8 {
            self.key(deck, i, false)?;
        }
        self.draw_strip(deck)
    }

    pub fn draw_strip(&mut self, deck: &mut Deck) -> Result<()> {
        (0..4u8).try_for_each(|seg| self.segment(deck, seg, false))?;
        deck.out.flush()
    }

    pub fn on_gesture(&mut self, deck: &mut Deck, g: Gesture) {
        let res = match g {
            Gesture::Down(Control::Key(i)) => self.key(deck, i, true),
            Gesture::Up(Control::Key(i)) => self.key(deck, i, false),
            Gesture::Down(Control::Encoder(i)) => self.segment(deck, i, true),
            Gesture::Up(Control::Encoder(i)) => self.segment(deck, i, false),
            Gesture::Tap(Control::Key(i)) => {
                let press = self.nav.press_key(&self.store.current, usize::from(i));
                self.handle(deck, press)
            }
            Gesture::StripSwipe((x0, _), (x1, _)) if x0.abs_diff(x1) >= SWIPE_MIN => {
                let press = self
                    .nav
                    .step_page(&self.store.current, if x1 < x0 { 1 } else { -1 });
                self.handle(deck, press)
            }
            _ => return,
        };
        if let Err(e) = res.and_then(|()| deck.out.flush()) {
            tracing::warn!(error = %e, "deck update failed");
        }
    }

    fn handle(&mut self, deck: &mut Deck, press: Press) -> Result<()> {
        match press {
            Press::Navigated => {
                tracing::debug!(page = self.nav.page, folder = ?self.nav.folder, "navigated");
                self.draw(deck)
            }
            Press::Run(b) => {
                tracing::info!(action = %b.action, "action (not implemented before M5)");
                Ok(())
            }
            Press::Nothing => Ok(()),
        }
    }

    fn key(&mut self, deck: &mut Deck, i: u8, pressed: bool) -> Result<()> {
        let binding = self.nav.keys(&self.store.current)[usize::from(i % 8)].take();
        let label = binding.as_ref().map(label);
        let view = KeyView {
            icon: binding.as_ref().and_then(icon),
            label: label.as_deref(),
            fg: Role::Foreground,
            bg: if pressed {
                Role::LighterBackground
            } else {
                Role::Background
            },
        };
        let img = to_image(self.renderer.key(&self.theme, &view)?)?;
        debug_assert_eq!(img.width(), KEY_SIZE);
        deck.out.set_key(i, &img)
    }

    /// Strip segment above encoder `seg`: icon and label of its dial.
    fn segment(&mut self, deck: &mut Deck, seg: u8, pressed: bool) -> Result<()> {
        let binding = self.nav.dials(&self.store.current)[usize::from(seg % 4)].take();
        let label = binding.as_ref().map(label);
        let view = SegmentView {
            icon: binding.as_ref().and_then(icon),
            text: label.as_deref(),
            level: None,
            fg: Role::Accent,
            bg: if pressed {
                Role::LighterBackground
            } else {
                Role::Background
            },
        };
        let img = to_image(self.renderer.segment(&self.theme, &view)?)?;
        debug_assert_eq!(img.height(), STRIP_H);
        deck.out.set_strip(u16::from(seg) * 200, &img)
    }
}

/// Explicit label, else the folder name, else empty for other structure
/// actions (the icon says it all), else the action name without its group.
fn label(b: &Binding) -> String {
    if let Some(l) = &b.label {
        return l.clone();
    }
    if b.action == FOLDER_ACTION {
        return b
            .args
            .get("folder")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();
    }
    if b.action.starts_with("structure.") {
        return String::new();
    }
    let name = b.action.rsplit('.').next().unwrap_or(&b.action);
    let mut name = name.replace('_', " ");
    if let Some(n) = b.args.get("n").and_then(|v| v.as_integer()) {
        name = format!("{name} {n}");
    }
    name
}

fn icon(b: &Binding) -> Option<&'static [u8]> {
    if let Some(name) = &b.icon {
        return icons::get(name);
    }
    let name = match b.action.as_str() {
        BACK_ACTION => "back".to_owned(),
        PAGE_ACTION => match b.args.get("to").and_then(|v| v.as_str()) {
            Some("prev") => "page-prev".to_owned(),
            _ => "page-next".to_owned(),
        },
        a => a.rsplit('.').next().unwrap_or(a).replace('_', "-"),
    };
    icons::get(&name)
}

fn to_image(img: duckydeck_core::render::RgbImage) -> Result<RgbImage> {
    RgbImage::from_raw(img.width, img.height, img.data)
        .context("renderer returned a malformed image")
}

fn load_theme() -> Option<Theme> {
    match Theme::current_path().map(|p| Theme::load(&p)) {
        Some(Ok(t)) => Some(t),
        other => {
            tracing::warn!(error = ?other.map(|r| r.err()), "theme unavailable");
            None
        }
    }
}
