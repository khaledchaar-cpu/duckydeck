//! Temporary M1 test pattern: colored keys and strip, keys light up while pressed.
//! Replaced by the real renderer in M2.

use anyhow::Result;
use elgato_streamdeck::DeviceStateUpdate;
use elgato_streamdeck::images::ImageRect;
use image::{DynamicImage, Rgb, RgbImage};

use crate::device::Deck;

const KEY: u32 = 120;

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

fn key_image(i: u8, pressed: bool) -> DynamicImage {
    let c = if pressed {
        Rgb([255, 255, 255])
    } else {
        hue(i)
    };
    DynamicImage::ImageRgb8(RgbImage::from_pixel(KEY, KEY, c))
}

pub fn draw(deck: &Deck) -> Result<()> {
    let d = &deck.out;
    d.set_brightness(60)?;
    for i in 0..8 {
        d.set_button_image(i, key_image(i, false))?;
    }
    draw_strip(deck)?;
    d.flush()?;
    Ok(())
}

pub fn on_input(deck: &mut Deck, u: &DeviceStateUpdate) {
    let res = match *u {
        DeviceStateUpdate::ButtonDown(i) => deck.out.set_button_image(i, key_image(i, true)),
        DeviceStateUpdate::ButtonUp(i) => deck.out.set_button_image(i, key_image(i, false)),
        DeviceStateUpdate::EncoderTwist(i, delta) => {
            let Some(level) = deck.levels.get_mut(usize::from(i)) else {
                return;
            };
            *level = level
                .saturating_add_signed(delta.saturating_mul(5))
                .min(100);
            let level = *level;
            log_err(draw_segment(deck, i, level, false));
            return;
        }
        DeviceStateUpdate::EncoderDown(i) => {
            log_err(draw_segment(deck, i, 0, true));
            return;
        }
        DeviceStateUpdate::EncoderUp(i) => {
            let level = deck.levels.get(usize::from(i)).copied().unwrap_or(50);
            log_err(draw_segment(deck, i, level, false));
            return;
        }
        _ => return,
    };
    if let Err(e) = res.and_then(|()| deck.out.flush()) {
        tracing::warn!(error = %e, "key update failed");
    }
}

pub fn draw_strip(deck: &Deck) -> Result<()> {
    for seg in 0..4u8 {
        draw_segment(deck, seg, 50, false)?;
    }
    Ok(())
}

/// One strip segment above an encoder: colored bar showing `level` (0-100),
/// all white while the encoder is pressed.
fn draw_segment(deck: &Deck, seg: u8, level: u8, pressed: bool) -> Result<()> {
    let fill = u32::from(level) * 2;
    let img = RgbImage::from_fn(200, 100, |x, _| {
        if pressed {
            Rgb([255, 255, 255])
        } else if x < fill {
            hue(seg * 2 + 1)
        } else {
            Rgb([30, 30, 30])
        }
    });
    let rect = ImageRect::from_image(DynamicImage::ImageRgb8(img))?;
    deck.out.write_lcd(u16::from(seg) * 200, 0, &rect)?;
    Ok(())
}

fn log_err(res: Result<()>) {
    if let Err(e) = res {
        tracing::warn!(error = %e, "strip update failed");
    }
}
