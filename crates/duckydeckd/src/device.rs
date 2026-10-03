//! Stream Deck + connection: discovery, input thread and udev hotplug.
//! With `DUCKYDECK_FAKE_DEVICE=1` a fake deck renders to a PNG and reads
//! input lines (fixture format) from stdin.

use std::io::BufRead;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use elgato_streamdeck::info::Kind;
use elgato_streamdeck::{StreamDeck, list_devices, new_hidapi};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc::UnboundedSender;

use crate::input::Input;
use crate::surface::{FakeSurface, Surface};

/// Events delivered to the daemon's main loop.
#[derive(Debug)]
pub enum DeckEvent {
    /// A hidraw device appeared; the main loop should try to connect.
    Added,
    /// The firmware has finished booting after a plug-in; redraw the strip.
    BootDone,
    /// `omarchy theme set` replaced the current theme; reload and redraw.
    ThemeChanged,
    /// Something under the config directory changed; reload it.
    ConfigChanged,
    /// The input thread lost the device.
    Disconnected,
    Input(Input),
    /// Volume, mic or brightness changed.
    Levels(duckydeck_core::dial::Levels),
    /// An MPRIS player appeared, left or changed state.
    Media(duckydeck_core::media::Players),
    /// The focused workspace or workspace occupancy changed.
    Workspaces(duckydeck_core::hypr::Workspaces),
    /// A catalog toggle (night light, recording …) changed state.
    Toggles(crate::toggles::Toggles),
}

/// Handle used for output (images, brightness). Input runs on a separate
/// connection owned by a dedicated thread, so a blocking read never stalls writes.
pub struct Deck {
    pub serial: String,
    pub out: Box<dyn Surface>,
    input: Option<InputSource>,
}

enum InputSource {
    Hid(StreamDeck),
    Stdin,
}

impl Deck {
    /// Starts the input thread. Call after the first full draw: input reads
    /// that start while the strip image is uploading make the device drop it.
    pub fn start_input(&mut self, tx: UnboundedSender<DeckEvent>) -> Result<()> {
        let Some(input) = self.input.take() else {
            return Ok(());
        };
        thread::Builder::new()
            .name("deck-input".into())
            .spawn(move || match input {
                InputSource::Hid(dev) => input_loop(dev, tx),
                InputSource::Stdin => stdin_loop(tx),
            })
            .context("spawn input thread")?;
        Ok(())
    }
}

/// Opens the first Stream Deck +; input starts with [`Deck::start_input`].
pub fn connect() -> Result<Deck> {
    if fake_enabled() {
        return Ok(Deck {
            serial: "FAKE".into(),
            out: Box::new(FakeSurface::new()?),
            input: Some(InputSource::Stdin),
        });
    }
    let hid = new_hidapi().context("hidapi init")?;
    let Some((kind, serial)) = list_devices(&hid)
        .into_iter()
        .find(|(k, _)| *k == Kind::Plus)
    else {
        bail!("no Stream Deck + found");
    };
    let out = StreamDeck::connect(&hid, kind, &serial).context("open output handle")?;
    let input = StreamDeck::connect(&hid, kind, &serial).context("open input handle")?;
    Ok(Deck {
        serial,
        out: Box::new(out),
        input: Some(InputSource::Hid(input)),
    })
}

const READ_TIMEOUT: Duration = Duration::from_secs(3600);

fn input_loop(input: StreamDeck, tx: UnboundedSender<DeckEvent>) {
    // `get_reader` requires an `Arc`; the reader never leaves this thread.
    #[allow(clippy::arc_with_non_send_sync)]
    let reader = Arc::new(input).get_reader();
    loop {
        // `None` would put hidapi into non-blocking mode (busy loop). A long
        // timeout blocks in poll(2) until the device reports something.
        match reader.read(Some(READ_TIMEOUT)) {
            Ok(updates) => {
                for i in updates.into_iter().filter_map(Input::from_update) {
                    if tx.send(DeckEvent::Input(i)).is_err() {
                        return;
                    }
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "input read failed");
                let _ = tx.send(DeckEvent::Disconnected);
                return;
            }
        }
    }
}

pub fn fake_enabled() -> bool {
    std::env::var_os("DUCKYDECK_FAKE_DEVICE").is_some_and(|v| v != "0" && !v.is_empty())
}

/// Fake input: one event per line, e.g. `cat fixture.txt | duckydeckd`.
/// EOF just ends input; the fake deck stays "connected".
fn stdin_loop(tx: UnboundedSender<DeckEvent>) {
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { return };
        match Input::parse(&line) {
            Some(i) => {
                if tx.send(DeckEvent::Input(i)).is_err() {
                    return;
                }
            }
            None if line.trim().is_empty() || line.starts_with('#') => {}
            None => tracing::warn!(%line, "unknown fake input"),
        }
    }
}

/// Watches udev for new hidraw devices and reports them as [`DeckEvent::Added`].
pub async fn watch_hotplug(tx: UnboundedSender<DeckEvent>) -> Result<()> {
    let socket = udev::MonitorBuilder::new()?
        .match_subsystem("hidraw")?
        .listen()?;
    let fd = AsyncFd::new(socket)?;
    loop {
        let mut guard = fd.readable().await?;
        for ev in guard.get_inner().iter() {
            if ev.event_type() == udev::EventType::Add {
                tracing::debug!(dev = ?ev.devnode(), "hidraw added");
                if tx.send(DeckEvent::Added).is_err() {
                    return Ok(());
                }
            }
        }
        guard.clear_ready();
    }
}
