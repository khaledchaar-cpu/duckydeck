//! Turns raw input into gestures. The firmware recognizes strip gestures
//! itself; tap vs. long press on keys and encoders is measured here.
//!
//! Pure state machine: the caller passes the current time and sleeps until
//! [`Recognizer::deadline`] instead of polling.

use std::time::{Duration, Instant};

use crate::input::Input;

pub const LONG_PRESS: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Key(u8),
    Encoder(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// Pressed down (for visual feedback, no action).
    Down(Control),
    /// Released (for visual feedback, no action).
    Up(Control),
    /// Released before [`LONG_PRESS`].
    Tap(Control),
    /// Held for [`LONG_PRESS`]; fires while still held, no `Tap` follows.
    LongPress(Control),
    /// `pressed`: the encoder is held down while turning (no tap/long press then).
    Twist {
        encoder: u8,
        delta: i8,
        pressed: bool,
    },
    StripTap(u16, u16),
    StripLongPress(u16, u16),
    StripSwipe((u16, u16), (u16, u16)),
}

#[derive(Clone, Copy)]
struct Held {
    since: Instant,
    /// Long press fired or press turned into a twist: release yields no tap.
    consumed: bool,
}

/// Controls currently held down, in press order.
#[derive(Default)]
pub struct Recognizer {
    held: Vec<(Control, Held)>,
}

impl Recognizer {
    pub fn input(&mut self, input: Input, now: Instant) -> Vec<Gesture> {
        match input {
            Input::KeyDown(i) => self.down(Control::Key(i), now),
            Input::KeyUp(i) => self.up(Control::Key(i)),
            Input::EncoderDown(i) => self.down(Control::Encoder(i), now),
            Input::EncoderUp(i) => self.up(Control::Encoder(i)),
            Input::EncoderTwist(encoder, delta) => {
                let held = self
                    .held_mut(Control::Encoder(encoder))
                    .map(|h| h.consumed = true)
                    .is_some();
                vec![Gesture::Twist {
                    encoder,
                    delta,
                    pressed: held,
                }]
            }
            Input::StripTap(x, y) => vec![Gesture::StripTap(x, y)],
            Input::StripLongPress(x, y) => vec![Gesture::StripLongPress(x, y)],
            Input::StripSwipe(a, b) => vec![Gesture::StripSwipe(a, b)],
        }
    }

    /// Earliest pending long press, if any.
    pub fn deadline(&self) -> Option<Instant> {
        self.held
            .iter()
            .map(|(_, h)| h)
            .filter(|h| !h.consumed)
            .map(|h| h.since + LONG_PRESS)
            .min()
    }

    /// Fires all long presses that are due at `now`.
    pub fn tick(&mut self, now: Instant) -> Vec<Gesture> {
        let mut out = Vec::new();
        for (c, h) in &mut self.held {
            if !h.consumed && now >= h.since + LONG_PRESS {
                h.consumed = true;
                out.push(Gesture::LongPress(*c));
            }
        }
        out
    }

    fn held_mut(&mut self, c: Control) -> Option<&mut Held> {
        self.held.iter_mut().find(|(h, _)| *h == c).map(|(_, h)| h)
    }

    fn down(&mut self, c: Control, now: Instant) -> Vec<Gesture> {
        self.held.retain(|(h, _)| *h != c);
        self.held.push((
            c,
            Held {
                since: now,
                consumed: false,
            },
        ));
        vec![Gesture::Down(c)]
    }

    fn up(&mut self, c: Control) -> Vec<Gesture> {
        let pos = self.held.iter().position(|(h, _)| *h == c);
        match pos.map(|i| self.held.remove(i).1) {
            Some(h) if !h.consumed => vec![Gesture::Up(c), Gesture::Tap(c)],
            _ => vec![Gesture::Up(c)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: Control = Control::Key(3);
    const E: Control = Control::Encoder(1);

    #[test]
    fn short_press_is_tap() {
        let t = Instant::now();
        let mut r = Recognizer::default();
        assert_eq!(r.input(Input::KeyDown(3), t), [Gesture::Down(K)]);
        assert_eq!(r.deadline(), Some(t + LONG_PRESS));
        assert!(r.tick(t + Duration::from_millis(200)).is_empty());
        assert_eq!(
            r.input(Input::KeyUp(3), t + Duration::from_millis(300)),
            [Gesture::Up(K), Gesture::Tap(K)]
        );
        assert_eq!(r.deadline(), None);
    }

    #[test]
    fn xl_keys_and_dials_tap() {
        let t = Instant::now();
        let mut r = Recognizer::default();
        let (k, e) = (Control::Key(35), Control::Encoder(5));
        r.input(Input::KeyDown(35), t);
        r.input(Input::EncoderDown(5), t);
        assert_eq!(
            r.input(Input::KeyUp(35), t),
            [Gesture::Up(k), Gesture::Tap(k)]
        );
        assert_eq!(
            r.input(Input::EncoderUp(5), t),
            [Gesture::Up(e), Gesture::Tap(e)]
        );
    }

    #[test]
    fn long_press_fires_while_held() {
        let t = Instant::now();
        let mut r = Recognizer::default();
        r.input(Input::EncoderDown(1), t);
        assert_eq!(r.tick(t + LONG_PRESS), [Gesture::LongPress(E)]);
        assert_eq!(r.deadline(), None);
        assert!(r.tick(t + LONG_PRESS * 2).is_empty());
        assert_eq!(r.input(Input::EncoderUp(1), t), [Gesture::Up(E)]);
    }

    #[test]
    fn twist_while_pressed_cancels_tap() {
        let t = Instant::now();
        let mut r = Recognizer::default();
        r.input(Input::EncoderDown(1), t);
        assert_eq!(
            r.input(Input::EncoderTwist(1, -2), t),
            [Gesture::Twist {
                encoder: 1,
                delta: -2,
                pressed: true
            }]
        );
        assert_eq!(r.deadline(), None);
        assert_eq!(r.input(Input::EncoderUp(1), t), [Gesture::Up(E)]);
        assert_eq!(
            r.input(Input::EncoderTwist(1, 1), t),
            [Gesture::Twist {
                encoder: 1,
                delta: 1,
                pressed: false
            }]
        );
    }

    #[test]
    fn independent_controls_and_stray_release() {
        let t = Instant::now();
        let mut r = Recognizer::default();
        r.input(Input::KeyDown(0), t);
        r.input(Input::KeyDown(7), t + Duration::from_millis(100));
        assert_eq!(
            r.tick(t + LONG_PRESS),
            [Gesture::LongPress(Control::Key(0))]
        );
        assert_eq!(
            r.deadline(),
            Some(t + Duration::from_millis(100) + LONG_PRESS)
        );
        // A release without press (held before the daemon started) is no tap.
        assert_eq!(
            r.input(Input::EncoderUp(5), t),
            [Gesture::Up(Control::Encoder(5))]
        );
    }

    #[test]
    fn real_fixture_yields_expected_gestures() {
        let t = Instant::now();
        let mut r = Recognizer::default();
        let out: Vec<_> = include_str!("../tests/fixtures/real-events.txt")
            .lines()
            .filter_map(Input::parse)
            .flat_map(|i| r.input(i, t))
            .filter(|g| !matches!(g, Gesture::Down(_) | Gesture::Up(_)))
            .collect();
        assert_eq!(
            out[..4],
            [
                Gesture::Tap(Control::Key(4)),
                Gesture::Tap(Control::Key(0)),
                Gesture::Tap(Control::Key(7)),
                Gesture::Tap(Control::Encoder(0)),
            ]
        );
        // The second encoder press is turned while held: no tap.
        assert!(!out[4..].iter().any(|g| matches!(g, Gesture::Tap(_))));
        assert_eq!(
            out.iter()
                .filter(|g| matches!(g, Gesture::StripSwipe(..)))
                .count(),
            3
        );
    }
}
