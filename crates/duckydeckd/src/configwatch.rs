//! Config live reload: watches the config directory, `profiles/` and, while
//! those do not exist yet, their parents. Editors save via write, rename or
//! delete+create, so any change in a watched directory counts; bursts are
//! debounced into one `ConfigChanged`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use inotify::{Inotify, WatchMask, Watches};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use crate::device::DeckEvent;

const DEBOUNCE: Duration = Duration::from_millis(200);

pub async fn watch(dir: PathBuf, tx: mpsc::UnboundedSender<DeckEvent>) -> Result<()> {
    let inotify = Inotify::init()?;
    let mut watches = inotify.watches();
    add_watches(&mut watches, &dir);
    let mut events = inotify.into_event_stream([0u8; 4096])?;
    while let Some(ev) = events.next().await {
        let mut relevant = is_relevant(ev?.name.as_deref());
        // Drain the burst of events one save produces.
        while let Ok(Some(ev)) = tokio::time::timeout(DEBOUNCE, events.next()).await {
            relevant |= is_relevant(ev?.name.as_deref());
        }
        // Directories may have been created meanwhile; adding is idempotent.
        add_watches(&mut watches, &dir);
        if relevant {
            tracing::debug!("config changed");
            if tx.send(DeckEvent::ConfigChanged).is_err() {
                break;
            }
        }
    }
    Ok(())
}

/// Watches `dir` and `dir/profiles`, or the nearest existing ancestor of each.
fn add_watches(watches: &mut Watches, dir: &Path) {
    let mask = WatchMask::CLOSE_WRITE
        | WatchMask::MOVED_TO
        | WatchMask::MOVED_FROM
        | WatchMask::CREATE
        | WatchMask::DELETE;
    for target in [dir.join("profiles"), dir.to_owned()] {
        if let Some(existing) = target.ancestors().find(|p| p.is_dir())
            && let Err(e) = watches.add(existing, mask)
        {
            tracing::warn!(path = %existing.display(), error = %e, "config watch failed");
        }
    }
}

/// TOML files and our own directories; skips editor swap/backup files and
/// unrelated entries of watched ancestors such as `~/.config`.
fn is_relevant(name: Option<&OsStr>) -> bool {
    let Some(name) = name.and_then(OsStr::to_str) else {
        return false;
    };
    matches!(name, "duckydeck" | "profiles") || (name.ends_with(".toml") && !name.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_editor_temp_files() {
        assert!(is_relevant(Some(OsStr::new("config.toml"))));
        assert!(is_relevant(Some(OsStr::new("duckydeck"))));
        assert!(!is_relevant(Some(OsStr::new(".config.toml.swp"))));
        assert!(!is_relevant(Some(OsStr::new("work.toml~"))));
        assert!(!is_relevant(Some(OsStr::new("4913"))));
        assert!(!is_relevant(Some(OsStr::new("mimeapps.list"))));
        assert!(!is_relevant(None));
    }
}
