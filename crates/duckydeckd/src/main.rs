//! DuckyDeck daemon: owns the Stream Deck +, renders keys and runs actions.

mod device;
mod testpattern;

use std::time::Duration;

use anyhow::Result;
use device::{Deck, DeckEvent};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

const BOOT_TIME: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() -> Result<()> {
    let debug = std::env::args().any(|a| a == "--debug");
    let default = if debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default)),
        )
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "duckydeckd starting");

    let (tx, mut rx) = mpsc::unbounded_channel();
    let hotplug_tx = tx.clone();
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

    let mut deck = try_connect(&tx);
    while let Some(ev) = rx.recv().await {
        match ev {
            DeckEvent::Added if deck.is_none() => {
                deck = try_connect(&tx);
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
                if let Some(d) = &deck
                    && let Err(e) = testpattern::draw_strip(d)
                {
                    tracing::warn!(error = %e, "strip redraw failed");
                }
            }
            DeckEvent::Disconnected => {
                tracing::info!("Stream Deck + disconnected");
                deck = None;
            }
            DeckEvent::Input(u) => {
                tracing::info!(event = ?u, "input");
                if let Some(d) = &mut deck {
                    testpattern::on_input(d, &u);
                }
            }
        }
    }
    Ok(())
}

fn try_connect(tx: &mpsc::UnboundedSender<DeckEvent>) -> Option<Deck> {
    match device::connect() {
        Ok(mut d) => {
            tracing::info!(serial = %d.serial, "Stream Deck + connected");
            if let Err(e) = testpattern::draw(&d) {
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
