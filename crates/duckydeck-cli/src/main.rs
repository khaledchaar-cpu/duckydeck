//! `duckydeck` command line interface: talks to `duckydeckd` over its
//! Unix socket (protocol: `docs/ipc.md`).

use anyhow::{Context, Result, bail};
use duckydeck_core::catalog::Catalog;
use duckydeck_core::ipc::{self, Command, Request, Response, Status};
use duckydeck_core::{check, config, setup};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::OwnedReadHalf;

const USAGE: &str = "\
usage: duckydeck <command> [--json]

commands:
  status                show device, profile, page and brightness
  profile <name>        switch to a profile (until the next config change)
  page <n>              open page n (1-based) of the active profile
  brightness <0-100>    set the brightness (until the next config change)
  check                 validate config and profiles (no daemon needed)
  export <profile>      print a profile as TOML (also the built-in one)
  reload                re-read system font, theme and config
  actions               list every action (id, slot, label) for the editor
  subscribe             print status events as JSON lines until killed
  setup [--remove]      link shell plugins, add menu entry and font hook (or undo)
  version               print the version";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut json = false;
    let mut args = Vec::new();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--json" => json = true,
            "-h" | "--help" | "help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => args.push(a),
        }
    }
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["check"] => return check(),
        ["export", id] => return export(id),
        ["check", ..] | ["export", ..] => {
            eprintln!("duckydeck: wrong arguments for {}\n\n{USAGE}", args[0]);
            std::process::exit(2);
        }
        _ => {}
    }
    if args.first().is_some_and(|a| a == "setup") {
        let remove = match &args[1..] {
            [] => false,
            [flag] if flag == "--remove" => true,
            _ => {
                eprintln!("duckydeck: setup takes only --remove\n\n{USAGE}");
                std::process::exit(2);
            }
        };
        return setup(remove).await;
    }
    let cmd = match parse(&args) {
        Ok(Some(cmd)) => cmd,
        Ok(None) => {
            println!("duckydeck {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Err(e) => {
            eprintln!("duckydeck: {e:#}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(cmd, json).await {
        if json {
            println!(
                "{}",
                serde_json::to_string(&Response::error(format!("{e:#}")))?
            );
        } else {
            eprintln!("duckydeck: {e:#}");
        }
        std::process::exit(1);
    }
    Ok(())
}

/// `Ok(None)` means `version`.
fn parse(args: &[String]) -> Result<Option<Command>> {
    let arg = |name: &str| -> Result<&String> {
        match args {
            [_, v] => Ok(v),
            _ => bail!("{} expects exactly one argument: {name}", args[0]),
        }
    };
    let Some(first) = args.first() else {
        return Ok(Some(Command::Status));
    };
    let cmd = match first.as_str() {
        "version" => return Ok(None),
        "status" if args.len() == 1 => Command::Status,
        "subscribe" if args.len() == 1 => Command::Subscribe,
        "actions" if args.len() == 1 => Command::ListActions,
        "reload" if args.len() == 1 => Command::Reload,
        "profile" => Command::SetProfile {
            profile: arg("<name>")?.clone(),
        },
        "page" => Command::SetPage {
            page: arg("<n>")?.parse().context("page must be a number")?,
        },
        "brightness" => Command::SetBrightness {
            brightness: arg("<0-100>")?
                .parse()
                .context("brightness must be 0-100")?,
        },
        other => bail!("unknown command or arguments: {other}"),
    };
    Ok(Some(cmd))
}

fn check() -> Result<()> {
    let dir = config::dir().context("neither XDG_CONFIG_HOME nor HOME is set")?;
    let loaded = match config::Loaded::load(&dir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let problems = check::problems(&loaded, &Catalog::builtin()?);
    if problems.is_empty() {
        println!(
            "config OK: {} (profiles: {})",
            dir.display(),
            loaded.profiles.len()
        );
        return Ok(());
    }
    for p in &problems {
        eprintln!("{p}");
    }
    std::process::exit(1);
}

/// The profile file as written, so comments survive; the built-in default
/// profile comes from the binary unless a user file overrides it.
fn export(id: &str) -> Result<()> {
    let dir = config::dir().context("neither XDG_CONFIG_HOME nor HOME is set")?;
    let path = dir.join("profiles").join(format!("{id}.toml"));
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && id == config::DEFAULT_PROFILE_ID => {
            config::DEFAULT_PROFILE.to_owned()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!("unknown profile {id:?}"),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    print!("{src}");
    Ok(())
}

async fn setup(remove: bool) -> Result<()> {
    let paths = setup::Paths::detect()?;
    let done = if remove {
        setup::remove(&paths)?
    } else {
        setup::install(&paths, &duckydeck_core::TokioRunner).await?
    };
    if done.is_empty() {
        println!("nothing to do");
    }
    for line in done {
        println!("{line}");
    }
    Ok(())
}

async fn run(cmd: Command, json: bool) -> Result<()> {
    let path = ipc::socket_path().context("XDG_RUNTIME_DIR is not set")?;
    let stream = UnixStream::connect(&path)
        .await
        .with_context(|| format!("daemon not running? cannot connect to {}", path.display()))?;
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let subscribe = cmd == Command::Subscribe;
    let mut req = serde_json::to_vec(&Request::new(cmd))?;
    req.push(b'\n');
    write.write_all(&req).await?;

    let line = next(&mut lines).await?;
    let resp: Response = serde_json::from_str(&line).context("invalid daemon response")?;
    if !resp.ok {
        bail!("{}", resp.error.unwrap_or_else(|| "request failed".into()));
    }
    if !subscribe {
        match (json, &resp.status) {
            (true, _) => println!("{line}"),
            (false, _) if resp.actions.is_some() => print_actions(resp.actions.as_deref()),
            (false, Some(s)) => print_status(s),
            (false, None) => {}
        }
        return Ok(());
    }
    // Always JSON lines: the shell plugin reads them. The first line is the
    // current status so the reader starts with a complete picture.
    println!("{line}");
    loop {
        println!("{}", next(&mut lines).await?);
    }
}

async fn next(lines: &mut Lines<BufReader<OwnedReadHalf>>) -> Result<String> {
    lines
        .next_line()
        .await?
        .context("daemon closed the connection")
}

fn print_actions(items: Option<&[duckydeck_core::library::Item]>) {
    for it in items.unwrap_or_default() {
        let slot = match it.slot {
            duckydeck_core::library::Slot::Key => "key",
            duckydeck_core::library::Slot::Dial => "dial",
        };
        let note = if it.available { "" } else { "  (unavailable)" };
        println!("{:<30} {slot:<5} {}{note}", it.id, it.label);
    }
}

fn print_status(s: &Status) {
    match &s.serial {
        Some(serial) => println!("device:     Stream Deck + ({serial})"),
        None if s.connected => println!("device:     Stream Deck +"),
        None => println!("device:     not connected"),
    }
    println!("profile:    {} ({})", s.profile, s.profiles.join(", "));
    match &s.folder {
        Some(f) => println!("page:       {}/{} – folder {f}", s.page, s.pages),
        None => println!("page:       {}/{}", s.page, s.pages),
    }
    println!("brightness: {} %", s.brightness);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Option<Command>> {
        parse(&args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_commands() -> Result<()> {
        assert_eq!(p(&[])?, Some(Command::Status));
        assert_eq!(p(&["version"])?, None);
        assert_eq!(p(&["actions"])?, Some(Command::ListActions));
        assert_eq!(p(&["reload"])?, Some(Command::Reload));
        assert_eq!(p(&["page", "2"])?, Some(Command::SetPage { page: 2 }));
        assert_eq!(
            p(&["brightness", "40"])?,
            Some(Command::SetBrightness { brightness: 40 })
        );
        assert!(p(&["page"]).is_err());
        assert!(p(&["brightness", "300"]).is_err());
        assert!(p(&["status", "x"]).is_err());
        assert!(p(&["reboot"]).is_err());
        Ok(())
    }
}
