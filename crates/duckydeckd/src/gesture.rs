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

/// Slots 0-7 keys, 8-11 encoders.
#[derive(Default)]
pub struct Recognizer {
    held: [Option<Held>; 12],
}

fn slot(c: Control) -> Option<usize> {
    let (i, base, max) = match c {
        Control::Key(i) => (i, 0, 8),
        Control::Encoder(i) => (i, 8, 4),
    };
    (i < max).then_some(base + usize::from(i))
}

fn control(slot: usize) -> Control {
    // Slots are < 12, so the cast cannot truncate.
    let i = u8::try_from(slot).unwrap_or(0);
    if i < 8 {
        Control::Key(i)
    } else {
        Control::Encoder(i - 8)
    }
}

impl Recognizer {
    pub fn input(&mut self, input: Input, now: Instant) -> Vec<Gesture> {
        match input {
            Input::KeyDown(i) => self.down(Control::Key(i), now),
            Input::KeyUp(i) => self.up(Control::Key(i)),
            Input::EncoderDown(i) => self.down(Control::Encoder(i), now),
            Input::EncoderUp(i) => self.up(Control::Encoder(i)),
            Input::EncoderTwist(encoder, delta) => {
                let held = slot(Control::Encoder(encoder))
                    .and_then(|s| self.held[s].as_mut())
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
            .flatten()
            .filter(|h| !h.consumed)
            .map(|h| h.since + LONG_PRESS)
            .min()
    }

    /// Fires all long presses that are due at `now`.
    pub fn tick(&mut self, now: Instant) -> Vec<Gesture> {
        let mut out = Vec::new();
        for (s, h) in self.held.iter_mut().enumerate() {
            if let Some(h) = h
                && !h.consumed
                && now >= h.since + LONG_PRESS
            {
                h.consumed = true;
                out.push(Gesture::LongPress(control(s)));
            }
        }
        out
    }

    fn down(&mut self, c: Control, now: Instant) -> Vec<Gesture> {
        let Some(s) = slot(c) else { return Vec::new() };
        self.held[s] = Some(Held {
            since: now,
            consumed: false,
        });
        vec![Gesture::Down(c)]
    }

    fn up(&mut self, c: Control) -> Vec<Gesture> {
        let Some(s) = slot(c) else { return Vec::new() };
        match self.held[s].take() {
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
    fn independent_controls_and_bad_indices() {
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
        assert!(r.input(Input::KeyDown(8), t).is_empty());
        assert!(r.input(Input::EncoderUp(9), t).is_empty());
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
