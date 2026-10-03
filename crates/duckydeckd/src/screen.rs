//! Draws the active profile and turns gestures into navigation.
//!
//! Label and icon come from the binding, else the action catalog, else the
//! action id. Catalog actions run detached; dial actions go to the
//! [`levels`](crate::levels) worker.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use duckydeck_core::catalog::{Catalog, Confirm};
use duckydeck_core::config::{Binding, FOLDER_ACTION, Store};
use duckydeck_core::dial::{Dial, Levels};
use duckydeck_core::icons;
use duckydeck_core::media::{self, MediaKey, Players, Status};
use duckydeck_core::nav::{BACK_ACTION, Nav, PAGE_ACTION, Press};
use duckydeck_core::render::{KeyView, Renderer, SegmentView};
use duckydeck_core::theme::{Role, Theme};
use duckydeck_core::{CommandRunner, TokioRunner};
use image::RgbImage;
use tokio::sync::mpsc::UnboundedSender;

use crate::device::Deck;
use crate::gesture::{Control, Gesture};
use crate::levels::Job;
use crate::mpris;
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
    catalog: Catalog,
    /// Catalog ids whose omarchy route is missing; shown with a warning icon.
    unavailable: BTreeSet<String>,
    jobs: UnboundedSender<Job>,
    levels: Levels,
    media_tx: UnboundedSender<mpris::Press>,
    players: Players,
}

impl Screen {
    pub fn new(
        font: Option<Vec<u8>>,
        store: Store,
        catalog: Catalog,
        unavailable: BTreeSet<String>,
        jobs: UnboundedSender<Job>,
        media_tx: UnboundedSender<mpris::Press>,
    ) -> Result<Self> {
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
            catalog,
            unavailable,
            jobs,
            levels: Levels::default(),
            media_tx,
            players: Players::default(),
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
        self.draw_keys(deck)?;
        self.draw_strip(deck)
    }

    pub fn draw_keys(&mut self, deck: &mut Deck) -> Result<()> {
        (0..8u8).try_for_each(|i| self.key(deck, i, false))?;
        deck.out.flush()
    }

    /// Returns whether a shown media key changed its icon.
    pub fn set_media(&mut self, players: Players) -> bool {
        let keys = self.nav.keys(&self.store.current);
        let icons = |ps: &Players| {
            keys.iter()
                .flatten()
                .filter_map(|b| Some((MediaKey::from_binding(b)?, b)))
                .map(|(k, b)| k.icon(playing(ps, b)))
                .collect::<Vec<_>>()
        };
        let before = icons(&self.players);
        let after = icons(&players);
        self.players = players;
        before != after
    }

    pub fn draw_strip(&mut self, deck: &mut Deck) -> Result<()> {
        (0..4u8).try_for_each(|seg| self.segment(deck, seg, false))?;
        deck.out.flush()
    }

    /// Returns whether a shown dial changed.
    pub fn set_levels(&mut self, levels: Levels) -> bool {
        let old = std::mem::replace(&mut self.levels, levels);
        self.nav
            .dials(&self.store.current)
            .iter()
            .flatten()
            .filter_map(Dial::from_binding)
            .any(|d| old.of(d) != levels.of(d))
    }

    pub fn on_gesture(&mut self, deck: &mut Deck, g: Gesture) {
        let res = match g {
            Gesture::Down(Control::Key(i)) => self.key(deck, i, true),
            Gesture::Up(Control::Key(i)) => self.key(deck, i, false),
            Gesture::Down(Control::Encoder(i)) => self.segment(deck, i, true),
            Gesture::Up(Control::Encoder(i)) => self.segment(deck, i, false),
            Gesture::Tap(Control::Key(i)) => {
                let press = self.nav.press_key(&self.store.current, usize::from(i));
                self.handle(deck, press, false)
            }
            Gesture::LongPress(Control::Key(i)) => {
                let press = self.nav.press_key(&self.store.current, usize::from(i));
                self.handle(deck, press, true)
            }
            Gesture::Tap(Control::Encoder(i)) => {
                self.dial_job(i, Job::Press);
                return;
            }
            Gesture::Twist { encoder, delta, .. } => {
                self.dial_job(encoder, |d| Job::Twist(d, i32::from(delta)));
                return;
            }
            Gesture::StripSwipe((x0, _), (x1, _)) if x0.abs_diff(x1) >= SWIPE_MIN => {
                let press = self
                    .nav
                    .step_page(&self.store.current, if x1 < x0 { 1 } else { -1 });
                self.handle(deck, press, false)
            }
            _ => return,
        };
        if let Err(e) = res.and_then(|()| deck.out.flush()) {
            tracing::warn!(error = %e, "deck update failed");
        }
    }

    fn dial(&self, seg: u8) -> Option<Dial> {
        self.nav.dials(&self.store.current)[usize::from(seg % 4)]
            .as_ref()
            .and_then(Dial::from_binding)
    }

    fn dial_job(&self, seg: u8, job: impl FnOnce(Dial) -> Job) {
        match self.dial(seg) {
            Some(d) => {
                let _ = self.jobs.send(job(d));
            }
            None => tracing::debug!(seg, "no dial action on this encoder"),
        }
    }

    fn handle(&mut self, deck: &mut Deck, press: Press, long: bool) -> Result<()> {
        match press {
            Press::Navigated => {
                tracing::debug!(page = self.nav.page, folder = ?self.nav.folder, "navigated");
                self.draw(deck)
            }
            Press::Run(b) => {
                self.run(&b, long);
                Ok(())
            }
            Press::Nothing => Ok(()),
        }
    }

    /// Runs a catalog action detached. Failures are logged, never fatal.
    fn run(&self, b: &Binding, long: bool) {
        if let Some(key) = MediaKey::from_binding(b) {
            let wanted = media::wanted_player(b).map(str::to_owned);
            let _ = self.media_tx.send(mpris::Press { key, wanted });
            return;
        }
        let Some(entry) = self.catalog.get(&b.action) else {
            tracing::info!(action = %b.action, "action not implemented yet");
            return;
        };
        if self.unavailable.contains(&b.action) {
            tracing::warn!(action = %b.action, "action disabled: omarchy route missing");
            return;
        }
        if entry.confirm == Some(Confirm::LongPress) && !long {
            tracing::debug!(action = %b.action, "needs a long press");
            return;
        }
        let res = entry
            .command(&b.action, &b.args)
            .map_err(anyhow::Error::from)
            .and_then(|spec| Ok(TokioRunner.spawn(&spec)?));
        match res {
            Ok(()) => tracing::info!(action = %b.action, "action started"),
            Err(e) => tracing::warn!(action = %b.action, error = %e, "action failed"),
        }
    }

    fn key(&mut self, deck: &mut Deck, i: u8, pressed: bool) -> Result<()> {
        let binding = self.nav.keys(&self.store.current)[usize::from(i % 8)].take();
        let label = binding.as_ref().map(|b| self.label(b));
        let view = KeyView {
            icon: binding.as_ref().and_then(|b| self.icon(b)),
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

    /// Strip segment above encoder `seg`: icon and label of its dial, or
    /// its level once known.
    fn segment(&mut self, deck: &mut Deck, seg: u8, pressed: bool) -> Result<()> {
        let binding = self.nav.dials(&self.store.current)[usize::from(seg % 4)].take();
        let level = binding
            .as_ref()
            .and_then(Dial::from_binding)
            .map(|d| (d, self.levels.of(d)));
        let text = match level {
            Some((_, (_, true))) => Some("Muted".to_owned()),
            Some((_, (Some(pct), false))) => Some(format!("{pct}%")),
            _ => binding.as_ref().map(|b| self.label(b)),
        };
        let icon = match (&binding, level) {
            (Some(b), Some((d, (pct, muted)))) if b.icon.is_none() => {
                icons::get(dial_icon(d, pct, muted))
            }
            (b, _) => b.as_ref().and_then(|b| self.icon(b)),
        };
        let view = SegmentView {
            icon,
            text: text.as_deref(),
            level: level
                .and_then(|(_, (pct, _))| pct)
                .map(|p| f32::from(p) / 100.0),
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

    /// Explicit label, else the catalog label, else the folder name, else
    /// empty for other structure actions (the icon says it all), else the
    /// action name without its group.
    fn label(&self, b: &Binding) -> String {
        if let Some(l) = &b.label {
            return l.clone();
        }
        if let Some(e) = self.catalog.get(&b.action) {
            return e.label.clone();
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

    /// Explicit icon, else a warning for disabled actions, else the catalog
    /// icon, else one derived from the action id.
    fn icon(&self, b: &Binding) -> Option<&'static [u8]> {
        if let Some(name) = &b.icon {
            return icons::get(name);
        }
        if self.unavailable.contains(&b.action) {
            return icons::get("warning");
        }
        if let Some(k) = MediaKey::from_binding(b) {
            return icons::get(k.icon(playing(&self.players, b)));
        }
        if let Some(e) = self.catalog.get(&b.action) {
            return icons::get(e.icon.default_name());
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
}

/// Whether the player a media key controls is playing.
fn playing(players: &Players, b: &Binding) -> bool {
    players
        .active(media::wanted_player(b))
        .is_some_and(|p| p.status == Status::Playing)
}

fn dial_icon(d: Dial, pct: Option<u8>, muted: bool) -> &'static str {
    let low = pct.is_some_and(|p| p < 34);
    match d {
        Dial::Volume { .. } if muted => "volume-off",
        Dial::Volume { .. } if low => "volume-low",
        Dial::Volume { .. } => "volume",
        Dial::Mic if muted => "mic-off",
        Dial::Mic => "mic",
        Dial::Brightness { .. } if low => "brightness-low",
        Dial::Brightness { .. } => "brightness",
    }
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
