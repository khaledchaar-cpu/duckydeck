//! `duckydeck setup [--remove]`: links the shell plugins, adds the menu
//! entry and installs the font hook (see `docs/spec/architecture.md`,
//! "Plug & Play"). Idempotent; only touches files below
//! `~/.config/omarchy/` that belong to DuckyDeck.

use std::io;
use std::path::{Path, PathBuf};

use crate::command::{CommandRunner, CommandSpec};

const MENU_BEGIN: &str = "  // duckydeck:begin (managed by `duckydeck setup`)";
const MENU_END: &str = "  // duckydeck:end";
const MENU_BLOCK: &str = include_str!("../../../packaging/omarchy/menu.jsonc");
const FONT_HOOK: &str = include_str!("../../../packaging/omarchy/duckydeck-font-set");
const HOOK_NAME: &str = "duckydeck";
const PLUGIN_PREFIX: &str = "duckydeck.";
const WIDGET: &str = "duckydeck.widget";

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("HOME is not set")]
    NoHome,
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("`{command}` failed: {message}")]
    Command { command: String, message: String },
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> SetupError + '_ {
    move |source| SetupError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Where setup reads from and writes to.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Folder with the shipped `duckydeck.*` shell plugins.
    pub plugins_src: PathBuf,
    /// `~/.config/omarchy`.
    pub omarchy: PathBuf,
    /// Scratch folder for the hook file handed to `omarchy hook install`.
    pub scratch: PathBuf,
    /// Marker file recording which version was set up.
    pub marker: PathBuf,
}

impl Paths {
    /// Shipped plugins come from `$DUCKYDECK_SHARE_DIR`, else
    /// `/usr/share/duckydeck`, else (debug builds) this repository.
    pub fn detect() -> Result<Self, SetupError> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .ok_or(SetupError::NoHome)?;
        let env_dir = |var: &str, fallback: &str| {
            std::env::var_os(var)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(fallback))
        };
        let plugins_src = match std::env::var_os("DUCKYDECK_SHARE_DIR").filter(|s| !s.is_empty()) {
            Some(dir) => PathBuf::from(dir).join("shell-plugins"),
            None if cfg!(debug_assertions) && !Path::new("/usr/share/duckydeck").exists() => {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shell-plugins")
            }
            None => PathBuf::from("/usr/share/duckydeck/shell-plugins"),
        };
        let scratch = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|s| !s.is_empty())
            .map(|d| PathBuf::from(d).join("duckydeck/setup"))
            .unwrap_or_else(|| std::env::temp_dir().join("duckydeck-setup"));
        Ok(Self {
            plugins_src,
            omarchy: env_dir("XDG_CONFIG_HOME", ".config").join("omarchy"),
            scratch,
            marker: env_dir("XDG_STATE_HOME", ".local/state").join("duckydeck/setup"),
        })
    }

    fn menu(&self) -> PathBuf {
        self.omarchy.join("extensions/omarchy-menu.jsonc")
    }

    fn plugins(&self) -> PathBuf {
        self.omarchy.join("plugins")
    }

    fn hook(&self) -> PathBuf {
        self.omarchy.join("hooks/font-set.d").join(HOOK_NAME)
    }
}

/// Whether the daemon should run setup on start: the marker is missing or
/// from another version. `--remove` writes a marker too, so a removal sticks.
pub fn needed(paths: &Paths) -> bool {
    std::fs::read_to_string(&paths.marker).map_or(true, |m| {
        let m = m.trim();
        m != env!("CARGO_PKG_VERSION") && m != "removed"
    })
}

/// Installs everything; returns one line per change for the user.
pub async fn install(paths: &Paths, runner: &dyn CommandRunner) -> Result<Vec<String>, SetupError> {
    let mut done = Vec::new();
    let linked = link_plugins(paths)?;
    if linked.iter().any(|id| id == WIDGET) {
        omarchy(runner, ["bar", "put", WIDGET]).await?;
        done.push(format!("added {WIDGET} to the bar"));
    }
    done.extend(linked.into_iter().map(|id| format!("linked plugin {id}")));

    let menu = paths.menu();
    let old = read_optional(&menu)?;
    let new = with_menu_block(old.as_deref().unwrap_or(""));
    if old.as_deref() != Some(new.as_str()) {
        write(&menu, &new)?;
        done.push(format!("added the menu entry to {}", menu.display()));
    }

    if read_optional(&paths.hook())?.as_deref() != Some(FONT_HOOK) {
        std::fs::create_dir_all(&paths.scratch).map_err(io_err(&paths.scratch))?;
        let file = paths.scratch.join(HOOK_NAME);
        write(&file, FONT_HOOK)?;
        let file_arg = file.to_string_lossy().into_owned();
        omarchy(runner, ["hook", "install", "font-set", file_arg.as_str()]).await?;
        let _ = std::fs::remove_file(&file);
        done.push("installed the font-set hook".into());
    }
    write(&paths.marker, env!("CARGO_PKG_VERSION"))?;
    Ok(done)
}

/// Undoes [`install`]; returns one line per change.
pub fn remove(paths: &Paths) -> Result<Vec<String>, SetupError> {
    let mut done = Vec::new();
    let plugins = paths.plugins();
    if let Ok(entries) = std::fs::read_dir(&plugins) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_link = entry.file_type().is_ok_and(|t| t.is_symlink());
            if name.starts_with(PLUGIN_PREFIX) && is_link {
                std::fs::remove_file(entry.path()).map_err(io_err(&entry.path()))?;
                done.push(format!("unlinked plugin {name}"));
            }
        }
    }
    let menu = paths.menu();
    if let Some(old) = read_optional(&menu)? {
        let new = without_menu_block(&old);
        if new != old {
            write(&menu, &new)?;
            done.push(format!("removed the menu entry from {}", menu.display()));
        }
    }
    let hook = paths.hook();
    if hook.exists() {
        std::fs::remove_file(&hook).map_err(io_err(&hook))?;
        done.push("removed the font-set hook".into());
    }
    write(&paths.marker, "removed")?;
    Ok(done)
}

/// Links every shipped `duckydeck.*` plugin; returns the newly linked ids.
/// A real folder of the same name (user copy) is left alone.
fn link_plugins(paths: &Paths) -> Result<Vec<String>, SetupError> {
    let Ok(entries) = std::fs::read_dir(&paths.plugins_src) else {
        return Ok(Vec::new());
    };
    let dest_dir = paths.plugins();
    let mut linked = Vec::new();
    let mut sources: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("manifest.json").is_file())
        .collect();
    sources.sort();
    for src in sources {
        let Some(id) = src.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if !id.starts_with(PLUGIN_PREFIX) {
            continue;
        }
        let src = src.canonicalize().map_err(io_err(&src))?;
        let dest = dest_dir.join(&id);
        match std::fs::read_link(&dest) {
            Ok(target) if target == src => continue,
            Ok(_) => std::fs::remove_file(&dest).map_err(io_err(&dest))?,
            Err(_) if dest.exists() => {
                tracing::warn!(path = %dest.display(), "not a symlink, leaving it alone");
                continue;
            }
            Err(_) => {}
        }
        std::fs::create_dir_all(&dest_dir).map_err(io_err(&dest_dir))?;
        std::os::unix::fs::symlink(&src, &dest).map_err(io_err(&dest))?;
        linked.push(id);
    }
    Ok(linked)
}

/// Replaces (or appends) the marked block before the closing brace.
fn with_menu_block(text: &str) -> String {
    let text = without_menu_block(text);
    let block = format!("{MENU_BEGIN}\n{}{MENU_END}\n", MENU_BLOCK);
    let Some(close) = text.rfind('}') else {
        return format!("{{\n{block}}}\n");
    };
    let (head, tail) = text.split_at(close);
    let mut head = head.trim_end_matches([' ', '\t']).to_owned();
    if !head.ends_with('\n') {
        head.push('\n');
    }
    // The last entry before our block needs a separating comma.
    let last = head
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty() && !l.starts_with("//"));
    if let Some(last) = last
        && !last.ends_with(',')
        && !last.ends_with('{')
    {
        let at = head.rfind(last).map_or(head.len(), |i| i + last.len());
        head.insert(at, ',');
    }
    format!("{head}{block}{tail}")
}

fn without_menu_block(text: &str) -> String {
    let (Some(start), Some(end)) = (text.find(MENU_BEGIN), text.find(MENU_END)) else {
        return text.to_owned();
    };
    if end < start {
        return text.to_owned();
    }
    let end = end + MENU_END.len();
    let end = end + usize::from(text[end..].starts_with('\n'));
    format!("{}{}", &text[..start], &text[end..])
}

async fn omarchy<const N: usize>(
    runner: &dyn CommandRunner,
    args: [&str; N],
) -> Result<(), SetupError> {
    let spec = CommandSpec::omarchy(args);
    let command = format!("omarchy {}", spec.args.join(" "));
    match runner.run(&spec).await {
        Ok(out) if out.success() => Ok(()),
        Ok(out) => Err(SetupError::Command {
            command,
            message: out.stderr.trim().to_owned(),
        }),
        Err(e) => Err(SetupError::Command {
            command,
            message: e.to_string(),
        }),
    }
}

fn read_optional(path: &Path) -> Result<Option<String>, SetupError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_err(path)(e)),
    }
}

fn write(path: &Path, text: &str) -> Result<(), SetupError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    std::fs::write(path, text).map_err(io_err(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RecordingRunner;

    fn paths(name: &str) -> Paths {
        let root =
            std::env::temp_dir().join(format!("duckydeck-setup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Paths {
            plugins_src: root.join("share/shell-plugins"),
            omarchy: root.join("omarchy"),
            scratch: root.join("scratch"),
            marker: root.join("state/setup"),
        }
    }

    #[test]
    fn menu_block_is_idempotent_and_removable() {
        let user = "{\n  // comment\n  \"a\": {\"label\":\"A\"}\n}\n";
        let once = with_menu_block(user);
        assert!(once.contains("\"a\": {\"label\":\"A\"},\n"));
        assert!(once.contains(MENU_BEGIN) && once.ends_with("  // duckydeck:end\n}\n"));
        assert_eq!(with_menu_block(&once), once);
        assert_eq!(
            without_menu_block(&once),
            "{\n  // comment\n  \"a\": {\"label\":\"A\"},\n}\n"
        );
        assert!(with_menu_block("").starts_with("{\n  // duckydeck:begin"));
        let empty = with_menu_block("{\n  // only comments\n}\n");
        assert!(empty.contains("comments\n  // duckydeck:begin"));
    }

    #[tokio::test]
    async fn install_and_remove() -> anyhow::Result<()> {
        let p = paths("roundtrip");
        for id in [WIDGET, "duckydeck.panel", "spike"] {
            let dir = p.plugins_src.join(id);
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join("manifest.json"), "{}")?;
        }
        assert!(needed(&p));
        let runner = RecordingRunner::new();
        let done = install(&p, &runner).await?;
        assert_eq!(done.len(), 5, "{done:?}");
        let lines = runner.command_lines();
        assert_eq!(lines[0], "omarchy bar put duckydeck.widget");
        assert!(lines[1].starts_with("omarchy hook install font-set "));
        assert!(p.plugins().join("duckydeck.panel").is_symlink());
        assert!(!p.plugins().join("spike").exists());
        assert!(!needed(&p));

        // Second run: only the hook (the recording runner never copies it).
        let runner = RecordingRunner::new();
        let again = install(&p, &runner).await?;
        assert_eq!(again, ["installed the font-set hook"]);

        std::fs::create_dir_all(p.hook().parent().unwrap_or(&p.omarchy))?;
        std::fs::write(p.hook(), FONT_HOOK)?;
        let done = remove(&p)?;
        assert_eq!(done.len(), 4, "{done:?}");
        assert!(!p.plugins().join(WIDGET).exists());
        assert!(!std::fs::read_to_string(p.menu())?.contains("duckydeck"));
        assert!(!needed(&p));
        Ok(())
    }
}
