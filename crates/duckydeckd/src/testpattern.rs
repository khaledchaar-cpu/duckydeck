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

pub fn on_input(deck: &Deck, u: &DeviceStateUpdate) {
    let res = match *u {
        DeviceStateUpdate::ButtonDown(i) => deck.out.set_button_image(i, key_image(i, true)),
        DeviceStateUpdate::ButtonUp(i) => deck.out.set_button_image(i, key_image(i, false)),
        _ => return,
    };
    if let Err(e) = res.and_then(|()| deck.out.flush()) {
        tracing::warn!(error = %e, "key update failed");
    }
}

pub fn draw_strip(deck: &Deck) -> Result<()> {
    for seg in 0..4u8 {
        let img = RgbImage::from_pixel(200, 100, hue(seg * 2 + 1));
        let rect = ImageRect::from_image(DynamicImage::ImageRgb8(img))?;
        deck.out.write_lcd(u16::from(seg) * 200, 0, &rect)?;
    }
    Ok(())
}
