//! DuckyDeck daemon: owns the Stream Deck +, renders keys and runs actions.

mod configwatch;
mod device;
mod gesture;
mod hyprland;
mod input;
mod ipc;
mod levels;
mod mpris;
mod screen;
mod surface;
mod themewatch;
mod toggles;

use std::time::{Duration, Instant};

use anyhow::Result;
use device::{Deck, DeckEvent};
use gesture::Recognizer;
use tokio::sync::{broadcast, mpsc};
use tracing_subscriber::EnvFilter;

const BOOT_TIME: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() -> Result<()> {
    let debug = std::env::args().any(|a| a == "--debug");
    // cosmic-text logs every missing fallback family per text layout.
    let default = if debug {
        "debug,cosmic_text=warn"
    } else {
        "info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default)),
        )
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "duckydeckd starting");

    let (tx, mut rx) = mpsc::unbounded_channel();
    if !device::fake_enabled() {
        spawn_hotplug(tx.clone())?;
    }

    if let Some(colors) = duckydeck_core::theme::Theme::current_path() {
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = themewatch::watch(&colors, tx).await {
                tracing::warn!(error = %e, "theme watcher stopped");
            }
        });
    }

    let runner = duckydeck_core::TokioRunner;
    first_start_setup(&runner).await;
    let config_dir = duckydeck_core::config::dir()
        .ok_or_else(|| anyhow::anyhow!("neither XDG_CONFIG_HOME nor HOME is set"))?;
    let store = duckydeck_core::config::Store::open(config_dir.clone(), &runner)?;
    log_config(&store);
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = configwatch::watch(config_dir, tx).await {
                tracing::warn!(error = %e, "config watcher stopped");
            }
        });
    }

    let font = duckydeck_core::font::system_font(&duckydeck_core::TokioRunner).await;
    let catalog = duckydeck_core::catalog::Catalog::builtin()?;
    let unavailable = unavailable_actions(&catalog, &runner).await;
    let (jobs_tx, jobs_rx) = mpsc::unbounded_channel();
    tokio::spawn(levels::worker(runner.clone(), jobs_rx, tx.clone()));
    tokio::spawn(levels::watch_audio(jobs_tx.clone()));
    let (media_tx, media_rx) = mpsc::unbounded_channel();
    tokio::spawn(mpris::run(media_rx, tx.clone()));
    let (hypr_tx, hypr_rx) = mpsc::unbounded_channel();
    tokio::spawn(hyprland::run(hypr_rx, tx.clone()));
    let (toggle_tx, toggle_rx) = mpsc::unbounded_channel();
    {
        let (catalog, tx) = (catalog.clone(), tx.clone());
        tokio::spawn(async move {
            if let Err(e) = toggles::run(catalog, toggle_rx, tx).await {
                tracing::warn!(error = %e, "toggle watcher stopped");
            }
        });
    }
    let mut painter = screen::Screen::new(
        font,
        store,
        catalog,
        unavailable,
        screen::Tasks {
            jobs: jobs_tx,
            media: media_tx,
            hypr: hypr_tx,
            toggles: toggle_tx,
        },
    )?;
    let (events, _) = broadcast::channel(32);
    match duckydeck_core::ipc::socket_path() {
        Some(path) => {
            let listener = ipc::bind(&path).await?;
            tokio::spawn(ipc::serve(listener, path, tx.clone(), events.clone()));
        }
        None => tracing::warn!("XDG_RUNTIME_DIR not set: IPC disabled"),
    }
    let mut deck = try_connect(&mut painter, &tx);
    let mut gestures = Recognizer::default();
    let mut last = painter.status(deck.as_ref());
    loop {
        // Every change of what the deck shows becomes an event.
        let now = painter.status(deck.as_ref());
        if let Some(ev) = duckydeck_core::ipc::Event::between(&last, &now) {
            let _ = events.send(ev);
            last = now;
        }
        let deadline = gestures.deadline();
        let strip_deadline = deck.as_ref().and_then(|_| painter.strip_deadline());
        let key_deadline = deck.as_ref().and_then(|_| painter.key_deadline());
        let ev = tokio::select! {
            ev = rx.recv() => match ev {
                Some(ev) => ev,
                None => break,
            },
            () = sleep_until(deadline) => {
                for g in gestures.tick(Instant::now()) {
                    on_gesture(&mut painter, &mut deck, g);
                }
                continue;
            }
            () = sleep_until(strip_deadline) => {
                if let Some(d) = &mut deck
                    && let Err(e) = painter.draw_strip(d)
                {
                    tracing::warn!(error = %e, "strip redraw failed");
                }
                continue;
            }
            () = sleep_until(key_deadline) => {
                if let Some(d) = &mut deck
                    && let Err(e) = painter.draw_keys(d)
                {
                    tracing::warn!(error = %e, "key redraw failed");
                }
                continue;
            }
        };
        match ev {
            DeckEvent::Added if deck.is_none() => {
                deck = try_connect(&mut painter, &tx);
                // The booting firmware clears the strip once, 1.0–1.5 s after
                // plug-in (measured on fw 2.0.3.7); keys are not affected.
                let tx = tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(BOOT_TIME).await;
                    let _ = tx.send(DeckEvent::BootDone);
                });
            }
            DeckEvent::Added => {}
            DeckEvent::BootDone => {
                if let Some(d) = &mut deck
                    && let Err(e) = painter.draw_strip(d)
                {
                    tracing::warn!(error = %e, "strip redraw failed");
                }
            }
            DeckEvent::ThemeChanged => {
                painter.reload_theme();
                if let Some(d) = &mut deck
                    && let Err(e) = painter.draw(d)
                {
                    tracing::warn!(error = %e, "redraw after theme change failed");
                }
            }
            DeckEvent::ConfigChanged => {
                if painter.reload_config(&runner)
                    && let Some(d) = &mut deck
                    && let Err(e) = painter.draw(d)
                {
                    tracing::warn!(error = %e, "redraw after config change failed");
                }
            }
            DeckEvent::Levels(l) => {
                if painter.set_levels(l)
                    && let Some(d) = &mut deck
                    && let Err(e) = painter.draw_strip(d)
                {
                    tracing::warn!(error = %e, "strip redraw failed");
                }
            }
            DeckEvent::Media(p) => {
                if painter.set_media(p)
                    && let Some(d) = &mut deck
                    && let Err(e) = painter.draw_keys(d).and_then(|()| painter.draw_strip(d))
                {
                    tracing::warn!(error = %e, "redraw after media change failed");
                }
            }
            DeckEvent::Workspaces(w) => {
                if painter.set_workspaces(w)
                    && let Some(d) = &mut deck
                    && let Err(e) = painter.draw_keys(d).and_then(|()| painter.draw_strip(d))
                {
                    tracing::warn!(error = %e, "redraw after workspace change failed");
                }
            }
            DeckEvent::Toggles(t) => {
                if painter.set_toggles(t)
                    && let Some(d) = &mut deck
                    && let Err(e) = painter.draw_keys(d)
                {
                    tracing::warn!(error = %e, "redraw after toggle change failed");
                }
            }
            DeckEvent::Disconnected => {
                tracing::info!("Stream Deck + disconnected");
                deck = None;
                gestures = Recognizer::default();
            }
            DeckEvent::Ipc(cmd, reply) => {
                let resp = match painter.command(deck.as_mut(), &cmd) {
                    Ok(()) => duckydeck_core::ipc::Response::ok(painter.status(deck.as_ref())),
                    Err(e) => duckydeck_core::ipc::Response::error(e),
                };
                let _ = reply.send(resp);
            }
            DeckEvent::Input(i) => {
                tracing::debug!(input = ?i, "input");
                for g in gestures.input(i, Instant::now()) {
                    on_gesture(&mut painter, &mut deck, g);
                }
            }
        }
    }
    Ok(())
}

fn log_config(store: &duckydeck_core::config::Store) {
    let c = &store.current;
    tracing::info!(
        dir = %store.dir().display(),
        profile = %c.config.profile,
        profiles = c.profiles.len(),
        "config loaded"
    );
}

/// Catalog actions whose omarchy route is missing; empty if the check fails.
async fn unavailable_actions(
    catalog: &duckydeck_core::catalog::Catalog,
    runner: &dyn duckydeck_core::CommandRunner,
) -> std::collections::BTreeSet<String> {
    let spec = duckydeck_core::CommandSpec::omarchy(["commands", "--json"]);
    let routes = match runner.run(&spec).await {
        Ok(out) if out.success() => duckydeck_core::catalog::parse_routes(&out.stdout),
        other => {
            tracing::warn!(result = ?other, "route check skipped");
            return Default::default();
        }
    };
    match routes {
        Ok(r) => {
            let missing = catalog.unavailable(&r);
            if !missing.is_empty() {
                tracing::warn!(actions = ?missing, "actions disabled: omarchy route missing");
            }
            missing
        }
        Err(e) => {
            tracing::warn!(error = %e, "route check skipped");
            Default::default()
        }
    }
}

fn on_gesture(p: &mut screen::Screen, deck: &mut Option<Deck>, g: gesture::Gesture) {
    tracing::info!(gesture = ?g, "gesture");
    if let Some(d) = deck {
        p.on_gesture(d, g);
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(t) => tokio::time::sleep_until(t.into()).await,
        None => std::future::pending().await,
    }
}

fn spawn_hotplug(hotplug_tx: mpsc::UnboundedSender<DeckEvent>) -> Result<()> {
    // The udev monitor is not `Sync`, so it gets its own single-threaded runtime.
    std::thread::Builder::new()
        .name("hotplug".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build();
            let res = rt
                .map_err(anyhow::Error::from)
                .and_then(|rt| rt.block_on(device::watch_hotplug(hotplug_tx)));
            if let Err(e) = res {
                tracing::error!(error = %e, "hotplug watcher stopped");
            }
        })?;
    Ok(())
}

fn try_connect(p: &mut screen::Screen, tx: &mpsc::UnboundedSender<DeckEvent>) -> Option<Deck> {
    match device::connect() {
        Ok(mut d) => {
            tracing::info!(serial = %d.serial, "Stream Deck + connected");
            if let Err(e) = p.draw(&mut d) {
                tracing::warn!(error = %e, "initial draw failed");
            }
            if let Err(e) = d.start_input(tx.clone()) {
                tracing::error!(error = %e, "starting input failed");
                return None;
            }
            Some(d)
        }
        Err(e) => {
            tracing::info!(reason = %e, "waiting for Stream Deck +");
            None
        }
    }
}

/// Plug & Play: run `duckydeck setup` once per installed version.
async fn first_start_setup(runner: &duckydeck_core::TokioRunner) {
    use duckydeck_core::setup;
    let paths = match setup::Paths::detect() {
        Ok(p) if setup::needed(&p) => p,
        Ok(_) => return,
        Err(e) => return tracing::warn!(error = %e, "setup skipped"),
    };
    match setup::install(&paths, runner).await {
        Ok(done) => tracing::info!(?done, "setup done"),
        Err(e) => tracing::warn!(error = %e, "setup failed"),
    }
}
