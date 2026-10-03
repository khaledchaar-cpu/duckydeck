//! Free icon libraries the user can download on demand (explicit action
//! only, rule 6) into `$XDG_DATA_HOME/duckydeck/libraries/<id>/`. Searching
//! them is local; a picked icon is copied into the user icons
//! ([`crate::custom_icons`]) as `<id>-<name>`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::command::{CommandError, CommandRunner, CommandSpec};

/// One downloadable library: a pinned npm tarball and the folders with SVGs.
#[derive(Debug)]
pub struct Library {
    pub id: &'static str,
    pub name: &'static str,
    pub license: &'static str,
    pub version: &'static str,
    url: &'static str,
    sha256: &'static str,
    /// Folder inside the tarball and the suffix added to its file names.
    dirs: &'static [(&'static str, &'static str)],
}

pub const LIBRARIES: &[Library] = &[
    Library {
        id: "tabler",
        name: "Tabler Icons",
        license: "MIT",
        version: "3.48.0",
        url: "https://registry.npmjs.org/@tabler/icons/-/icons-3.48.0.tgz",
        sha256: "28447dcf6f0bb2b8d92c59a1b3d30900a180de2e97d7db4d267e6320dc68f449",
        dirs: &[
            ("package/icons/outline", ""),
            ("package/icons/filled", "-filled"),
        ],
    },
    Library {
        id: "lucide",
        name: "Lucide",
        license: "ISC",
        version: "1.51.0",
        url: "https://registry.npmjs.org/lucide-static/-/lucide-static-1.51.0.tgz",
        sha256: "6936026d1512256483161ae999982e3be3d5d74226996c4ec7447e901726a2d8",
        dirs: &[("package/icons", "")],
    },
];

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);
const VERSION_FILE: &str = ".version";

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("unknown icon library {0:?}")]
    Unknown(String),
    #[error("{0}")]
    Command(#[from] CommandError),
    #[error("`{0}` failed: {1}")]
    Failed(&'static str, String),
    #[error("checksum mismatch – download rejected")]
    Checksum,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `$XDG_DATA_HOME/duckydeck/libraries`, falling back to `~/.local/share`.
pub fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("duckydeck/libraries"))
}

pub fn get(id: &str) -> Result<&'static Library, LibraryError> {
    LIBRARIES
        .iter()
        .find(|l| l.id == id)
        .ok_or_else(|| LibraryError::Unknown(id.to_owned()))
}

/// Whether the pinned version of `lib` is installed in `base`.
pub fn installed(base: &Path, lib: &Library) -> bool {
    std::fs::read_to_string(base.join(lib.id).join(VERSION_FILE))
        .is_ok_and(|v| v.trim() == lib.version)
}

/// Downloads, verifies and unpacks `lib` into `base/<id>`, replacing an
/// older version. Returns the number of icons.
pub async fn install(
    runner: &dyn CommandRunner,
    base: &Path,
    lib: &Library,
) -> Result<usize, LibraryError> {
    let tmp = base.join(format!(".{}.tmp", lib.id));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let result = fetch_and_unpack(runner, base, lib, &tmp).await;
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

async fn fetch_and_unpack(
    runner: &dyn CommandRunner,
    base: &Path,
    lib: &Library,
    tmp: &Path,
) -> Result<usize, LibraryError> {
    let tgz = tmp.join("package.tgz").to_string_lossy().into_owned();
    let curl = CommandSpec::new("curl")
        .args(["-fsSL", "--proto", "=https", "-o", &tgz, lib.url])
        .timeout(DOWNLOAD_TIMEOUT);
    check("curl", runner.run(&curl).await?)?;
    let sum = check(
        "sha256sum",
        runner.run(&CommandSpec::new("sha256sum").arg(&tgz)).await?,
    )?;
    if sum.split_whitespace().next() != Some(lib.sha256) {
        return Err(LibraryError::Checksum);
    }
    let tar = CommandSpec::new("tar")
        .args(["-xzf", &tgz, "-C", &tmp.to_string_lossy()])
        .args(lib.dirs.iter().map(|(d, _)| *d))
        .timeout(Duration::from_secs(60));
    check("tar", runner.run(&tar).await?)?;

    let new = tmp.join("out");
    std::fs::create_dir_all(&new)?;
    let mut count = 0;
    for (sub, suffix) in lib.dirs {
        for entry in std::fs::read_dir(tmp.join(sub))? {
            let path = entry?.path();
            let Some(stem) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|_| path.extension().is_some_and(|e| e == "svg"))
            else {
                continue;
            };
            let stem = stem.strip_suffix(suffix).unwrap_or(stem);
            std::fs::rename(&path, new.join(format!("{stem}{suffix}.svg")))?;
            count += 1;
        }
    }
    std::fs::write(new.join(VERSION_FILE), lib.version)?;
    let target = base.join(lib.id);
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&new, &target)?;
    Ok(count)
}

fn check(
    program: &'static str,
    out: crate::command::CommandOutput,
) -> Result<String, LibraryError> {
    if out.success() {
        Ok(out.stdout)
    } else {
        Err(LibraryError::Failed(program, out.stderr.trim().to_owned()))
    }
}

/// Deletes library `lib`; returns whether it was installed.
pub fn remove(base: &Path, lib: &Library) -> Result<bool, LibraryError> {
    match std::fs::remove_dir_all(base.join(lib.id)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// One search hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub library: &'static str,
    pub name: String,
    pub path: PathBuf,
}

/// Icons of all installed libraries whose name contains every word of
/// `query`; exact and prefix matches first, at most `limit`.
pub fn search(base: &Path, query: &str, limit: usize) -> Vec<Hit> {
    let words: Vec<String> = query
        .split(|c: char| c.is_whitespace() || c == '-')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let q = words.join("-");
    let mut hits: Vec<(u8, Hit)> = Vec::new();
    for lib in LIBRARIES.iter().filter(|l| installed(base, l)) {
        let Ok(entries) = std::fs::read_dir(base.join(lib.id)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|_| path.extension().is_some_and(|e| e == "svg"))
            else {
                continue;
            };
            if !words.iter().all(|w| name.contains(w.as_str())) {
                continue;
            }
            let rank = if name == q {
                0
            } else if name.starts_with(&q) {
                1
            } else {
                2
            };
            hits.push((
                rank,
                Hit {
                    library: lib.id,
                    name: name.to_owned(),
                    path,
                },
            ));
        }
    }
    hits.sort_by(|(ra, a), (rb, b)| {
        (ra, a.name.len(), &a.name, a.library).cmp(&(rb, b.name.len(), &b.name, b.library))
    });
    hits.into_iter().take(limit).map(|(_, h)| h).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutput, RecordingRunner};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dd-lib-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn fake_install(base: &Path, id: &str, version: &str, names: &[&str]) {
        let d = base.join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(VERSION_FILE), version).unwrap();
        for n in names {
            std::fs::write(d.join(format!("{n}.svg")), "<svg/>").unwrap();
        }
    }

    #[test]
    fn search_ranks_exact_and_prefix_first() {
        let base = tmp("search");
        fake_install(&base, "tabler", "3.48.0", &["home-2", "smart-home", "home"]);
        fake_install(&base, "lucide", "1.51.0", &["house", "home-plus"]);
        let names: Vec<_> = search(&base, "home", 10)
            .into_iter()
            .map(|h| format!("{}:{}", h.library, h.name))
            .collect();
        assert_eq!(
            names,
            [
                "tabler:home",
                "tabler:home-2",
                "lucide:home-plus",
                "tabler:smart-home"
            ]
        );
        assert_eq!(search(&base, "smart home", 10).len(), 1);
        assert!(search(&base, " ", 10).is_empty());
    }

    #[test]
    fn outdated_library_is_not_installed() {
        let base = tmp("outdated");
        fake_install(&base, "lucide", "0.1.0", &["house"]);
        assert!(!installed(&base, get("lucide").unwrap()));
        assert!(search(&base, "house", 10).is_empty());
        assert!(remove(&base, get("lucide").unwrap()).unwrap());
        assert!(!remove(&base, get("lucide").unwrap()).unwrap());
    }

    #[tokio::test]
    async fn bad_checksum_aborts_and_cleans_up() {
        let base = tmp("checksum");
        let runner = RecordingRunner::with_response(CommandOutput {
            status: Some(0),
            stdout: "deadbeef  package.tgz\n".into(),
            stderr: String::new(),
        });
        let lib = get("lucide").unwrap();
        let err = install(&runner, &base, lib).await.unwrap_err();
        assert!(matches!(err, LibraryError::Checksum));
        let lines = runner.command_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("curl -fsSL --proto =https -o "));
        assert!(lines[0].ends_with(lib.url));
        assert!(!base.join(".lucide.tmp").exists());
        assert!(!installed(&base, lib));
    }

    #[test]
    fn unknown_library() {
        assert!(matches!(get("x"), Err(LibraryError::Unknown(_))));
    }
}
