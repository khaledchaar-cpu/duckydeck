//! Raw device input, independent of the HID library so the fake device and
//! tests can produce it from text (format of `tests/fixtures/real-events.txt`).

use elgato_streamdeck::DeviceStateUpdate;

/// One raw report from the device. Keys 0-7 row-major, encoders 0-3, strip
/// coordinates in pixels (800x100).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    KeyDown(u8),
    KeyUp(u8),
    EncoderDown(u8),
    EncoderUp(u8),
    EncoderTwist(u8, i8),
    StripTap(u16, u16),
    StripLongPress(u16, u16),
    StripSwipe((u16, u16), (u16, u16)),
}

impl Input {
    /// Converts a library update; `None` for reports the Stream Deck + never sends.
    pub fn from_update(u: DeviceStateUpdate) -> Option<Self> {
        Some(match u {
            DeviceStateUpdate::ButtonDown(i) => Self::KeyDown(i),
            DeviceStateUpdate::ButtonUp(i) => Self::KeyUp(i),
            DeviceStateUpdate::EncoderDown(i) => Self::EncoderDown(i),
            DeviceStateUpdate::EncoderUp(i) => Self::EncoderUp(i),
            DeviceStateUpdate::EncoderTwist(i, d) => Self::EncoderTwist(i, d),
            DeviceStateUpdate::TouchScreenPress(x, y) => Self::StripTap(x, y),
            DeviceStateUpdate::TouchScreenLongPress(x, y) => Self::StripLongPress(x, y),
            DeviceStateUpdate::TouchScreenSwipe(a, b) => Self::StripSwipe(a, b),
            DeviceStateUpdate::TouchPointDown(_) | DeviceStateUpdate::TouchPointUp(_) => {
                return None;
            }
        })
    }

    /// Parses one line in `DeviceStateUpdate` debug format, e.g.
    /// `EncoderTwist(0, -1)` or `TouchScreenSwipe((238, 26), (183, 29))`.
    /// Blank lines and `#` comments yield `None`, as do unknown lines.
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (name, rest) = line.split_once('(')?;
        let n: Vec<i32> = rest
            .split(|c: char| !(c.is_ascii_digit() || c == '-'))
            .filter(|s| !s.is_empty())
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        let u8_at = |i: usize| n.get(i).and_then(|v| u8::try_from(*v).ok());
        let u16_at = |i: usize| n.get(i).and_then(|v| u16::try_from(*v).ok());
        Some(match (name, n.len()) {
            ("ButtonDown", 1) => Self::KeyDown(u8_at(0)?),
            ("ButtonUp", 1) => Self::KeyUp(u8_at(0)?),
            ("EncoderDown", 1) => Self::EncoderDown(u8_at(0)?),
            ("EncoderUp", 1) => Self::EncoderUp(u8_at(0)?),
            ("EncoderTwist", 2) => Self::EncoderTwist(u8_at(0)?, i8::try_from(n[1]).ok()?),
            ("TouchScreenPress", 2) => Self::StripTap(u16_at(0)?, u16_at(1)?),
            ("TouchScreenLongPress", 2) => Self::StripLongPress(u16_at(0)?, u16_at(1)?),
            ("TouchScreenSwipe", 4) => {
                Self::StripSwipe((u16_at(0)?, u16_at(1)?), (u16_at(2)?, u16_at(3)?))
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_fixture() {
        let text = include_str!("../tests/fixtures/real-events.txt");
        let lines: Vec<_> = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .collect();
        let parsed: Vec<_> = lines.iter().filter_map(|l| Input::parse(l)).collect();
        assert_eq!(parsed.len(), lines.len(), "every fixture line parses");
        assert_eq!(parsed[0], Input::KeyDown(4));
        assert!(parsed.contains(&Input::EncoderTwist(0, -1)));
        assert!(parsed.contains(&Input::StripSwipe((238, 26), (183, 29))));
        assert!(parsed.contains(&Input::StripTap(772, 89)));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(Input::parse("# comment"), None);
        assert_eq!(Input::parse("ButtonDown(300)"), None);
        assert_eq!(Input::parse("Nope(1)"), None);
        assert_eq!(Input::parse("EncoderTwist(0)"), None);
    }
}
