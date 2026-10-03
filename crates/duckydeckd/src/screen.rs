//! Draws the active profile and turns gestures into navigation.
//!
//! Label and icon come from the binding, else the action catalog, else the
//! action id. Catalog actions run detached; dial actions go to the
//! [`levels`](crate::levels) worker.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use duckydeck_core::catalog::{Catalog, Confirm, Exec};
use duckydeck_core::config::{Binding, FOLDER_ACTION, Store};
use duckydeck_core::dial::{Dial, Levels};
use duckydeck_core::hypr::{self, WorkspaceState, Workspaces};
use duckydeck_core::icons;
use duckydeck_core::media::{self, MediaKey, Players, Status};
use duckydeck_core::nav::{BACK_ACTION, Nav, PAGE_ACTION, Press};
use duckydeck_core::render::{GlyphView, KeyView, MediaView, Renderer, SegmentView};
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
/// How long dial values replace the media view after touching a dial.
const OVERLAY: Duration = Duration::from_secs(2);
/// Media view refresh while playing (progress and time).
const MEDIA_TICK: Duration = Duration::from_secs(1);
/// The media view stays this long after playback stops: players report a
/// short pause while seeking.
const MEDIA_HOLD: Duration = Duration::from_millis(1500);

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
    /// Dial values are shown instead of the media view until then.
    overlay_until: Option<Instant>,
    /// Next media view refresh; `Some` while the strip shows it.
    media_tick: Option<Instant>,
    /// Player last shown in the media view and until when it stays.
    media_hold: Option<(String, Instant)>,
    /// Lua dispatchers for the [`hyprland`](crate::hyprland) task.
    hypr_tx: UnboundedSender<String>,
    workspaces: Workspaces,
}

impl Screen {
    pub fn new(
        font: Option<Vec<u8>>,
        store: Store,
        catalog: Catalog,
        unavailable: BTreeSet<String>,
        jobs: UnboundedSender<Job>,
        media_tx: UnboundedSender<mpris::Press>,
        hypr_tx: UnboundedSender<String>,
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
            overlay_until: None,
            media_tick: None,
            media_hold: None,
            hypr_tx,
            workspaces: Workspaces::default(),
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
        let configured = self.store.current.config.profile.clone();
        if !self.store.reload(runner) {
            return false;
        }
        if self.store.current.config.profile != configured {
            // `profile` in config.toml changed: switch to it.
            self.nav = Nav::new(&self.store.current);
        } else {
            self.nav.reconcile(&self.store.current);
        }
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
                .map(|(k, b)| (k.icon(playing(ps, b)), enabled(ps, k, b)))
                .collect::<Vec<_>>()
        };
        let before = icons(&self.players);
        let after = icons(&players);
        let strip_before = self.playing_now().cloned();
        self.players = players;
        let strip_after = self.playing_now().cloned();
        before != after || strip_before != strip_after
    }

    pub fn draw_strip(&mut self, deck: &mut Deck) -> Result<()> {
        let now = Instant::now();
        if self.overlay_until.is_some_and(|t| t <= now) {
            self.overlay_until = None;
        }
        if self.overlay_until.is_none()
            && let Some(p) = self.media_player(now)
        {
            if p.status == Status::Playing {
                self.media_hold = Some((p.name.clone(), now + MEDIA_HOLD));
            }
            let time = p.time_text(now);
            let view = MediaView {
                icon: icons::get("play"),
                title: p.title.as_deref().unwrap_or(p.short_name()),
                artist: p.artist.as_deref(),
                time: time.as_deref(),
                progress: p.progress(now),
            };
            let img = to_image(self.renderer.media(&self.theme, &view)?)?;
            self.media_tick = Some(now + MEDIA_TICK);
            deck.out.set_strip(0, &img)?;
        } else {
            self.media_tick = None;
            (0..4u8).try_for_each(|seg| self.segment(deck, seg, false))?;
        }
        deck.out.flush()
    }

    /// The player for the media view: the playing one, else the one shown
    /// last while its hold lasts.
    fn media_player(&self, now: Instant) -> Option<media::Player> {
        if let Some(p) = self.playing_now() {
            return Some(p.clone());
        }
        let (name, until) = self.media_hold.as_ref()?;
        if *until <= now {
            return None;
        }
        self.players.list.iter().find(|p| &p.name == name).cloned()
    }

    /// The player shown on the strip: the active one, if it is playing.
    fn playing_now(&self) -> Option<&media::Player> {
        self.players
            .active(None)
            .filter(|p| p.status == Status::Playing)
    }

    /// When the strip needs the next redraw without an event.
    pub fn strip_deadline(&self) -> Option<Instant> {
        if let Some(t) = self.overlay_until {
            return Some(t);
        }
        let hold = self.media_hold.as_ref().map(|(_, t)| *t);
        match (self.media_tick, hold) {
            (Some(a), Some(b)) if self.playing_now().is_none() => Some(a.min(b)),
            (a, _) => a,
        }
    }

    /// Dial values become visible (or stay visible) for [`OVERLAY`].
    fn touch_dials(&mut self, deck: &mut Deck) -> Result<()> {
        let was = self.overlay_until.is_some() || self.media_tick.is_none();
        let now = Instant::now();
        self.overlay_until = self.media_player(now).map(|_| now + OVERLAY);
        if was { Ok(()) } else { self.draw_strip(deck) }
    }

    /// Returns whether a shown workspace key or the scroll dial changed.
    pub fn set_workspaces(&mut self, ws: Workspaces) -> bool {
        let c = &self.store.current;
        let shown = |w: &Workspaces| {
            let keys = self.nav.keys(c);
            let dials = self.nav.dials(c);
            let keys: Vec<_> = keys
                .iter()
                .flatten()
                .filter_map(hypr::workspace_of)
                .map(|n| w.state(n))
                .collect();
            let scroll = dials
                .iter()
                .flatten()
                .any(|b| b.action == hypr::SCROLL_ACTION)
                .then_some(w.active);
            (keys, scroll)
        };
        let changed = shown(&self.workspaces) != shown(&ws);
        self.workspaces = ws;
        changed
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
            Gesture::Down(Control::Encoder(i)) => self
                .touch_dials(deck)
                .and_then(|()| self.segment(deck, i, true)),
            Gesture::Up(Control::Encoder(i)) => self
                .touch_dials(deck)
                .and_then(|()| self.segment(deck, i, false)),
            Gesture::Tap(Control::Key(i)) => {
                let press = self.nav.press_key(&self.store.current, usize::from(i));
                self.handle(deck, press, false)
            }
            Gesture::LongPress(Control::Key(i)) => {
                let press = self.nav.press_key(&self.store.current, usize::from(i));
                self.handle(deck, press, true)
            }
            Gesture::Tap(Control::Encoder(i)) => {
                if self.is_scroll(i) {
                    self.dispatch(hypr::SCROLL_PRESS.to_owned());
                } else {
                    self.dial_job(i, Job::Press);
                }
                return;
            }
            Gesture::Twist { encoder, delta, .. } => {
                if self.is_scroll(encoder) {
                    if let Some(d) = hypr::scroll(i32::from(delta)) {
                        self.dispatch(d);
                    }
                } else {
                    self.dial_job(encoder, |d| Job::Twist(d, i32::from(delta)));
                }
                self.touch_dials(deck)
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

    fn is_scroll(&self, seg: u8) -> bool {
        self.nav.dials(&self.store.current)[usize::from(seg % 4)]
            .as_ref()
            .is_some_and(|b| b.action == hypr::SCROLL_ACTION)
    }

    fn dispatch(&self, lua: String) {
        let _ = self.hypr_tx.send(lua);
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
            .exec(&b.action, &b.args)
            .map_err(anyhow::Error::from)
            .and_then(|exec| match exec {
                Exec::Command(spec) => Ok(TokioRunner.spawn(&spec)?),
                Exec::Dispatch(lua) => {
                    self.dispatch(lua);
                    Ok(())
                }
            });
        match res {
            Ok(()) => tracing::info!(action = %b.action, "action started"),
            Err(e) => tracing::warn!(action = %b.action, error = %e, "action failed"),
        }
    }

    fn key(&mut self, deck: &mut Deck, i: u8, pressed: bool) -> Result<()> {
        let binding = self.nav.keys(&self.store.current)[usize::from(i % 8)].take();
        let bg = if pressed {
            Role::LighterBackground
        } else {
            Role::Background
        };
        if let Some(b) = binding.as_ref().filter(|b| b.icon.is_none())
            && let Some(n) = hypr::workspace_of(b)
        {
            let state = self.workspaces.state(n);
            let view = GlyphView {
                text: &n.to_string(),
                marked: state == WorkspaceState::Active,
                fg: match state {
                    WorkspaceState::Active => Role::Accent,
                    WorkspaceState::Occupied => Role::Foreground,
                    WorkspaceState::Empty => Role::Muted,
                },
                bg,
            };
            let img = to_image(self.renderer.glyph_key(&self.theme, &view)?)?;
            return deck.out.set_key(i, &img);
        }
        let label = binding.as_ref().map(|b| self.label(b));
        let view = KeyView {
            icon: binding.as_ref().and_then(|b| self.icon(b)),
            label: label.as_deref(),
            fg: match binding
                .as_ref()
                .and_then(|b| Some((MediaKey::from_binding(b)?, b)))
            {
                Some((k, b)) if !enabled(&self.players, k, b) => Role::Muted,
                _ => Role::Foreground,
            },
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
        let scroll = binding
            .as_ref()
            .filter(|b| b.action == hypr::SCROLL_ACTION)
            .and(self.workspaces.active);
        let text = match level {
            _ if scroll.is_some() => scroll.map(|n| n.to_string()),
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
        if b.action == "window.focus" {
            let dir = match b.args.get("dir").and_then(|v| v.as_str()) {
                Some("l") => "focus-left",
                Some("u") => "focus-up",
                Some("d") => "focus-down",
                _ => "focus-right",
            };
            return icons::get(dir);
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

/// Whether the player a media key controls accepts it.
fn enabled(players: &Players, k: MediaKey, b: &Binding) -> bool {
    k.enabled(players.active(media::wanted_player(b)))
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
