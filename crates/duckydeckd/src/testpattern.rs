//! Temporary M1 test pattern: colored keys and strip, keys light up while
//! pressed and turn gray on long press; strip gestures color the strip
//! until an encoder redraws its segment. Replaced by the real renderer in M2.

use anyhow::Result;
use image::{Rgb, RgbImage};

use crate::device::Deck;
use crate::gesture::{Control, Gesture};
use crate::surface::{KEY_SIZE, STRIP_H};

const WHITE: Rgb<u8> = Rgb([255, 255, 255]);
const GRAY: Rgb<u8> = Rgb([110, 110, 110]);

fn hue(i: u8) -> Rgb<u8> {
    const COLORS: [[u8; 3]; 8] = [
        [231, 76, 60],
        [230, 126, 34],
        [241, 196, 15],
        [46, 204, 113],
        [26, 188, 156],
        [52, 152, 219],
        [155, 89, 182],
        [236, 112, 99],
    ];
    Rgb(COLORS[usize::from(i % 8)])
}

fn key(deck: &mut Deck, i: u8, c: Rgb<u8>) -> Result<()> {
    deck.out
        .set_key(i, &RgbImage::from_pixel(KEY_SIZE, KEY_SIZE, c))
}

pub fn draw(deck: &mut Deck) -> Result<()> {
    deck.out.set_brightness(60)?;
    for i in 0..8 {
        key(deck, i, hue(i))?;
    }
    draw_strip(deck)?;
    deck.out.flush()
}

pub fn on_gesture(deck: &mut Deck, g: Gesture) {
    let res = match g {
        Gesture::Down(Control::Key(i)) => key(deck, i, WHITE),
        Gesture::Up(Control::Key(i)) => key(deck, i, hue(i)),
        Gesture::LongPress(Control::Key(i)) => key(deck, i, GRAY),
        Gesture::Twist { encoder, delta, .. } => {
            let Some(level) = deck.levels.get_mut(usize::from(encoder)) else {
                return;
            };
            *level = level
                .saturating_add_signed(delta.saturating_mul(5))
                .min(100);
            let level = *level;
            draw_segment(deck, encoder, level, None)
        }
        Gesture::Down(Control::Encoder(i)) => draw_segment(deck, i, 0, Some(WHITE)),
        Gesture::LongPress(Control::Encoder(i)) => draw_segment(deck, i, 0, Some(GRAY)),
        Gesture::Up(Control::Encoder(i)) => {
            let level = deck.levels.get(usize::from(i)).copied().unwrap_or(50);
            draw_segment(deck, i, level, None)
        }
        // Strip: touched segment turns white (tap) or gray (long press);
        // a swipe fills the whole strip, blue to the left, orange to the right.
        Gesture::StripTap(x, _) => draw_segment(deck, segment(x), 0, Some(WHITE)),
        Gesture::StripLongPress(x, _) => draw_segment(deck, segment(x), 0, Some(GRAY)),
        Gesture::StripSwipe((x0, _), (x1, _)) => {
            let c = if x1 < x0 { hue(5) } else { hue(1) };
            (0..4).try_for_each(|seg| draw_segment(deck, seg, 0, Some(c)))
        }
        _ => return,
    };
    if let Err(e) = res.and_then(|()| deck.out.flush()) {
        tracing::warn!(error = %e, "test pattern update failed");
    }
}

fn segment(x: u16) -> u8 {
    u8::try_from((x / 200).min(3)).unwrap_or(3)
}

pub fn draw_strip(deck: &mut Deck) -> Result<()> {
    for seg in 0..4u8 {
        draw_segment(deck, seg, 50, None)?;
    }
    Ok(())
}

/// One strip segment above an encoder: colored bar showing `level` (0-100),
/// or filled with `solid` while the encoder is held.
fn draw_segment(deck: &mut Deck, seg: u8, level: u8, solid: Option<Rgb<u8>>) -> Result<()> {
    let fill = u32::from(level) * 2;
    let img = RgbImage::from_fn(200, STRIP_H, |x, _| match solid {
        Some(c) => c,
        None if x < fill => hue(seg * 2 + 1),
        None => Rgb([30, 30, 30]),
    });
    deck.out.set_strip(u16::from(seg) * 200, &img)
}
