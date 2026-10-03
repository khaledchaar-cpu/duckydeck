//! Key and strip renderer: monochrome SVG icons tinted with a theme token,
//! one-line text, level bars, on a theme background. Output is raw RGB888.

use cosmic_text::{
    Align, Attrs, Buffer, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping,
    SwashCache, Wrap,
};
use resvg::tiny_skia::{self, Paint, Pixmap, Rect, Transform};
use resvg::usvg;

use crate::theme::{Color, MIN_CONTRAST, Role, Theme};

pub const KEY_SIZE: u32 = 120;

const ICON_WITH_LABEL: f32 = 56.0;
const ICON_ALONE: f32 = 64.0;
const LABEL_PX: f32 = 13.0;
const LABEL_PAD: f32 = 6.0;
/// Icon area top edge when a label is shown; label sits below it.
const ICON_TOP_WITH_LABEL: f32 = 18.0;

/// One touchstrip segment (the strip is 800×100, one segment per encoder).
pub const SEGMENT_W: u32 = 200;
pub const SEGMENT_H: u32 = 100;
const SEG_PAD: f32 = 16.0;
const SEG_ICON: f32 = 36.0;
const SEG_ICON_TOP: f32 = 16.0;
const SEG_TEXT_PX: f32 = 22.0;
const SEG_BAR_TOP: f32 = 68.0;
const SEG_BAR_H: f32 = 10.0;

/// The whole strip, used for the media view.
pub const STRIP_W: u32 = 800;
const MEDIA_ICON: f32 = 40.0;
const MEDIA_TITLE_PX: f32 = 24.0;
const MEDIA_ARTIST_PX: f32 = 18.0;
const MEDIA_TIME_W: f32 = 140.0;
const MEDIA_BAR_TOP: f32 = 80.0;
const MEDIA_BAR_H: f32 = 6.0;

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

/// What one strip segment shows: icon top left, text top right, level bar
/// below (track in `muted`, fill in `fg`).
#[derive(Debug, Clone)]
pub struct SegmentView<'a> {
    /// SVG using `currentColor`.
    pub icon: Option<&'a [u8]>,
    pub text: Option<&'a str>,
    /// 0.0–1.0, clamped; `None` hides the bar.
    pub level: Option<f32>,
    pub fg: Role,
    pub bg: Role,
}

impl Default for SegmentView<'_> {
    fn default() -> Self {
        Self {
            icon: None,
            text: None,
            level: None,
            fg: Role::Foreground,
            bg: Role::Background,
        }
    }
}

/// What the whole strip shows during playback: status icon, title and time
/// on the first line, artist below, progress bar at the bottom.
#[derive(Debug, Clone, Default)]
pub struct MediaView<'a> {
    pub icon: Option<&'a [u8]>,
    pub title: &'a str,
    pub artist: Option<&'a str>,
    pub time: Option<&'a str>,
    /// 0.0–1.0, clamped; `None` hides the bar.
    pub progress: Option<f32>,
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
        let mut pm = canvas(KEY_SIZE, KEY_SIZE, bg)?;

        let label = view.label.filter(|l| !l.is_empty());
        if let Some(svg) = view.icon {
            let (size, top) = match label {
                Some(_) => (ICON_WITH_LABEL, ICON_TOP_WITH_LABEL),
                None => (ICON_ALONE, (KEY_SIZE as f32 - ICON_ALONE) / 2.0),
            };
            let left = (KEY_SIZE as f32 - size) / 2.0;
            draw_svg(&mut pm, svg, &fg.hex(), size, left, top)?;
        }
        if let Some(text) = label {
            let top = ICON_TOP_WITH_LABEL + ICON_WITH_LABEL + 10.0;
            let width = KEY_SIZE as f32 - 2.0 * LABEL_PAD;
            let text_box = TextBox {
                left: LABEL_PAD,
                top,
                width,
                px: LABEL_PX,
                align: Align::Center,
            };
            self.draw_text(&mut pm, text, &text_box, fg);
        }
        Ok(to_rgb(&pm))
    }

    pub fn segment(&mut self, theme: &Theme, view: &SegmentView) -> Result<RgbImage, RenderError> {
        let bg = theme.get(view.bg);
        let fg = theme.get(view.fg).readable_on(bg, MIN_CONTRAST);
        let mut pm = canvas(SEGMENT_W, SEGMENT_H, bg)?;
        let inner = SEGMENT_W as f32 - 2.0 * SEG_PAD;
        if let Some(svg) = view.icon {
            draw_svg(&mut pm, svg, &fg.hex(), SEG_ICON, SEG_PAD, SEG_ICON_TOP)?;
        }
        if let Some(text) = view.text.filter(|t| !t.is_empty()) {
            let left = SEG_PAD + SEG_ICON + 8.0;
            let text_box = TextBox {
                left,
                top: SEG_ICON_TOP + (SEG_ICON - SEG_TEXT_PX * 1.25) / 2.0,
                width: SEG_PAD + inner - left,
                px: SEG_TEXT_PX,
                align: Align::Right,
            };
            self.draw_text(&mut pm, text, &text_box, fg);
        }
        if let Some(level) = view.level {
            let track = theme.get(Role::Muted);
            fill_rect(&mut pm, SEG_PAD, SEG_BAR_TOP, inner, SEG_BAR_H, track);
            let fill = (inner * level.clamp(0.0, 1.0)).round();
            fill_rect(&mut pm, SEG_PAD, SEG_BAR_TOP, fill, SEG_BAR_H, fg);
        }
        Ok(to_rgb(&pm))
    }

    pub fn media(&mut self, theme: &Theme, view: &MediaView) -> Result<RgbImage, RenderError> {
        let bg = theme.get(Role::Background);
        let fg = theme.get(Role::Foreground).readable_on(bg, MIN_CONTRAST);
        let accent = theme.get(Role::Accent).readable_on(bg, MIN_CONTRAST);
        let mut pm = canvas(STRIP_W, SEGMENT_H, bg)?;
        let right = STRIP_W as f32 - SEG_PAD;
        if let Some(svg) = view.icon {
            draw_svg(&mut pm, svg, &accent.hex(), MEDIA_ICON, SEG_PAD, 16.0)?;
        }
        let left = SEG_PAD + MEDIA_ICON + 16.0;
        let time_left = right - MEDIA_TIME_W;
        let title_right = if view.time.is_some() {
            time_left - 16.0
        } else {
            right
        };
        let title = TextBox {
            left,
            top: 10.0,
            width: title_right - left,
            px: MEDIA_TITLE_PX,
            align: Align::Left,
        };
        if !view.title.is_empty() {
            self.draw_text(&mut pm, view.title, &title, fg);
        }
        if let Some(time) = view.time {
            let b = TextBox {
                left: time_left,
                top: 13.0,
                width: MEDIA_TIME_W,
                px: MEDIA_ARTIST_PX,
                align: Align::Right,
            };
            self.draw_text(&mut pm, time, &b, fg);
        }
        if let Some(artist) = view.artist.filter(|a| !a.is_empty()) {
            let b = TextBox {
                left,
                top: 44.0,
                width: right - left,
                px: MEDIA_ARTIST_PX,
                align: Align::Left,
            };
            self.draw_text(&mut pm, artist, &b, accent);
        }
        if let Some(p) = view.progress {
            let w = right - SEG_PAD;
            fill_rect(
                &mut pm,
                SEG_PAD,
                MEDIA_BAR_TOP,
                w,
                MEDIA_BAR_H,
                theme.get(Role::Muted),
            );
            let fill = (w * p.clamp(0.0, 1.0)).round();
            fill_rect(&mut pm, SEG_PAD, MEDIA_BAR_TOP, fill, MEDIA_BAR_H, accent);
        }
        Ok(to_rgb(&pm))
    }

    /// One line, ellipsized to `b.width`.
    fn draw_text(&mut self, pm: &mut Pixmap, text: &str, b: &TextBox, fg: Color) {
        let line_h = b.px * 1.25;
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(b.px, line_h));
        buf.set_wrap(Wrap::None);
        buf.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        buf.set_size(Some(b.width), Some(line_h));
        let attrs = Attrs::new().family(Family::Name(&self.family));
        buf.set_text(text, &attrs, Shaping::Advanced, Some(b.align));
        let color = cosmic_text::Color::rgb(fg.0, fg.1, fg.2);
        let mut paint = Paint::default();
        buf.draw(&mut self.fonts, &mut self.glyphs, color, |x, y, w, h, c| {
            if c.a() == 0 {
                return;
            }
            let Some(rect) =
                Rect::from_xywh(b.left + x as f32, b.top + y as f32, w as f32, h as f32)
            else {
                return;
            };
            paint.set_color_rgba8(c.r(), c.g(), c.b(), c.a());
            pm.fill_rect(rect, &paint, Transform::identity(), None);
        });
    }
}

struct TextBox {
    left: f32,
    top: f32,
    width: f32,
    px: f32,
    align: Align,
}

fn canvas(w: u32, h: u32, bg: Color) -> Result<Pixmap, RenderError> {
    let mut pm = Pixmap::new(w, h).ok_or(RenderError::Pixmap)?;
    pm.fill(tiny_skia::Color::from_rgba8(bg.0, bg.1, bg.2, 255));
    Ok(pm)
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: Color) {
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(c.0, c.1, c.2, 255);
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

/// Renders `svg` with `currentColor` replaced by `color`, centered in the
/// `size` square at (`left`, `top`).
fn draw_svg(
    pm: &mut Pixmap,
    svg: &[u8],
    color: &str,
    size: f32,
    left: f32,
    top: f32,
) -> Result<(), RenderError> {
    let src = String::from_utf8_lossy(svg).replace("currentColor", color);
    let tree = usvg::Tree::from_str(&src, &usvg::Options::default())?;
    let s = tree.size();
    let scale = size / s.width().max(s.height());
    let left = left + (size - s.width() * scale) / 2.0;
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
    fn media_strip() {
        let mut r = Renderer::new(vec![]);
        let view = MediaView {
            icon: Some(ICON),
            // No font in tests: text is not drawn.
            progress: Some(0.25),
            ..Default::default()
        };
        let img = r.media(&theme(), &view).unwrap();
        assert_eq!((img.width, img.height), (800, 100));
        // Bar: filled in accent up to a quarter, track after it.
        assert_eq!(img.pixel(100, 82), [0xe6, 0x8e, 0x0d]);
        assert_eq!(img.pixel(600, 82), [0x33, 0x33, 0x33]);
        insta::assert_snapshot!(ascii(&img, [0x12, 0x12, 0x12]));
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
    fn segment_with_icon_and_bar() {
        let mut r = Renderer::new(vec![]);
        let view = SegmentView {
            icon: Some(ICON),
            level: Some(0.5),
            fg: Role::Accent,
            ..Default::default()
        };
        let img = r.segment(&theme(), &view).unwrap();
        assert_eq!((img.width, img.height), (200, 100));
        insta::assert_snapshot!(ascii(&img, [0x12, 0x12, 0x12]));
    }

    #[test]
    fn segment_bar_fill_and_track() {
        let mut r = Renderer::new(vec![]);
        let view = SegmentView {
            level: Some(0.25),
            fg: Role::Accent,
            ..Default::default()
        };
        let img = r.segment(&theme(), &view).unwrap();
        let y = 72;
        assert_eq!(img.pixel(16, y), [0xe6, 0x8e, 0x0d]);
        assert_eq!(img.pixel(57, y), [0xe6, 0x8e, 0x0d]);
        assert_eq!(img.pixel(58, y), [0x33, 0x33, 0x33]);
        assert_eq!(img.pixel(183, y), [0x33, 0x33, 0x33]);
        assert_eq!(img.pixel(184, y), [0x12, 0x12, 0x12]);
        // Out-of-range levels are clamped.
        let full = SegmentView {
            level: Some(7.0),
            ..view
        };
        let img = r.segment(&theme(), &full).unwrap();
        assert_ne!(img.pixel(183, y), [0x33, 0x33, 0x33]);
    }

    #[test]
    fn segment_text_is_right_aligned() {
        let Ok(font) = std::fs::read("/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf")
        else {
            return;
        };
        let mut r = Renderer::new(vec![font]);
        let view = SegmentView {
            text: Some("42%"),
            ..Default::default()
        };
        let img = r.segment(&theme(), &view).unwrap();
        let bg = [0x12, 0x12, 0x12];
        let cols: Vec<u32> = (0..200)
            .filter(|&x| (16..52).any(|y| img.pixel(x, y) != bg))
            .collect();
        assert!(
            cols.first().is_some_and(|&x| x > 100),
            "text not right-aligned"
        );
        assert!(
            cols.last().is_some_and(|&x| x < 184),
            "text overflows padding"
        );
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
