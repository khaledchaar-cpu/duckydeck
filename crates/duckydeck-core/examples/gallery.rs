//! All built-in icons in all stock Omarchy themes as one PNG for review.
//!
//! `cargo run -p duckydeck-core --example gallery [out.png]`
//! (default `target/gallery.png`).

use std::path::PathBuf;

use anyhow::Context as _;
use duckydeck_core::render::{KEY_SIZE, KeyView, Renderer, RgbImage};
use duckydeck_core::theme::Theme;
use duckydeck_core::{TokioRunner, icons};

const THEMES: &str = "/usr/share/omarchy/themes";
const COLS: u32 = 16;
const GAP: u32 = 4;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/gallery.png"), PathBuf::from);

    let mut themes = Vec::new();
    for entry in std::fs::read_dir(THEMES).context("reading stock themes")? {
        let dir = entry?.path();
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
        if let (Some(name), Ok(theme)) = (name, Theme::load(&dir.join("colors.toml"))) {
            themes.push((name, theme));
        }
    }
    themes.sort_by(|a, b| a.0.cmp(&b.0));

    let font = duckydeck_core::font::system_font(&TokioRunner).await;
    let mut renderer = Renderer::new(font.into_iter().collect());

    let per_theme = icons::ICONS.len() as u32 + 1;
    let rows_per_theme = per_theme.div_ceil(COLS);
    let cell = KEY_SIZE + GAP;
    let width = COLS * cell + GAP;
    let height = themes.len() as u32 * (rows_per_theme * cell + GAP) + GAP;
    let mut img = image::RgbImage::from_pixel(width, height, image::Rgb([40, 40, 40]));

    for (t, (name, theme)) in themes.iter().enumerate() {
        let top = GAP + t as u32 * (rows_per_theme * cell + GAP);
        let header = KeyView {
            label: Some(name),
            ..KeyView::default()
        };
        let keys = std::iter::once(header).chain(icons::ICONS.iter().map(|(n, svg)| KeyView {
            icon: Some(svg),
            label: Some(n),
            ..KeyView::default()
        }));
        for (i, view) in keys.enumerate() {
            let key = renderer.key(theme, &view)?;
            let (col, row) = (i as u32 % COLS, i as u32 / COLS);
            blit(&mut img, &key, GAP + col * cell, top + row * cell);
        }
    }

    img.save(&out)
        .with_context(|| format!("writing {}", out.display()))?;
    println!(
        "{} themes × {} icons → {}",
        themes.len(),
        icons::ICONS.len(),
        out.display()
    );
    Ok(())
}

fn blit(dst: &mut image::RgbImage, src: &RgbImage, x0: u32, y0: u32) {
    for y in 0..src.height {
        for x in 0..src.width {
            dst.put_pixel(x0 + x, y0 + y, image::Rgb(src.pixel(x, y)));
        }
    }
}
