//! User icons in `<config dir>/icons/<name>.svg|png`. A binding's `icon`
//! falls back to these when no built-in icon has that name. SVGs using
//! `currentColor` are tinted like built-in icons; everything else is drawn
//! as is.

use std::path::{Path, PathBuf};

use resvg::{tiny_skia, usvg};

/// Larger files are rejected on import and ignored on load.
pub const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum IconError {
    #[error("only .svg and .png files are supported")]
    Extension,
    #[error("file is larger than 1 MiB")]
    TooLarge,
    #[error("invalid name {0:?} (use letters, digits, - and _)")]
    Name(String),
    #[error("{0:?} is a built-in icon name")]
    Builtin(String),
    #[error("not a valid image: {0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `<config dir>/icons`.
pub fn dir() -> Option<PathBuf> {
    crate::config::dir().map(|d| d.join("icons"))
}

/// Bytes of user icon `name` in `dir`, if present, small enough and valid.
pub fn load(dir: &Path, name: &str) -> Option<Vec<u8>> {
    if !valid_name(name) {
        return None;
    }
    ["svg", "png"].into_iter().find_map(|ext| {
        let path = dir.join(format!("{name}.{ext}"));
        if std::fs::metadata(&path).ok()?.len() > MAX_BYTES {
            return None;
        }
        let data = std::fs::read(&path).ok()?;
        validate(&data).ok()?;
        Some(data)
    })
}

/// Sorted `(name, bytes)` of every loadable user icon in `dir`.
pub fn all(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| {
            let path = e.ok()?.path();
            matches!(path.extension()?.to_str()?, "svg" | "png")
                .then(|| path.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter_map(|n| load(dir, &n).map(|d| (n, d)))
        .collect()
}

/// Copies `src` into `dir` as `<name>.<ext>` (name defaults to the file
/// stem) after validating it. Replaces an existing icon of that name.
pub fn add(dir: &Path, src: &Path, name: Option<&str>) -> Result<String, IconError> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| e == "svg" || e == "png")
        .ok_or(IconError::Extension)?;
    let name = match name {
        Some(n) => n.to_owned(),
        None => sanitize(&src.file_stem().unwrap_or_default().to_string_lossy()),
    };
    if !valid_name(&name) {
        return Err(IconError::Name(name));
    }
    if crate::icons::get(&name).is_some() {
        return Err(IconError::Builtin(name));
    }
    if std::fs::metadata(src)?.len() > MAX_BYTES {
        return Err(IconError::TooLarge);
    }
    let data = std::fs::read(src)?;
    validate(&data)?;
    std::fs::create_dir_all(dir)?;
    for old in ["svg", "png"] {
        let _ = std::fs::remove_file(dir.join(format!("{name}.{old}")));
    }
    std::fs::write(dir.join(format!("{name}.{ext}")), data)?;
    Ok(name)
}

/// Deletes user icon `name`; returns whether a file was removed.
pub fn remove(dir: &Path, name: &str) -> Result<bool, IconError> {
    if !valid_name(name) {
        return Err(IconError::Name(name.to_owned()));
    }
    let mut removed = false;
    for ext in ["svg", "png"] {
        match std::fs::remove_file(dir.join(format!("{name}.{ext}"))) {
            Ok(()) => removed = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(removed)
}

/// Data URL for previews: SVGs tinted with `color`, PNGs base64-encoded.
pub fn data_url(data: &[u8], color: &str) -> Option<String> {
    if data.starts_with(b"\x89PNG") {
        return Some(format!("data:image/png;base64,{}", base64(data)));
    }
    crate::icons::data_url(data, color)
}

fn validate(data: &[u8]) -> Result<(), IconError> {
    if data.starts_with(b"\x89PNG") {
        tiny_skia::Pixmap::decode_png(data).map_err(|e| IconError::Invalid(e.to_string()))?;
    } else {
        let src = String::from_utf8_lossy(data).replace("currentColor", "#000000");
        usvg::Tree::from_str(&src, &usvg::Options::default())
            .map_err(|e| IconError::Invalid(e.to_string()))?;
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn sanitize(stem: &str) -> String {
    let s: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    s.trim_matches('-').chars().take(64).collect()
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path stroke="currentColor" d="M1 1L20 20"/></svg>"#;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dd-custom-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn add_load_list_remove() {
        let d = tmp("roundtrip");
        let src = d.join("My Logo!.svg");
        std::fs::write(&src, SVG).unwrap();
        let icons = d.join("icons");
        assert_eq!(add(&icons, &src, None).unwrap(), "my-logo");
        assert_eq!(load(&icons, "my-logo").as_deref(), Some(SVG));
        assert_eq!(all(&icons).len(), 1);
        assert!(remove(&icons, "my-logo").unwrap());
        assert!(load(&icons, "my-logo").is_none());
    }

    #[test]
    fn rejects_bad_input() {
        let d = tmp("bad");
        let txt = d.join("a.txt");
        std::fs::write(&txt, "x").unwrap();
        assert!(matches!(add(&d, &txt, None), Err(IconError::Extension)));
        let svg = d.join("b.svg");
        std::fs::write(&svg, "not svg").unwrap();
        assert!(matches!(add(&d, &svg, None), Err(IconError::Invalid(_))));
        std::fs::write(&svg, SVG).unwrap();
        assert!(matches!(
            add(&d, &svg, Some("../x")),
            Err(IconError::Name(_))
        ));
        assert!(matches!(
            add(&d, &svg, Some("lock")),
            Err(IconError::Builtin(_))
        ));
        assert!(load(&d, "../b").is_none());
    }

    #[test]
    fn base64_padding() {
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
    }
}
