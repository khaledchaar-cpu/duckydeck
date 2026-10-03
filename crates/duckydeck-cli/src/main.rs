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
  export <profile>      print a profile as TOML (also the built-in one); with
                        --json: parsed, 8 keys and 4 dials per page (empty = null)
  edit <profile> set <page|folder> key|dial <n> <json>
  edit <profile> clear <page|folder> key|dial <n>
  edit <profile> swap <page|folder> key|dial <n> <page|folder> <n>
                        change one slot in the profile file (no daemon needed;
                        <json> = {action, args?, label?, icon?} as JSON)
  edit <profile> page add | page remove <n> | page move <from> <to>
  edit <profile> name <text> | match <class regex> | title <title regex>
                        (empty regex = off; class and title must both match)
  edit <profile> create <name> [<copy of>] | delete
                        manage pages and profiles; deleting the default
                        profile resets it to the built-in one
  reload                re-read system font, theme and config
  actions               list every action (id, slot, label) for the editor
  profiles              list profile ids and names (no daemon needed)
  icons                 list the built-in icon names (no daemon needed)
  apps                  list installed apps: desktop-entry id and name (no daemon needed)
  preview <profile> [<page>|<folder>]
                        render a page or folder to PNG files (paths printed)
  subscribe             print status events as JSON lines until killed
  learn                 like subscribe, plus slot_pressed events; the deck runs
                        no actions until this command ends (editor learn mode)
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
        ["icons"] => {
            let names: Vec<&str> = duckydeck_core::icons::names().collect();
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "v": 1, "ok": true, "icons": names })
                );
            } else {
                println!("{}", names.join("\n"));
            }
            return Ok(());
        }
        ["profiles"] => {
            let dir = config::dir().context("neither XDG_CONFIG_HOME nor HOME is set")?;
            let loaded = config::Loaded::load(&dir)?;
            if json {
                let list: Vec<_> = loaded
                    .profiles
                    .iter()
                    .map(|(id, p)| serde_json::json!({ "id": id, "name": p.name }))
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({ "v": 1, "ok": true, "profiles": list })
                );
            } else {
                for (id, p) in &loaded.profiles {
                    println!("{id}\t{}", p.name);
                }
            }
            return Ok(());
        }
        ["apps"] => {
            let apps = duckydeck_core::apps::scan(&duckydeck_core::apps::dirs());
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "v": 1, "ok": true, "apps": apps })
                );
            } else {
                for a in apps {
                    println!("{}\t{}", a.id, a.name);
                }
            }
            return Ok(());
        }
        ["export", id] if json => return export_json(id),
        ["export", id] => return export(id),
        ["edit", ref rest @ ..] => {
            if let Err(e) = edit(rest) {
                eprintln!("duckydeck: {e:#}");
                std::process::exit(if e.is::<Usage>() { 2 } else { 1 });
            }
            return Ok(());
        }
        ["check", ..] | ["export", ..] | ["icons", ..] | ["profiles", ..] | ["apps", ..] => {
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
        "learn" if args.len() == 1 => Command::Learn,
        "preview" if (2..=3).contains(&args.len()) => {
            let (page, folder) = match args.get(2) {
                None => (None, None),
                Some(a) => match a.parse::<usize>() {
                    Ok(n) => (Some(n), None),
                    Err(_) => (None, Some(a.clone())),
                },
            };
            Command::Preview {
                profile: args[1].clone(),
                page,
                folder,
            }
        }
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

/// Wrong arguments for `edit` (exit code 2).
#[derive(Debug)]
struct Usage(String);

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Usage {}

fn edit(args: &[&str]) -> Result<()> {
    use duckydeck_core::edit::{self, Kind, Location, SlotRef};

    let usage = || anyhow::Error::new(Usage(format!("wrong arguments for edit\n\n{USAGE}")));
    let at = |s: &str| match s.parse::<usize>() {
        Ok(n) => Location::Page(n),
        Err(_) => Location::Folder(s.to_owned()),
    };
    let kind = |s: &str| match s {
        "key" => Ok(Kind::Key),
        "dial" => Ok(Kind::Dial),
        _ => Err(usage()),
    };
    let index = |s: &str| s.parse::<usize>().map_err(|_| usage());
    let dir = config::dir().context("neither XDG_CONFIG_HOME nor HOME is set")?;
    let catalog = Catalog::builtin()?;
    match args {
        [id, "set", loc, k, n, json] => {
            let slot = SlotRef {
                at: at(loc),
                kind: kind(k)?,
                index: index(n)?,
            };
            let binding: serde_json::Value =
                serde_json::from_str(json).context("binding is not valid JSON")?;
            edit::apply(&dir, id, &catalog, |src| {
                edit::set(src, &slot, Some(&binding))
            })?;
        }
        [id, "clear", loc, k, n] => {
            let slot = SlotRef {
                at: at(loc),
                kind: kind(k)?,
                index: index(n)?,
            };
            edit::apply(&dir, id, &catalog, |src| edit::set(src, &slot, None))?;
        }
        [id, "swap", loc, k, n, loc2, n2] => {
            let k = kind(k)?;
            let a = SlotRef {
                at: at(loc),
                kind: k,
                index: index(n)?,
            };
            let b = SlotRef {
                at: at(loc2),
                kind: k,
                index: index(n2)?,
            };
            edit::apply(&dir, id, &catalog, |src| edit::swap(src, &a, &b))?;
        }
        [id, "page", "add"] => {
            edit::apply(&dir, id, &catalog, edit::page_add)?;
        }
        [id, "page", "remove", n] => {
            let n = index(n)?;
            edit::apply(&dir, id, &catalog, |src| edit::page_remove(src, n))?;
        }
        [id, "page", "move", from, to] => {
            let (from, to) = (index(from)?, index(to)?);
            edit::apply(&dir, id, &catalog, |src| edit::page_move(src, from, to))?;
        }
        [id, "name", name] => {
            edit::apply(&dir, id, &catalog, |src| edit::set_name(src, name))?;
        }
        [id, "match", class] => {
            edit::apply(&dir, id, &catalog, |src| {
                edit::set_match(src, edit::MatchField::Class, class)
            })?;
        }
        [id, "title", title] => {
            edit::apply(&dir, id, &catalog, |src| {
                edit::set_match(src, edit::MatchField::Title, title)
            })?;
        }
        [id, "create", name] => {
            edit::create(&dir, id, name, None)?;
        }
        [id, "create", name, from] => {
            edit::create(&dir, id, name, Some(from))?;
        }
        [id, "delete"] => edit::delete(&dir, id)?,
        _ => return Err(usage()),
    }
    Ok(())
}

fn export_json(id: &str) -> Result<()> {
    let dir = config::dir().context("neither XDG_CONFIG_HOME nor HOME is set")?;
    let loaded = config::Loaded::load(&dir)?;
    let p = loaded
        .profiles
        .get(id)
        .with_context(|| format!("unknown profile {id:?}"))?;
    println!("{}", serde_json::to_string(&profile_json(p))?);
    Ok(())
}

fn profile_json(p: &config::Profile) -> serde_json::Value {
    use serde_json::{Value, json};
    let slots = |s: &[config::Slot], n: usize| -> Value {
        (0..n)
            .map(|i| match s.get(i).and_then(|s| s.0.as_ref()) {
                Some(b) => json!({
                    "action": b.action,
                    "args": b.args,
                    "label": b.label,
                    "icon": b.icon,
                }),
                None => Value::Null,
            })
            .collect()
    };
    let page = |pg: &config::Page| json!({ "keys": slots(&pg.keys, config::KEYS), "dials": slots(&pg.dials, config::DIALS) });
    json!({
        "name": p.name,
        "match": p.matcher.as_ref().map(|m| json!({ "class": m.class, "title": m.title })),
        "pages": p.pages.iter().map(page).collect::<Vec<_>>(),
        "folders": p.folders.iter().map(|(k, v)| (k.clone(), page(v))).collect::<serde_json::Map<_, _>>(),
    })
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
    let subscribe = matches!(cmd, Command::Subscribe | Command::Learn);
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
            (false, _) if let Some(p) = &resp.preview => {
                for path in p.keys.iter().chain(&p.dials) {
                    println!("{}", path.display());
                }
            }
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
        assert_eq!(
            p(&["preview", "dev", "f"])?,
            Some(Command::Preview {
                profile: "dev".into(),
                page: None,
                folder: Some("f".into())
            })
        );
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
