//! Draws the active profile and turns gestures into navigation.
//!
//! Label and icon come from the binding, else the action catalog, else the
//! action id. Catalog actions run detached; dial actions go to the
//! [`levels`](crate::levels) worker.

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use duckydeck_core::catalog::{Catalog, Confirm, Exec};
use duckydeck_core::compound::{self, MULTI_ACTION, PROFILE_ACTION, Step, TOGGLE_ACTION};
use duckydeck_core::config::{Binding, FOLDER_ACTION, Store};
use duckydeck_core::context::Context as WindowContext;
use duckydeck_core::dial::{Dial, Levels};
use duckydeck_core::hypr::{self, WorkspaceState, Workspaces};
use duckydeck_core::icons;
use duckydeck_core::ipc;
use duckydeck_core::media::{self, MediaKey, Players, Status};
use duckydeck_core::nav::{BACK_ACTION, Nav, PAGE_ACTION, PAGE_SCROLL_ACTION, Press};
use duckydeck_core::render::{GlyphView, KeyView, MediaView, Renderer, SegmentView};
use duckydeck_core::theme::{Role, Theme};
use duckydeck_core::toggle;
use duckydeck_core::{CommandRunner, TokioRunner};
use image::RgbImage;
use tokio::sync::mpsc::UnboundedSender;

use crate::device::{Deck, DeckEvent};
use crate::gesture::{Control, Gesture};
use crate::levels::Job;
use crate::mpris;
use crate::surface::{KEY_SIZE, STRIP_H};
use crate::toggles::Toggles;

const FALLBACK_THEME: &str =
    "background = \"#121212\"\nforeground = \"#bebebe\"\naccent = \"#e68e0d\"";
/// Minimum horizontal travel for a strip swipe to change the page. The
/// device reports only ~50-100 px for a quick swipe (measured).
const SWIPE_MIN: u16 = 40;
/// How long dial values replace the media view after touching a dial.
const OVERLAY: Duration = Duration::from_secs(2);
/// Media view refresh while playing (progress and time).
const MEDIA_TICK: Duration = Duration::from_secs(1);
/// The media view stays this long after playback stops: players report a
/// short pause while seeking.
const MEDIA_HOLD: Duration = Duration::from_millis(1500);

/// Channels to the background tasks.
/// Profile, page, folder and key of a toggle.
type ToggleSlot = (String, usize, Option<String>, u8);

pub struct Tasks {
    pub jobs: UnboundedSender<Job>,
    pub media: UnboundedSender<mpris::Press>,
    pub hypr: UnboundedSender<String>,
    pub toggles: UnboundedSender<String>,
    /// Delayed multi-action steps back into the main loop.
    pub events: UnboundedSender<DeckEvent>,
}

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
    toggles: Toggles,
    /// Pressed toggle actions for the [`toggles`](crate::toggles) task.
    toggle_tx: UnboundedSender<String>,
    /// Automatic profile switching by active window.
    context: WindowContext,
    /// Next state index of toggle keys, until the config reloads.
    toggled: HashMap<ToggleSlot, usize>,
    events_tx: UnboundedSender<DeckEvent>,
    /// IPC `set_brightness` override until the next config change.
    brightness: Option<u8>,
    preview_rev: u64,
    /// Open `learn` connections; while > 0 the deck only selects slots.
    learners: usize,
    /// Slot touched in learn mode, picked up by the main loop.
    learned: Option<ipc::SlotPress>,
}

impl Screen {
    pub fn new(
        font: Option<Vec<u8>>,
        store: Store,
        catalog: Catalog,
        unavailable: BTreeSet<String>,
        tasks: Tasks,
    ) -> Result<Self> {
        let theme = match load_theme() {
            Some(t) => t,
            None => Theme::parse(FALLBACK_THEME).context("fallback theme")?,
        };
        let nav = Nav::new(&store.current);
        let context = WindowContext::new(&store.current);
        Ok(Self {
            renderer: Renderer::new(font.into_iter().collect()),
            theme,
            store,
            nav,
            catalog,
            unavailable,
            jobs: tasks.jobs,
            levels: Levels::default(),
            media_tx: tasks.media,
            players: Players::default(),
            overlay_until: None,
            media_tick: None,
            media_hold: None,
            hypr_tx: tasks.hypr,
            workspaces: Workspaces::default(),
            toggles: Toggles::new(),
            toggle_tx: tasks.toggles,
            context,
            toggled: HashMap::new(),
            events_tx: tasks.events,
            brightness: None,
            preview_rev: 0,
            learners: 0,
            learned: None,
        })
    }

    /// Swaps the label font; `None` keeps the current one.
    pub fn set_font(&mut self, font: Option<Vec<u8>>) {
        if let Some(f) = font {
            self.renderer = Renderer::new(vec![f]);
        }
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
        duckydeck_core::check::notify_problems(&self.store.current, &self.catalog, runner);
        self.brightness = None;
        self.toggled.clear();
        let reset = self.store.current.config.profile != configured;
        self.context.reload(&self.store.current, reset);
        let wanted = self.context.wanted().to_owned();
        if wanted != self.nav.profile {
            // `profile` in config.toml or a `match` changed: switch.
            if let Err(e) = self.nav.set_profile(&self.store.current, &wanted) {
                tracing::warn!(error = %e, "profile switch after reload failed");
            }
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
        deck.out.set_brightness(self.brightness())?;
        self.draw_keys(deck)?;
        self.draw_strip(deck)
    }

    fn brightness(&self) -> u8 {
        self.brightness
            .unwrap_or(self.store.current.config.brightness)
    }

    pub fn status(&self, deck: Option<&Deck>) -> ipc::Status {
        let c = &self.store.current;
        ipc::Status {
            connected: deck.is_some(),
            serial: deck.map(|d| d.serial.clone()),
            profile: self.nav.profile.clone(),
            profiles: c.profiles.keys().cloned().collect(),
            page: self.nav.page + 1,
            pages: c
                .profiles
                .get(&self.nav.profile)
                .map_or(0, |p| p.pages.len()),
            folder: self.nav.folder.clone(),
            brightness: self.brightness(),
        }
    }

    /// Every action the editor offers, with availability on this system.
    pub fn actions(&self) -> Vec<duckydeck_core::library::Item> {
        duckydeck_core::library::items(&self.catalog, &self.unavailable)
    }

    /// Renders a page or folder of any profile into `dir`, replacing the
    /// previous preview.
    pub fn preview(
        &mut self,
        dir: &std::path::Path,
        profile: &str,
        page: Option<usize>,
        folder: Option<String>,
    ) -> Result<ipc::Preview> {
        let p = self
            .store
            .current
            .profiles
            .get(profile)
            .with_context(|| format!("unknown profile {profile:?}"))?;
        if let Some(f) = &folder
            && !p.folders.contains_key(f)
        {
            anyhow::bail!("unknown folder {f:?}");
        }
        let page = match page.unwrap_or(1).checked_sub(1) {
            Some(n) if n < p.pages.len() => n,
            _ => anyhow::bail!("profile {profile:?} has {} page(s)", p.pages.len()),
        };
        let nav = Nav {
            profile: profile.to_owned(),
            page,
            folder,
        };
        self.preview_rev += 1;
        let rev = self.preview_rev;
        std::fs::create_dir_all(dir).context("create preview dir")?;
        for e in std::fs::read_dir(dir)?.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
        let write = |name: String, img: RgbImage| -> Result<std::path::PathBuf> {
            let path = dir.join(format!("{name}-{rev}.png"));
            img.save_with_format(&path, image::ImageFormat::Png)
                .with_context(|| format!("write {}", path.display()))?;
            Ok(path)
        };
        let mut keys = Vec::new();
        for i in 0..8 {
            let img = self.key_image(&nav, i, false)?;
            keys.push(write(format!("key{i}"), img)?);
        }
        let mut dials = Vec::new();
        for i in 0..4 {
            let img = self.segment_image(&nav, i, false)?;
            dials.push(write(format!("dial{i}"), img)?);
        }
        Ok(ipc::Preview { rev, keys, dials })
    }

    /// Applies an IPC command; `Status` and `Subscribe` change nothing,
    /// `Reload` is handled by the main loop (it needs async font lookup).
    pub fn command(
        &mut self,
        deck: Option<&mut Deck>,
        cmd: &ipc::Command,
    ) -> std::result::Result<(), String> {
        let press = match cmd {
            ipc::Command::Status
            | ipc::Command::Subscribe
            | ipc::Command::Reload
            | ipc::Command::ListActions
            | ipc::Command::Preview { .. } => {
                return Ok(());
            }
            ipc::Command::Learn => {
                self.learners += 1;
                tracing::info!("learn mode on");
                return Ok(());
            }
            ipc::Command::SetProfile { profile } => {
                let press = self.nav.set_profile(&self.store.current, profile)?;
                self.context.set_manual(profile);
                press
            }
            ipc::Command::SetPage { page } => self.nav.set_page(
                &self.store.current,
                page.checked_sub(1).ok_or("pages start at 1")?,
            )?,
            ipc::Command::SetBrightness { brightness } => {
                if *brightness > 100 {
                    return Err("brightness must be 0-100".into());
                }
                self.brightness = Some(*brightness);
                if let Some(d) = deck {
                    d.out
                        .set_brightness(*brightness)
                        .map_err(|e| e.to_string())?;
                }
                return Ok(());
            }
        };
        if press == Press::Navigated
            && let Some(d) = deck
        {
            self.draw(d).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// A window got focus; returns whether the profile changed.
    pub fn focus(&mut self, class: &str, title: &str) -> bool {
        let wanted = self.context.focus(class, title);
        if self.learners > 0 {
            return false;
        }
        if wanted == self.nav.profile {
            return false;
        }
        tracing::info!(profile = wanted, class, "switching profile for window");
        let wanted = wanted.to_owned();
        self.nav
            .set_profile(&self.store.current, &wanted)
            .is_ok_and(|p| p == Press::Navigated)
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

    /// Returns whether a shown key changed.
    pub fn set_toggles(&mut self, t: Toggles) -> bool {
        let keys = self.nav.keys(&self.store.current);
        let changed = keys
            .iter()
            .flatten()
            .any(|b| self.toggles.get(&b.action) != t.get(&b.action));
        self.toggles = t;
        changed
    }

    /// Next full second of a shown recording time, if any.
    pub fn key_deadline(&self) -> Option<Instant> {
        let now = SystemTime::now();
        self.nav
            .keys(&self.store.current)
            .iter()
            .flatten()
            .filter_map(|b| self.toggles.get(&b.action)?.since)
            .map(|since| {
                let elapsed = now.duration_since(since).unwrap_or_default();
                Instant::now() + Duration::from_secs(1)
                    - Duration::from_nanos(elapsed.subsec_nanos().into())
            })
            .min()
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

    pub fn learn_ended(&mut self) {
        self.learners = self.learners.saturating_sub(1);
        if self.learners == 0 {
            tracing::info!("learn mode off");
        }
    }

    /// The slot touched in learn mode since the last call.
    pub fn take_learned(&mut self) -> Option<ipc::SlotPress> {
        self.learned.take()
    }

    /// Learn mode: presses select slots instead of running actions.
    fn learn(&mut self, g: &Gesture) -> bool {
        use duckydeck_core::library::Slot;
        let slot = match *g {
            Gesture::Tap(Control::Key(i)) | Gesture::LongPress(Control::Key(i)) => (Slot::Key, i),
            Gesture::Tap(Control::Encoder(i))
            | Gesture::LongPress(Control::Encoder(i))
            | Gesture::Twist { encoder: i, .. } => (Slot::Dial, i),
            Gesture::StripTap(x, _) | Gesture::StripLongPress(x, _) => {
                (Slot::Dial, u8::try_from(x / 200).unwrap_or(3).min(3))
            }
            _ => return false,
        };
        self.learned = Some(ipc::SlotPress {
            kind: slot.0,
            index: usize::from(slot.1) + 1,
        });
        true
    }

    pub fn on_gesture(&mut self, deck: &mut Deck, g: Gesture) {
        if self.learners > 0 && self.learn(&g) {
            return;
        }
        let res = match g {
            Gesture::Down(Control::Key(i)) => self.key(deck, i, true),
            Gesture::Up(Control::Key(i)) => self.key(deck, i, false),
            Gesture::Down(Control::Encoder(i)) => self
                .touch_dials(deck)
                .and_then(|()| self.segment(deck, i, true)),
            Gesture::Up(Control::Encoder(i)) => self
                .touch_dials(deck)
                .and_then(|()| self.segment(deck, i, false)),
            Gesture::Tap(Control::Key(i)) => self.press(deck, i, false),
            Gesture::LongPress(Control::Key(i)) => self.press(deck, i, true),
            Gesture::Tap(Control::Encoder(i)) if self.dial_is(i, PAGE_SCROLL_ACTION) => {
                let press = self.nav.first_page();
                self.handle(deck, press, false)
            }
            Gesture::Twist { encoder, delta, .. } if self.dial_is(encoder, PAGE_SCROLL_ACTION) => {
                let press = self.nav.step_page(&self.store.current, isize::from(delta));
                self.handle(deck, press, false)
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
        self.dial_is(seg, hypr::SCROLL_ACTION)
    }

    fn dial_is(&self, seg: u8, action: &str) -> bool {
        self.nav.dials(&self.store.current)[usize::from(seg % 4)]
            .as_ref()
            .is_some_and(|b| b.action == action)
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

    fn press(&mut self, deck: &mut Deck, i: u8, long: bool) -> Result<()> {
        let mut press = self.nav.press_key(&self.store.current, usize::from(i));
        if let Press::Run(b) = &press
            && b.action == TOGGLE_ACTION
        {
            // Validated by the parser.
            let Ok(states) = compound::states(b) else {
                return Ok(());
            };
            let n = self.toggled.entry(self.toggle_slot(i)).or_default();
            press = Press::Run(states[*n].clone());
            *n = (*n + 1) % states.len();
            self.key(deck, i, false)?;
        }
        self.handle(deck, press, long)
    }

    fn toggle_slot(&self, i: u8) -> ToggleSlot {
        toggle_slot(&self.nav, i)
    }

    /// A toggle key shows the binding its next press runs.
    fn shown_in(&self, nav: &Nav, i: u8, b: Binding) -> Binding {
        if b.action != TOGGLE_ACTION {
            return b;
        }
        let n = self.toggled.get(&toggle_slot(nav, i)).copied().unwrap_or(0);
        match compound::states(&b) {
            Ok(states) => states[n].clone(),
            Err(_) => b,
        }
    }

    /// A delayed step of a multi action.
    pub fn run_step(&mut self, deck: &mut Deck, b: Binding, long: bool) {
        if let Err(e) = self.handle(deck, Press::Run(b), long) {
            tracing::warn!(error = %e, "multi action step failed");
        }
    }

    fn handle(&mut self, deck: &mut Deck, press: Press, long: bool) -> Result<()> {
        match press {
            Press::Run(b) if b.action == PROFILE_ACTION => {
                let Ok(profile) = compound::profile(&b) else {
                    return Ok(());
                };
                match self.nav.set_profile(&self.store.current, profile) {
                    Ok(_) => {
                        self.context.set_manual(profile);
                        self.draw(deck)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "profile action failed");
                        Ok(())
                    }
                }
            }
            Press::Run(b) if b.action == MULTI_ACTION => {
                let Ok(steps) = compound::steps(&b) else {
                    return Ok(());
                };
                let tx = self.events_tx.clone();
                tokio::spawn(async move {
                    for s in steps {
                        match s {
                            Step::Delay(d) => tokio::time::sleep(d).await,
                            Step::Run(b) => {
                                if tx.send(DeckEvent::RunStep(b, long)).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                });
                Ok(())
            }
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
                Exec::Command(spec) => {
                    let label = b.label.clone().unwrap_or_else(|| entry.label.clone());
                    let action = b.action.clone();
                    Ok(TokioRunner.spawn_watched(
                        &spec,
                        Box::new(move |reason| notify_failed(&action, &label, &reason)),
                    )?)
                }
                Exec::Dispatch(lua) => {
                    self.dispatch(lua);
                    Ok(())
                }
            });
        if entry.state.as_ref().is_some_and(|s| !s.command.is_empty()) {
            let _ = self.toggle_tx.send(b.action.clone());
        }
        match res {
            Ok(()) => tracing::info!(action = %b.action, "action started"),
            Err(e) => tracing::warn!(action = %b.action, error = %e, "action failed"),
        }
    }

    fn key(&mut self, deck: &mut Deck, i: u8, pressed: bool) -> Result<()> {
        let nav = self.nav.clone();
        let img = self.key_image(&nav, i, pressed)?;
        deck.out.set_key(i, &img)
    }

    fn key_image(&mut self, nav: &Nav, i: u8, pressed: bool) -> Result<RgbImage> {
        let binding = nav.keys(&self.store.current)[usize::from(i % 8)]
            .take()
            .map(|b| self.shown_in(nav, i, b));
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
            return to_image(self.renderer.glyph_key(&self.theme, &view)?);
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
        Ok(img)
    }

    /// Strip segment above encoder `seg`: icon and label of its dial, or
    /// its level once known.
    fn segment(&mut self, deck: &mut Deck, seg: u8, pressed: bool) -> Result<()> {
        let nav = self.nav.clone();
        let img = self.segment_image(&nav, seg, pressed)?;
        deck.out.set_strip(u16::from(seg) * 200, &img)
    }

    fn segment_image(&mut self, nav: &Nav, seg: u8, pressed: bool) -> Result<RgbImage> {
        let binding = nav.dials(&self.store.current)[usize::from(seg % 4)].take();
        let level = binding
            .as_ref()
            .and_then(Dial::from_binding)
            .map(|d| (d, self.levels.of(d)));
        let scroll = binding
            .as_ref()
            .filter(|b| b.action == hypr::SCROLL_ACTION)
            .and(self.workspaces.active);
        let pages = binding
            .as_ref()
            .filter(|b| b.action == PAGE_SCROLL_ACTION)
            .and_then(|_| self.store.current.profiles.get(&nav.profile))
            .map(|p| format!("{}/{}", nav.page + 1, p.pages.len()));
        let text = match level {
            _ if scroll.is_some() => scroll.map(|n| n.to_string()),
            _ if pages.is_some() => pages,
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
        Ok(img)
    }

    /// Explicit label, else the catalog label, else the folder name, else
    /// empty for other structure actions (the icon says it all), else the
    /// action name without its group.
    fn label(&self, b: &Binding) -> String {
        if let Some(since) = self.toggles.get(&b.action).and_then(|t| t.since) {
            let d = SystemTime::now().duration_since(since).unwrap_or_default();
            return toggle::elapsed_text(d);
        }
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
            return icons::get(e.icon.name(self.toggles.get(&b.action).map(|t| t.on)));
        }
        let name = match b.action.as_str() {
            BACK_ACTION => "back".to_owned(),
            MULTI_ACTION => "multi-action".to_owned(),
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

fn toggle_slot(nav: &Nav, i: u8) -> ToggleSlot {
    (nav.profile.clone(), nav.page, nav.folder.clone(), i)
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

/// Shell notification for a catalog command that failed right away.
fn notify_failed(action: &str, label: &str, reason: &str) {
    tracing::warn!(%action, %reason, "action failed");
    let spec =
        duckydeck_core::CommandSpec::omarchy(["notification", "send", "--app-name", "DuckyDeck"])
            .args([format!("{label} failed"), reason.to_owned()]);
    if let Err(e) = TokioRunner.spawn(&spec) {
        tracing::warn!(error = %e, "action failure notification failed");
    }
}
