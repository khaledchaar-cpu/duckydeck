//! Output side of the deck: the real device or a fake that writes one PNG of
//! the whole deck to `$XDG_RUNTIME_DIR/duckydeck/fake/deck.png`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use duckydeck_core::model::{Model, SEGMENT_W};
use elgato_streamdeck::StreamDeck;
use elgato_streamdeck::images::ImageRect;
use elgato_streamdeck::info::ImageRotation;
use image::{DynamicImage, GenericImage, Rgb, RgbImage, imageops};

pub const KEY_SIZE: u32 = 120;
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
        let rotation = self
            .kind()
            .lcd_image_format()
            .map_or(ImageRotation::Rot0, |f| f.rotation);
        let (img, x, y) = to_native(img, x, rotation);
        let rect = ImageRect::from_image(DynamicImage::ImageRgb8(img))?;
        Ok(self.write_lcd(x, y, &rect)?)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(StreamDeck::flush(self)?)
    }
}

/// Maps a strip image at logical `x` (left to right) to the device's native
/// LCD orientation. The Stream Deck + XL stores its strip as a portrait
/// 100×1200 panel rotated by 270°; its native y runs left to right.
fn to_native(img: &RgbImage, x: u16, rotation: ImageRotation) -> (RgbImage, u16, u16) {
    match rotation {
        ImageRotation::Rot270 => (imageops::rotate270(img), 0, x),
        ImageRotation::Rot90 => (imageops::rotate90(img), 0, x),
        _ => (img.clone(), x, 0),
    }
}

// Composite layout: keys are spread over the strip width like on the
// device; encoders are drawn as placeholders below the strip.
const MARGIN: u32 = 20;
const ROW_GAP: u32 = 20;
const KNOB: u32 = 60;
const BODY: Rgb<u8> = Rgb([18, 18, 18]);

/// Where a key sits in the composite image.
pub fn key_origin(model: Model, key: u8) -> (u32, u32) {
    let cols = model.columns() as u32;
    let (col, row) = (u32::from(key) % cols, u32::from(key) / cols);
    let pitch = model.strip_width() / cols;
    (
        MARGIN + col * pitch + (pitch - KEY_SIZE) / 2,
        MARGIN + row * (KEY_SIZE + ROW_GAP),
    )
}

fn strip_y(model: Model) -> u32 {
    let rows = (model.keys() / model.columns()) as u32;
    MARGIN + rows * KEY_SIZE + (rows - 1) * ROW_GAP + 20
}

pub struct FakeSurface {
    model: Model,
    canvas: RgbImage,
    path: PathBuf,
}

impl FakeSurface {
    pub fn new(model: Model) -> Result<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .context("XDG_RUNTIME_DIR not set")?
            .join("duckydeck/fake");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        Ok(Self::at(model, dir.join("deck.png")))
    }

    fn at(model: Model, path: PathBuf) -> Self {
        let knob_y = strip_y(model) + STRIP_H + 20;
        let width = model.strip_width() + 2 * MARGIN;
        let mut canvas = RgbImage::from_pixel(width, knob_y + KNOB + MARGIN, BODY);
        let knob = RgbImage::from_pixel(KNOB, KNOB, Rgb([60, 60, 60]));
        for i in 0..model.dials() as u32 {
            // Fits by construction of the canvas size.
            let _ = canvas.copy_from(
                &knob,
                MARGIN + i * SEGMENT_W + (SEGMENT_W - KNOB) / 2,
                knob_y,
            );
        }
        Self {
            model,
            canvas,
            path,
        }
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
        anyhow::ensure!(
            usize::from(key) < self.model.keys(),
            "key {key} out of range"
        );
        let (x, y) = key_origin(self.model, key);
        self.blit(x, y, img)
    }

    fn set_strip(&mut self, x: u16, img: &RgbImage) -> Result<()> {
        self.blit(MARGIN + u32::from(x), strip_y(self.model), img)?;
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
        let mut f = FakeSurface::at(Model::Plus, path.clone());
        let red = Rgb([255, 0, 0]);
        let blue = Rgb([0, 0, 255]);
        f.set_key(5, &RgbImage::from_pixel(KEY_SIZE, KEY_SIZE, red))?;
        f.set_strip(600, &RgbImage::from_pixel(200, STRIP_H, blue))?;
        assert!(f.set_key(8, &RgbImage::new(KEY_SIZE, KEY_SIZE)).is_err());

        let png = image::open(&path)?.to_rgb8();
        let (kx, ky) = key_origin(Model::Plus, 5);
        let strip = strip_y(Model::Plus);
        assert_eq!(*png.get_pixel(kx + 60, ky + 60), red);
        assert_eq!(*png.get_pixel(kx - 1, ky), BODY);
        assert_eq!(*png.get_pixel(MARGIN + 700, strip + 50), blue);
        assert_eq!(*png.get_pixel(MARGIN + 100, strip + 50), BODY);
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn xl_strip_segments_map_to_the_portrait_panel() {
        let seg = RgbImage::from_pixel(200, STRIP_H, Rgb([1, 2, 3]));
        let (img, x, y) = to_native(&seg, 0, ImageRotation::Rot270);
        assert_eq!((img.dimensions(), x, y), ((100, 200), 0, 0));
        let (_, _, y) = to_native(&seg, 1000, ImageRotation::Rot270);
        assert_eq!(y, 1000);
        let (img, x, y) = to_native(&seg, 600, ImageRotation::Rot0);
        assert_eq!((img.dimensions(), x, y), ((200, 100), 600, 0));
    }

    #[test]
    fn xl_composite_has_four_rows_of_nine() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("duckydeck-fake-xl-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("deck.png");
        let mut f = FakeSurface::at(Model::PlusXl, path.clone());
        let red = Rgb([255, 0, 0]);
        f.set_key(35, &RgbImage::from_pixel(KEY_SIZE, KEY_SIZE, red))?;
        f.set_strip(1000, &RgbImage::from_pixel(200, STRIP_H, red))?;
        assert!(f.set_key(36, &RgbImage::new(KEY_SIZE, KEY_SIZE)).is_err());
        let png = image::open(&path)?.to_rgb8();
        assert_eq!(png.width(), 1200 + 2 * MARGIN);
        let (kx, ky) = key_origin(Model::PlusXl, 35);
        assert_eq!(ky, MARGIN + 3 * (KEY_SIZE + ROW_GAP));
        assert_eq!(*png.get_pixel(kx + 60, ky + 60), red);
        assert_eq!(
            *png.get_pixel(MARGIN + 1100, strip_y(Model::PlusXl) + 50),
            red
        );
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }
}
