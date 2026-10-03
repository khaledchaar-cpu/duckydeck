//! DuckyDeck daemon: owns the Stream Deck +, renders keys and runs actions.

mod configwatch;
mod device;
mod gesture;
mod input;
mod surface;
mod testpattern;
mod themewatch;

use std::time::{Duration, Instant};

use anyhow::Result;
use device::{Deck, DeckEvent};
use gesture::Recognizer;
use tokio::sync::mpsc;
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
    let config_dir = duckydeck_core::config::dir()
        .ok_or_else(|| anyhow::anyhow!("neither XDG_CONFIG_HOME nor HOME is set"))?;
    let mut store = duckydeck_core::config::Store::open(config_dir.clone(), &runner)?;
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
    let mut painter = testpattern::Painter::new(font)?;
    let mut deck = try_connect(&mut painter, &tx);
    let mut gestures = Recognizer::default();
    loop {
        let deadline = gestures.deadline();
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
                    && let Err(e) = testpattern::draw_strip(&mut painter, d)
                {
                    tracing::warn!(error = %e, "strip redraw failed");
                }
            }
            DeckEvent::ThemeChanged => {
                painter.reload_theme();
                if let Some(d) = &mut deck
                    && let Err(e) = testpattern::draw(&mut painter, d)
                {
                    tracing::warn!(error = %e, "redraw after theme change failed");
                }
            }
            DeckEvent::ConfigChanged => {
                if store.reload(&runner) {
                    log_config(&store);
                }
            }
            DeckEvent::Disconnected => {
                tracing::info!("Stream Deck + disconnected");
                deck = None;
                gestures = Recognizer::default();
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

fn on_gesture(p: &mut testpattern::Painter, deck: &mut Option<Deck>, g: gesture::Gesture) {
    tracing::info!(gesture = ?g, "gesture");
    if let Some(d) = deck {
        testpattern::on_gesture(p, d, g);
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

fn try_connect(
    p: &mut testpattern::Painter,
    tx: &mpsc::UnboundedSender<DeckEvent>,
) -> Option<Deck> {
    match device::connect() {
        Ok(mut d) => {
            tracing::info!(serial = %d.serial, "Stream Deck + connected");
            if let Err(e) = testpattern::draw(p, &mut d) {
                tracing::warn!(error = %e, "drawing test pattern failed");
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
