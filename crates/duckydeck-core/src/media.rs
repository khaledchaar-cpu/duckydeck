//! Media keys over MPRIS: which player a key controls and what it shows.
//!
//! The D-Bus side lives in the daemon; this module only decides.

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
}

impl Player {
    /// Name without the MPRIS prefix and instance suffix (`chromium`).
    pub fn short_name(&self) -> &str {
        let s = self.name.strip_prefix(MPRIS_PREFIX).unwrap_or(&self.name);
        s.split('.').next().unwrap_or(s)
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
