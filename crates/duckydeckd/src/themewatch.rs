//! Theme changes: `omarchy theme set` builds the new theme in `next-theme`
//! and moves it to `current/theme`, so we watch `current/` for that rename.

use std::ffi::OsStr;
use std::path::Path;

use anyhow::{Context, Result};
use inotify::{EventMask, Inotify, WatchMask};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use crate::device::DeckEvent;

/// Watches the directory holding `theme/colors.toml` and sends
/// `ThemeChanged` whenever the `theme` directory is replaced.
pub async fn watch(colors: &Path, tx: mpsc::UnboundedSender<DeckEvent>) -> Result<()> {
    let theme_dir = colors.parent().context("colors.toml has no parent")?;
    let current = theme_dir.parent().context("theme dir has no parent")?;
    let name = theme_dir.file_name().context("theme dir has no name")?;
    let inotify = Inotify::init()?;
    inotify
        .watches()
        .add(current, WatchMask::MOVED_TO | WatchMask::CREATE)
        .with_context(|| format!("watching {}", current.display()))?;
    let mut events = inotify.into_event_stream([0u8; 1024])?;
    while let Some(ev) = events.next().await {
        let ev = ev?;
        if is_theme_swap(ev.mask, ev.name.as_deref(), name) {
            tracing::info!("theme changed");
            if tx.send(DeckEvent::ThemeChanged).is_err() {
                break;
            }
        }
    }
    Ok(())
}

fn is_theme_swap(mask: EventMask, name: Option<&OsStr>, theme: &OsStr) -> bool {
    mask.contains(EventMask::ISDIR) && name == Some(theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_theme_dir_counts() {
        let t = OsStr::new("theme");
        let dir = EventMask::MOVED_TO | EventMask::ISDIR;
        assert!(is_theme_swap(dir, Some(t), t));
        assert!(!is_theme_swap(dir, Some(OsStr::new("next-theme")), t));
        assert!(!is_theme_swap(EventMask::CREATE, Some(t), t));
    }
}
