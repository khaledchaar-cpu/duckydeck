//! Key renderer: monochrome SVG icon tinted with a theme token, optional
//! one-line label below, on a theme background. Output is raw RGB888.

use cosmic_text::{
    Align, Attrs, Buffer, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping,
    SwashCache, Wrap,
};
use resvg::tiny_skia::{self, Paint, Pixmap, Rect, Transform};
use resvg::usvg;

use crate::theme::{MIN_CONTRAST, Role, Theme};

pub const KEY_SIZE: u32 = 120;

const ICON_WITH_LABEL: f32 = 56.0;
const ICON_ALONE: f32 = 64.0;
const LABEL_PX: f32 = 13.0;
const LABEL_PAD: f32 = 6.0;
/// Icon area top edge when a label is shown; label sits below it.
const ICON_TOP_WITH_LABEL: f32 = 18.0;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("invalid SVG: {0}")]
    Svg(#[from] usvg::Error),
    #[error("pixmap allocation failed")]
    Pixmap,
}

/// What one key shows.
#[derive(Debug, Clone)]
pub struct KeyView<'a> {
    /// SVG using `currentColor`.
    pub icon: Option<&'a [u8]>,
    pub label: Option<&'a str>,
    pub fg: Role,
    pub bg: Role,
}

impl Default for KeyView<'_> {
    fn default() -> Self {
        Self {
            icon: None,
            label: None,
            fg: Role::Foreground,
            bg: Role::Background,
        }
    }
}

/// Raw RGB888 image, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbImage {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.width + x) * 3) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }
}

pub struct Renderer {
    fonts: FontSystem,
    glyphs: SwashCache,
    family: String,
}

impl Renderer {
    /// Loads only the given font file(s) to keep memory low; the first
    /// family found is used for labels.
    pub fn new(font_data: Vec<Vec<u8>>) -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        for data in font_data {
            db.load_font_data(data);
        }
        let family = db
            .faces()
            .next()
            .and_then(|f| f.families.first())
            .map(|(name, _)| name.clone())
            .unwrap_or_default();
        Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            glyphs: SwashCache::new(),
            family,
        }
    }

    pub fn key(&mut self, theme: &Theme, view: &KeyView) -> Result<RgbImage, RenderError> {
        let bg = theme.get(view.bg);
        let fg = theme.get(view.fg).readable_on(bg, MIN_CONTRAST);
        let mut pm = Pixmap::new(KEY_SIZE, KEY_SIZE).ok_or(RenderError::Pixmap)?;
        pm.fill(tiny_skia::Color::from_rgba8(bg.0, bg.1, bg.2, 255));

        let label = view.label.filter(|l| !l.is_empty());
        if let Some(svg) = view.icon {
            let (size, top) = match label {
                Some(_) => (ICON_WITH_LABEL, ICON_TOP_WITH_LABEL),
                None => (ICON_ALONE, (KEY_SIZE as f32 - ICON_ALONE) / 2.0),
            };
            draw_svg(&mut pm, svg, &fg.hex(), size, top)?;
        }
        if let Some(text) = label {
            self.draw_label(&mut pm, text, fg);
        }
        Ok(to_rgb(&pm))
    }

    fn draw_label(&mut self, pm: &mut Pixmap, text: &str, fg: crate::theme::Color) {
        let line_h = LABEL_PX * 1.25;
        let width = KEY_SIZE as f32 - 2.0 * LABEL_PAD;
        let top = ICON_TOP_WITH_LABEL + ICON_WITH_LABEL + 10.0;
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(LABEL_PX, line_h));
        buf.set_wrap(Wrap::None);
        buf.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        buf.set_size(Some(width), Some(line_h));
        let attrs = Attrs::new().family(Family::Name(&self.family));
        buf.set_text(text, &attrs, Shaping::Advanced, Some(Align::Center));
        let color = cosmic_text::Color::rgb(fg.0, fg.1, fg.2);
        let mut paint = Paint::default();
        buf.draw(&mut self.fonts, &mut self.glyphs, color, |x, y, w, h, c| {
            if c.a() == 0 {
                return;
            }
            let Some(rect) =
                Rect::from_xywh(LABEL_PAD + x as f32, top + y as f32, w as f32, h as f32)
            else {
                return;
            };
            paint.set_color_rgba8(c.r(), c.g(), c.b(), c.a());
            pm.fill_rect(rect, &paint, Transform::identity(), None);
        });
    }
}

/// Renders `svg` with `currentColor` replaced by `color`, scaled into a
/// `size` square, horizontally centered, top edge at `top`.
fn draw_svg(
    pm: &mut Pixmap,
    svg: &[u8],
    color: &str,
    size: f32,
    top: f32,
) -> Result<(), RenderError> {
    let src = String::from_utf8_lossy(svg).replace("currentColor", color);
    let tree = usvg::Tree::from_str(&src, &usvg::Options::default())?;
    let s = tree.size();
    let scale = size / s.width().max(s.height());
    let left = (KEY_SIZE as f32 - s.width() * scale) / 2.0;
    let top = top + (size - s.height() * scale) / 2.0;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale).post_translate(left, top),
        &mut pm.as_mut(),
    );
    Ok(())
}

/// Pixmap is fully opaque (filled background), so premultiplied == straight.
fn to_rgb(pm: &Pixmap) -> RgbImage {
    let data = pm
        .pixels()
        .iter()
        .flat_map(|p| [p.red(), p.green(), p.blue()])
        .collect();
    RgbImage {
        width: pm.width(),
        height: pm.height(),
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Color;

    const ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v9"/><path d="M6 6a9 9 0 1 0 12 0"/></svg>"#;

    fn theme() -> Theme {
        Theme::parse(
            "accent = \"#e68e0d\"\nbackground = \"#121212\"\nforeground = \"#bebebe\"\nmuted = \"#333333\"",
        )
        .unwrap()
    }

    /// Compact deterministic snapshot: ASCII art, `#` = icon-ish pixel.
    fn ascii(img: &RgbImage, bg: [u8; 3]) -> String {
        let mut out = String::new();
        for y in (0..img.height).step_by(4) {
            for x in (0..img.width).step_by(4) {
                let p = img.pixel(x, y);
                let d: u32 = p
                    .iter()
                    .zip(bg)
                    .map(|(a, b)| u32::from(a.abs_diff(b)))
                    .sum();
                out.push(if d > 150 {
                    '#'
                } else if d > 30 {
                    '+'
                } else {
                    '.'
                });
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn icon_only_key() {
        let mut r = Renderer::new(vec![]);
        let t = theme();
        let view = KeyView {
            icon: Some(ICON),
            fg: Role::Accent,
            ..Default::default()
        };
        let img = r.key(&t, &view).unwrap();
        assert_eq!(img.data.len(), 120 * 120 * 3);
        assert_eq!(img.pixel(0, 0), [0x12, 0x12, 0x12]);
        insta::assert_snapshot!(ascii(&img, [0x12, 0x12, 0x12]));
    }

    #[test]
    fn icon_tinted_with_role() {
        let mut r = Renderer::new(vec![]);
        let t = theme();
        let view = KeyView {
            icon: Some(ICON),
            fg: Role::Accent,
            ..Default::default()
        };
        let img = r.key(&t, &view).unwrap();
        // Top of the vertical stroke (12,3..9 in a 64 px icon at y=28).
        assert_eq!(img.pixel(60, 40), [0xe6, 0x8e, 0x0d]);
    }

    #[test]
    fn low_contrast_role_is_lifted() {
        let mut r = Renderer::new(vec![]);
        let t = theme();
        let view = KeyView {
            icon: Some(ICON),
            fg: Role::Muted,
            ..Default::default()
        };
        let img = r.key(&t, &view).unwrap();
        let [cr, cg, cb] = img.pixel(60, 40);
        assert!(Color(cr, cg, cb).contrast(t.background) >= MIN_CONTRAST);
    }

    #[test]
    fn label_uses_given_font() {
        let Ok(font) = std::fs::read("/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf")
        else {
            return; // font not installed; label layout is covered on dev machines
        };
        let mut r = Renderer::new(vec![font]);
        let t = theme();
        let view = KeyView {
            icon: Some(ICON),
            label: Some("A very long label text"),
            ..Default::default()
        };
        let img = r.key(&t, &view).unwrap();
        let bg = [0x12, 0x12, 0x12];
        let label_px = (88..110)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|&(x, y)| img.pixel(x, y) != bg)
            .count();
        assert!(label_px > 50, "label not drawn");
        // Ellipsized: nothing touches the side edges.
        assert!((88..110).all(|y| img.pixel(2, y) == bg && img.pixel(117, y) == bg));
    }

    #[test]
    fn invalid_svg_is_an_error() {
        let mut r = Renderer::new(vec![]);
        let view = KeyView {
            icon: Some(b"<nope"),
            ..Default::default()
        };
        assert!(r.key(&theme(), &view).is_err());
    }
}
