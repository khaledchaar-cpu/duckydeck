//! Media keys over MPRIS: which player a key controls and what it shows.
//!
//! The D-Bus side lives in the daemon; this module only decides.

use std::time::{Duration, Instant};

use crate::config::Binding;

pub const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKey {
    PlayPause,
    Next,
    Previous,
}

impl MediaKey {
    pub fn from_binding(b: &Binding) -> Option<Self> {
        match b.action.as_str() {
            "media.play_pause" => Some(Self::PlayPause),
            "media.next" => Some(Self::Next),
            "media.previous" => Some(Self::Previous),
            _ => None,
        }
    }

    /// Method on `org.mpris.MediaPlayer2.Player`.
    pub fn method(self) -> &'static str {
        match self {
            Self::PlayPause => "PlayPause",
            Self::Next => "Next",
            Self::Previous => "Previous",
        }
    }

    /// Icon for the current state: play/pause shows what a press does.
    pub fn icon(self, playing: bool) -> &'static str {
        match self {
            Self::PlayPause if playing => "pause",
            Self::PlayPause => "play",
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }
}

/// Preferred player from `args = { player = "spotify" }`.
pub fn wanted_player(b: &Binding) -> Option<&str> {
    b.args.get("player").and_then(|v| v.as_str())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl Status {
    /// `PlaybackStatus` property value.
    pub fn parse(s: &str) -> Self {
        match s {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            _ => Self::Stopped,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Player {
    /// Well-known name, e.g. `org.mpris.MediaPlayer2.spotify`.
    pub name: String,
    pub status: Status,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// Increases with each change to "Playing"; higher = more recent.
    pub last_played: u64,
    /// Track length (`mpris:length`).
    pub length: Option<Duration>,
    /// Last reported position and when it was reported.
    pub position: Option<(Duration, Instant)>,
}

impl Player {
    /// Position at `now`, extrapolated while playing, capped at the length.
    pub fn position_at(&self, now: Instant) -> Option<Duration> {
        let (pos, at) = self.position?;
        let pos = match self.status {
            Status::Playing => pos + now.saturating_duration_since(at),
            _ => pos,
        };
        Some(self.length.map_or(pos, |l| pos.min(l)))
    }

    /// 0.0–1.0 of the track; `None` without length or position.
    pub fn progress(&self, now: Instant) -> Option<f32> {
        let len = self.length.filter(|l| !l.is_zero())?;
        Some(self.position_at(now)?.as_secs_f32() / len.as_secs_f32())
    }

    /// `1:23 / 4:56`, or just the position without a length.
    pub fn time_text(&self, now: Instant) -> Option<String> {
        let pos = clock(self.position_at(now)?);
        Some(match self.length {
            Some(l) if !l.is_zero() => format!("{pos} / {}", clock(l)),
            _ => pos,
        })
    }

    /// Name without the MPRIS prefix and instance suffix (`chromium`).
    pub fn short_name(&self) -> &str {
        let s = self.name.strip_prefix(MPRIS_PREFIX).unwrap_or(&self.name);
        s.split('.').next().unwrap_or(s)
    }
}

/// `m:ss`, or `h:mm:ss` from one hour on.
pub fn clock(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// All known players.
#[derive(Debug, Clone, Default)]
pub struct Players {
    pub list: Vec<Player>,
    clock: u64,
}

impl Players {
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Player> {
        self.list.iter_mut().find(|p| p.name == name)
    }

    /// Adds a player or returns the existing one.
    pub fn insert(&mut self, name: &str) -> &mut Player {
        if let Some(i) = self.list.iter().position(|p| p.name == name) {
            return &mut self.list[i];
        }
        self.list.push(Player {
            name: name.to_owned(),
            ..Player::default()
        });
        let last = self.list.len() - 1;
        &mut self.list[last]
    }

    pub fn remove(&mut self, name: &str) {
        self.list.retain(|p| p.name != name);
    }

    pub fn set_status(&mut self, name: &str, status: Status) {
        self.clock += 1;
        let clock = self.clock;
        let p = self.insert(name);
        if status == Status::Playing && p.status != Status::Playing {
            p.last_played = clock;
        }
        p.status = status;
    }

    /// The player a key controls: the wanted one if running, else a playing
    /// one (most recently started), else the most recently played.
    pub fn active(&self, wanted: Option<&str>) -> Option<&Player> {
        if let Some(w) = wanted
            && let Some(p) = self.list.iter().find(|p| p.short_name() == w)
        {
            return Some(p);
        }
        self.list
            .iter()
            .max_by_key(|p| (p.status == Status::Playing, p.last_played))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> String {
        format!("{MPRIS_PREFIX}{s}")
    }

    #[test]
    fn picks_active_player() {
        let mut ps = Players::default();
        assert!(ps.active(None).is_none());
        ps.set_status(&name("spotify"), Status::Playing);
        ps.set_status(&name("chromium.instance42"), Status::Playing);
        assert_eq!(ps.active(None).unwrap().short_name(), "chromium");
        ps.set_status(&name("chromium.instance42"), Status::Paused);
        assert_eq!(ps.active(None).unwrap().short_name(), "spotify");
        ps.set_status(&name("spotify"), Status::Paused);
        // Nothing plays: the one started last.
        assert_eq!(ps.active(None).unwrap().short_name(), "chromium");
        assert_eq!(ps.active(Some("spotify")).unwrap().short_name(), "spotify");
        assert_eq!(ps.active(Some("vlc")).unwrap().short_name(), "chromium");
        ps.remove(&name("chromium.instance42"));
        assert_eq!(ps.list.len(), 1);
    }

    #[test]
    fn extrapolates_position() {
        let t0 = Instant::now();
        let mut p = Player {
            status: Status::Playing,
            length: Some(Duration::from_secs(200)),
            position: Some((Duration::from_secs(50), t0)),
            ..Player::default()
        };
        let t = t0 + Duration::from_secs(50);
        assert_eq!(p.progress(t), Some(0.5));
        assert_eq!(p.time_text(t).as_deref(), Some("1:40 / 3:20"));
        assert_eq!(
            p.position_at(t0 + Duration::from_secs(999)),
            Some(Duration::from_secs(200))
        );
        p.status = Status::Paused;
        assert_eq!(p.time_text(t).as_deref(), Some("0:50 / 3:20"));
        p.length = None;
        assert_eq!(p.progress(t), None);
        assert_eq!(clock(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn maps_keys() {
        let b = Binding {
            action: "media.play_pause".into(),
            args: toml::from_str("player = \"spotify\"").unwrap(),
            label: None,
            icon: None,
        };
        assert_eq!(MediaKey::from_binding(&b), Some(MediaKey::PlayPause));
        assert_eq!(wanted_player(&b), Some("spotify"));
        assert_eq!(MediaKey::PlayPause.icon(true), "pause");
        assert_eq!(MediaKey::Next.method(), "Next");
        assert_eq!(Status::parse("Playing"), Status::Playing);
    }
}
