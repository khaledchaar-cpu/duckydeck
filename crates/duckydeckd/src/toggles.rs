//! Toggle states of catalog actions (`state` in the catalog).
//!
//! File states are watched with inotify on the nearest existing directory;
//! command states run at startup and shortly after each press (the screen
//! sends the action id). Every change goes out as one full map.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use duckydeck_core::catalog::Catalog;
use duckydeck_core::toggle::{StateSource, Toggle};
use duckydeck_core::{CommandRunner, CommandSpec, TokioRunner};
use inotify::{Inotify, WatchMask, Watches};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;

use crate::device::DeckEvent;

pub type Toggles = HashMap<String, Toggle>;

/// Status checks after a press: the toggle may take a moment (nightlight
/// retries for up to 2 s).
const RECHECKS: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(2500)];

struct Source {
    id: String,
    state: StateSource,
    path: Option<PathBuf>,
}

pub async fn run(
    catalog: Catalog,
    mut presses: UnboundedReceiver<String>,
    tx: UnboundedSender<DeckEvent>,
) -> Result<()> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;
    let sources: Vec<Source> = catalog
        .iter()
        .filter_map(|(id, e)| {
            let state = e.state.clone()?;
            Some(Source {
                id: id.clone(),
                path: state.path(&home),
                state,
            })
        })
        .collect();
    let mut toggles = Toggles::new();
    for s in &sources {
        let t = match &s.path {
            Some(p) => file_state(&s.state, p),
            None => command_state(&s.state).await,
        };
        if let Some(t) = t {
            toggles.insert(s.id.clone(), t);
        }
    }
    let _ = tx.send(DeckEvent::Toggles(toggles.clone()));

    let inotify = Inotify::init()?;
    watch_dirs(&mut inotify.watches(), &sources);
    let mut events = inotify.into_event_stream([0u8; 1024])?;
    let (due_tx, mut due) = mpsc::unbounded_channel::<String>();
    loop {
        let mut next = toggles.clone();
        tokio::select! {
            ev = events.next() => {
                if ev.is_none() {
                    break;
                }
                // Directories may have appeared; watching again is idempotent.
                watch_dirs(&mut events.watches(), &sources);
                for s in &sources {
                    if let Some(p) = &s.path {
                        set(&mut next, &s.id, file_state(&s.state, p));
                    }
                }
            }
            id = presses.recv() => {
                let Some(id) = id else { break };
                if sources.iter().any(|s| s.id == id && s.path.is_none()) {
                    for d in RECHECKS {
                        let due_tx = due_tx.clone();
                        let id = id.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(d).await;
                            let _ = due_tx.send(id);
                        });
                    }
                }
                continue;
            }
            Some(id) = due.recv() => {
                if let Some(s) = sources.iter().find(|s| s.id == id) {
                    set(&mut next, &s.id, command_state(&s.state).await);
                }
            }
        }
        if next != toggles {
            toggles = next;
            tracing::debug!(toggles = ?sorted(&toggles), "toggle states");
            if tx.send(DeckEvent::Toggles(toggles.clone())).is_err() {
                break;
            }
        }
    }
    Ok(())
}

fn set(t: &mut Toggles, id: &str, state: Option<Toggle>) {
    match state {
        Some(s) => t.insert(id.to_owned(), s),
        None => t.remove(id),
    };
}

fn sorted(t: &Toggles) -> BTreeMap<&str, bool> {
    t.iter().map(|(k, v)| (k.as_str(), v.on)).collect()
}

/// Watches the nearest existing directory above each file.
fn watch_dirs(watches: &mut Watches, sources: &[Source]) {
    let mask = WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::CLOSE_WRITE
        | WatchMask::MOVED_TO
        | WatchMask::MOVED_FROM;
    for p in sources.iter().filter_map(|s| s.path.as_deref()) {
        if let Some(dir) = p.ancestors().skip(1).find(|d| d.is_dir())
            && let Err(e) = watches.add(dir, mask)
        {
            tracing::warn!(dir = %dir.display(), error = %e, "watching toggle state failed");
        }
    }
}

fn file_state(state: &StateSource, path: &Path) -> Option<Toggle> {
    let content = std::fs::read_to_string(path).ok();
    let on = state.from_file(content.as_deref())?;
    let since = (on && state.elapsed)
        .then(|| std::fs::metadata(path).and_then(|m| m.modified()).ok())
        .flatten();
    Some(Toggle { on, since })
}

async fn command_state(state: &StateSource) -> Option<Toggle> {
    let (program, args) = state.command.split_first()?;
    let spec = CommandSpec::new(program.clone()).args(args.to_vec());
    let out = match TokioRunner.run(&spec).await {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(command = ?state.command, error = %e, "status command failed");
            return None;
        }
    };
    let on = state.from_command(out.success(), &out.stdout)?;
    Some(Toggle { on, since: None })
}
