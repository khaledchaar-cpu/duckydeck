//! Hyprland IPC: socket paths, event parsing and workspace state.
//!
//! Hyprland (Lua config) takes dispatchers as Lua expressions
//! (`hl.dsp.focus({ workspace = "2" })`) on `.socket.sock` and sends events as
//! `name>>data` lines on `.socket2.sock`. The socket I/O lives in the daemon.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

use crate::config::Binding;

pub const SCROLL_ACTION: &str = "window.workspace_scroll";
pub const WORKSPACE_ACTION: &str = "window.workspace";

/// `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/<name>`.
pub fn socket_path(name: &str) -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    Some(PathBuf::from(runtime).join("hypr").join(sig).join(name))
}

/// What an event means for the workspace state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The focused workspace changed to this id.
    Active(i32),
    /// Windows or workspaces changed; occupancy must be re-read.
    Changed,
    /// The focused window changed: class and title (empty when none).
    Window(String, String),
}

/// Parses one `.socket2.sock` line; `None` for events that do not matter.
pub fn parse_event(line: &str) -> Option<Event> {
    let (name, data) = line.split_once(">>")?;
    match name {
        // `workspacev2>>ID,NAME`, `focusedmonv2>>MON,WSID`
        "workspacev2" => data.split(',').next()?.parse().ok().map(Event::Active),
        "focusedmonv2" => data.rsplit(',').next()?.parse().ok().map(Event::Active),
        // `activewindow>>CLASS,TITLE`; the title may contain commas.
        "activewindow" => {
            let (class, title) = data.split_once(',').unwrap_or((data, ""));
            Some(Event::Window(class.to_owned(), title.to_owned()))
        }
        "openwindow" | "closewindow" | "movewindowv2" | "createworkspacev2"
        | "destroyworkspacev2" | "moveworkspacev2" => Some(Event::Changed),
        _ => None,
    }
}

/// Class and title from `j/activewindow` (`{}` when nothing has focus).
pub fn active_window(json: &str) -> Result<(String, String), serde_json::Error> {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct W {
        class: String,
        title: String,
    }
    let w: W = serde_json::from_str(json)?;
    Ok((w.class, w.title))
}

/// Focused workspace and which workspaces have windows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Workspaces {
    pub active: Option<i32>,
    pub occupied: BTreeSet<i32>,
}

impl Workspaces {
    /// Reads `j/workspaces` (all workspaces with their window count).
    pub fn set_occupied(&mut self, json: &str) -> Result<(), serde_json::Error> {
        #[derive(Deserialize)]
        struct Ws {
            id: i32,
            windows: u32,
        }
        let list: Vec<Ws> = serde_json::from_str(json)?;
        self.occupied = list
            .into_iter()
            .filter(|w| w.windows > 0)
            .map(|w| w.id)
            .collect();
        Ok(())
    }

    /// Reads `j/activeworkspace`.
    pub fn set_active(&mut self, json: &str) -> Result<(), serde_json::Error> {
        #[derive(Deserialize)]
        struct Ws {
            id: i32,
        }
        self.active = Some(serde_json::from_str::<Ws>(json)?.id);
        Ok(())
    }

    pub fn state(&self, n: i32) -> WorkspaceState {
        if self.active == Some(n) {
            WorkspaceState::Active
        } else if self.occupied.contains(&n) {
            WorkspaceState::Occupied
        } else {
            WorkspaceState::Empty
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceState {
    Active,
    Occupied,
    Empty,
}

/// Workspace number of a `window.workspace` binding.
pub fn workspace_of(b: &Binding) -> Option<i32> {
    if b.action != WORKSPACE_ACTION {
        return None;
    }
    b.args
        .get("n")
        .and_then(|v| v.as_integer())
        .and_then(|n| i32::try_from(n).ok())
}

/// Workspace scroll dial: turn = next/previous workspace on the monitor,
/// press = scratchpad (Omarchy's special workspace).
pub fn scroll(delta: i32) -> Option<String> {
    match delta {
        0 => None,
        d => Some(format!(r#"hl.dsp.focus({{ workspace = "e{d:+}" }})"#)),
    }
}

pub const SCROLL_PRESS: &str = r#"hl.dsp.workspace.toggle_special("scratchpad")"#;

/// `ok` is Hyprland's success reply to a dispatch.
pub fn dispatch_ok(reply: &str) -> bool {
    reply.trim() == "ok"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events() {
        assert_eq!(parse_event("workspacev2>>3,3"), Some(Event::Active(3)));
        assert_eq!(parse_event("focusedmonv2>>DP-1,7"), Some(Event::Active(7)));
        assert_eq!(
            parse_event("openwindow>>5f80,1,kitty,title"),
            Some(Event::Changed)
        );
        assert_eq!(parse_event("workspace>>3"), None);
        assert_eq!(
            parse_event("activewindow>>kitty,a, b"),
            Some(Event::Window("kitty".into(), "a, b".into()))
        );
        assert_eq!(parse_event("garbage"), None);
        assert_eq!(parse_event("workspacev2>>x,special"), None);
    }

    #[test]
    fn tracks_state() {
        let mut w = Workspaces::default();
        w.set_occupied(
            r#"[{"id":1,"name":"1","windows":2},{"id":3,"windows":0},{"id":-98,"windows":1}]"#,
        )
        .unwrap();
        w.set_active(r#"{"id":3,"name":"3","windows":0}"#).unwrap();
        assert_eq!(w.state(3), WorkspaceState::Active);
        assert_eq!(w.state(1), WorkspaceState::Occupied);
        assert_eq!(w.state(2), WorkspaceState::Empty);
        assert!(w.set_active("nope").is_err());
    }

    #[test]
    fn scroll_dispatches() {
        assert_eq!(
            scroll(2).as_deref(),
            Some(r#"hl.dsp.focus({ workspace = "e+2" })"#)
        );
        assert_eq!(
            scroll(-1).as_deref(),
            Some(r#"hl.dsp.focus({ workspace = "e-1" })"#)
        );
        assert_eq!(scroll(0), None);
        assert!(dispatch_ok("ok\n"));
        assert!(!dispatch_ok("error: x"));
    }

    #[test]
    fn workspace_bindings() {
        let b = Binding {
            action: WORKSPACE_ACTION.into(),
            args: toml::from_str("n = 4").unwrap(),
            label: None,
            icon: None,
        };
        assert_eq!(workspace_of(&b), Some(4));
    }
}
