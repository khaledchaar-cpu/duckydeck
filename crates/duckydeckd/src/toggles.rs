//! Toggle states of catalog actions (`state` in the catalog).
//!
//! File states are watched with inotify on the nearest existing directory;
//! command states run at startup and shortly after each press (the screen
//! sends the action id). A press flips a file state right away and holds the
//! guess until the file changes or `HOLD` runs out: scripts like screen
//! recording write or remove their file only seconds later. Every change goes
//! out as one full map.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

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

/// How long a flipped file state waits for its file to follow (screen
/// recording finalizes the video before it removes the file).
const HOLD: Duration = Duration::from_secs(15);

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
    let (expire_tx, mut expire) = mpsc::unbounded_channel::<(String, u64)>();
    // Flipped file states: id -> (file state at the press, press number).
    let mut held: HashMap<String, (Option<Toggle>, u64)> = HashMap::new();
    let mut press_no = 0u64;
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
                        let now = file_state(&s.state, p);
                        if held.get(&s.id).is_some_and(|(before, _)| *before == now) {
                            continue;
                        }
                        held.remove(&s.id);
                        set(&mut next, &s.id, now);
                    }
                }
            }
            id = presses.recv() => {
                let Some(id) = id else { break };
                let Some(s) = sources.iter().find(|s| s.id == id) else { continue };
                if let Some(p) = &s.path {
                    let Some(cur) = next.get(&id).copied() else { continue };
                    press_no += 1;
                    let before = held.get(&id).map_or_else(|| file_state(&s.state, p), |h| h.0);
                    held.insert(id.clone(), (before, press_no));
                    let on = !cur.on;
                    let since = (on && s.state.elapsed).then(SystemTime::now);
                    next.insert(id.clone(), Toggle { on, since });
                    let expire_tx = expire_tx.clone();
                    let no = press_no;
                    tokio::spawn(async move {
                        tokio::time::sleep(HOLD).await;
                        let _ = expire_tx.send((id, no));
                    });
                } else {
                    for d in RECHECKS {
                        let due_tx = due_tx.clone();
                        let id = id.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(d).await;
                            let _ = due_tx.send(id);
                        });
                    }
                    continue;
                }
            }
            Some((id, no)) = expire.recv() => {
                if held.get(&id).is_some_and(|h| h.1 == no) {
                    held.remove(&id);
                    if let Some(s) = sources.iter().find(|s| s.id == id)
                        && let Some(p) = &s.path
                    {
                        set(&mut next, &id, file_state(&s.state, p));
                    }
                }
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
