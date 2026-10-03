//! Daemon ↔ CLI protocol: one JSON object per line over a Unix socket.
//! Schema and examples: `docs/ipc.md`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;

/// `$XDG_RUNTIME_DIR/duckydeck/duckydeck.sock`.
pub fn socket_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").filter(|s| !s.is_empty())?;
    Some(PathBuf::from(dir).join("duckydeck").join("duckydeck.sock"))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    #[serde(flatten)]
    pub cmd: Command,
}

impl Request {
    pub fn new(cmd: Command) -> Self {
        Self { v: VERSION, cmd }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Status,
    /// Runtime switch; `config.toml` is not changed.
    SetProfile {
        profile: String,
    },
    /// 1-based page of the active profile; closes an open folder.
    SetPage {
        page: usize,
    },
    /// Runtime override in percent until the next config change.
    SetBrightness {
        brightness: u8,
    },
    /// Re-reads system font, theme and config, then redraws.
    Reload,
    /// Answered with the status, then events until the client disconnects.
    Subscribe,
    /// Status plus every action the editor offers (`Response::actions`).
    ListActions,
    /// Renders a page (1-based) or folder of any profile to PNG files
    /// (`Response::preview`); the deck itself does not change.
    Preview {
        profile: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
}

/// Images of one page: 8 keys and 4 strip segments, left to right.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    /// Grows with every render; part of the file names so viewers never
    /// show a cached old image.
    pub rev: u64,
    pub keys: Vec<PathBuf>,
    pub dials: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub v: u32,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<crate::library::Item>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
}

impl Response {
    pub fn ok(status: Status) -> Self {
        Self {
            v: VERSION,
            ok: true,
            status: Some(status),
            error: None,
            actions: None,
            preview: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            v: VERSION,
            ok: false,
            status: None,
            error: Some(msg.into()),
            actions: None,
            preview: None,
        }
    }
}

/// What the deck shows right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub connected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    pub profile: String,
    /// All profile ids, sorted.
    pub profiles: Vec<String>,
    /// 1-based.
    pub page: usize,
    pub pages: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub brightness: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    pub event: EventKind,
    pub status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    DeviceConnected,
    DeviceDisconnected,
    /// Profile, page, folder or brightness changed.
    ProfileChanged,
}

impl Event {
    /// The event that turns `old` into `new`, if anything changed.
    pub fn between(old: &Status, new: &Status) -> Option<Self> {
        let event = match (old.connected, new.connected) {
            _ if old == new => return None,
            (false, true) => EventKind::DeviceConnected,
            (true, false) => EventKind::DeviceDisconnected,
            _ => EventKind::ProfileChanged,
        };
        Some(Self {
            v: VERSION,
            event,
            status: new.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Status {
        Status {
            connected: true,
            serial: Some("A1".into()),
            profile: "default".into(),
            profiles: vec!["default".into()],
            page: 1,
            pages: 2,
            folder: None,
            brightness: 60,
        }
    }

    #[test]
    fn request_wire_format() -> serde_json::Result<()> {
        let r: Request = serde_json::from_str(r#"{"v":1,"cmd":"set_page","page":2}"#)?;
        assert_eq!(r, Request::new(Command::SetPage { page: 2 }));
        assert_eq!(
            serde_json::to_string(&Request::new(Command::Status))?,
            r#"{"v":1,"cmd":"status"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::new(Command::Reload))?,
            r#"{"v":1,"cmd":"reload"}"#
        );
        assert!(serde_json::from_str::<Request>(r#"{"v":1,"cmd":"reboot"}"#).is_err());
        Ok(())
    }

    #[test]
    fn response_omits_empty_fields() -> serde_json::Result<()> {
        assert_eq!(
            serde_json::to_string(&Response::error("nope"))?,
            r#"{"v":1,"ok":false,"error":"nope"}"#
        );
        Ok(())
    }

    #[test]
    fn event_kind_from_status_change() {
        let a = status();
        assert_eq!(Event::between(&a, &a), None);
        let mut b = a.clone();
        b.page = 2;
        assert_eq!(
            Event::between(&a, &b).map(|e| e.event),
            Some(EventKind::ProfileChanged)
        );
        b.connected = false;
        assert_eq!(
            Event::between(&a, &b).map(|e| e.event),
            Some(EventKind::DeviceDisconnected)
        );
        assert_eq!(
            Event::between(&b, &a).map(|e| e.event),
            Some(EventKind::DeviceConnected)
        );
    }
}
