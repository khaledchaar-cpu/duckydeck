//! Output side of the deck: the real device or a fake that writes one PNG of
//! the whole deck to `$XDG_RUNTIME_DIR/duckydeck/fake/deck.png`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use elgato_streamdeck::StreamDeck;
use elgato_streamdeck::images::ImageRect;
use image::{DynamicImage, GenericImage, Rgb, RgbImage};

pub const KEY_SIZE: u32 = 120;
pub const STRIP_W: u32 = 800;
pub const STRIP_H: u32 = 100;

pub trait Surface: Send {
    fn set_brightness(&mut self, percent: u8) -> Result<()>;
    /// Queues a 120x120 key image; visible after [`Surface::flush`].
    fn set_key(&mut self, key: u8, img: &RgbImage) -> Result<()>;
    /// Writes part of the strip at `x` (shown immediately on the device).
    fn set_strip(&mut self, x: u16, img: &RgbImage) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
}

impl Surface for StreamDeck {
    fn set_brightness(&mut self, percent: u8) -> Result<()> {
        Ok(StreamDeck::set_brightness(self, percent)?)
    }

    fn set_key(&mut self, key: u8, img: &RgbImage) -> Result<()> {
        Ok(self.set_button_image(key, DynamicImage::ImageRgb8(img.clone()))?)
    }

    fn set_strip(&mut self, x: u16, img: &RgbImage) -> Result<()> {
        let rect = ImageRect::from_image(DynamicImage::ImageRgb8(img.clone()))?;
        Ok(self.write_lcd(x, 0, &rect)?)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(StreamDeck::flush(self)?)
    }
}

// Composite layout: keys are centered over the 200 px strip segments, like
// on the device; encoders are drawn as placeholders below the strip.
const MARGIN: u32 = 20;
const ROW_GAP: u32 = 20;
const STRIP_Y: u32 = MARGIN + 2 * KEY_SIZE + ROW_GAP + 20;
const KNOB: u32 = 60;
const KNOB_Y: u32 = STRIP_Y + STRIP_H + 20;
const WIDTH: u32 = STRIP_W + 2 * MARGIN;
const HEIGHT: u32 = KNOB_Y + KNOB + MARGIN;
const BODY: Rgb<u8> = Rgb([18, 18, 18]);

pub fn key_origin(key: u8) -> (u32, u32) {
    let (col, row) = (u32::from(key % 4), u32::from(key / 4));
    let seg = STRIP_W / 4;
    (
        MARGIN + col * seg + (seg - KEY_SIZE) / 2,
        MARGIN + row * (KEY_SIZE + ROW_GAP),
    )
}

pub struct FakeSurface {
    canvas: RgbImage,
    path: PathBuf,
}

impl FakeSurface {
    pub fn new() -> Result<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .context("XDG_RUNTIME_DIR not set")?
            .join("duckydeck/fake");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        Ok(Self::at(dir.join("deck.png")))
    }

    fn at(path: PathBuf) -> Self {
        let mut canvas = RgbImage::from_pixel(WIDTH, HEIGHT, BODY);
        let knob = RgbImage::from_pixel(KNOB, KNOB, Rgb([60, 60, 60]));
        let seg = STRIP_W / 4;
        for i in 0..4 {
            // Fits by construction of WIDTH/HEIGHT.
            let _ = canvas.copy_from(&knob, MARGIN + i * seg + (seg - KNOB) / 2, KNOB_Y);
        }
        Self { canvas, path }
    }

    fn blit(&mut self, x: u32, y: u32, img: &RgbImage) -> Result<()> {
        self.canvas
            .copy_from(img, x, y)
            .context("image outside the deck")
    }
}

impl Surface for FakeSurface {
    fn set_brightness(&mut self, _percent: u8) -> Result<()> {
        Ok(())
    }

    fn set_key(&mut self, key: u8, img: &RgbImage) -> Result<()> {
        anyhow::ensure!(key < 8, "key {key} out of range");
        let (x, y) = key_origin(key);
        self.blit(x, y, img)
    }

    fn set_strip(&mut self, x: u16, img: &RgbImage) -> Result<()> {
        self.blit(MARGIN + u32::from(x), STRIP_Y, img)?;
        // The device shows strip writes immediately.
        self.flush()
    }

    fn flush(&mut self) -> Result<()> {
        // Write + rename so viewers never see a half-written file.
        let tmp = self.path.with_extension("png.tmp");
        self.canvas
            .save_with_format(&tmp, image::ImageFormat::Png)
            .with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path).context("rename deck.png")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_places_keys_and_strip() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("duckydeck-fake-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("deck.png");
        let mut f = FakeSurface::at(path.clone());
        let red = Rgb([255, 0, 0]);
        let blue = Rgb([0, 0, 255]);
        f.set_key(5, &RgbImage::from_pixel(KEY_SIZE, KEY_SIZE, red))?;
        f.set_strip(600, &RgbImage::from_pixel(200, STRIP_H, blue))?;
        assert!(f.set_key(8, &RgbImage::new(KEY_SIZE, KEY_SIZE)).is_err());

        let png = image::open(&path)?.to_rgb8();
        assert_eq!(png.dimensions(), (WIDTH, HEIGHT));
        let (kx, ky) = key_origin(5);
        assert_eq!(*png.get_pixel(kx + 60, ky + 60), red);
        assert_eq!(*png.get_pixel(kx - 1, ky), BODY);
        assert_eq!(*png.get_pixel(MARGIN + 700, STRIP_Y + 50), blue);
        assert_eq!(*png.get_pixel(MARGIN + 100, STRIP_Y + 50), BODY);
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }
}
