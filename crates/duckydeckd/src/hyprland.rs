//! Hyprland over its IPC sockets: workspace and window events in, dispatches out.
//!
//! Events from `.socket2.sock` drive the state; occupancy is re-read with
//! `j/workspaces` only after window/workspace events. Each request (query or
//! dispatch) uses its own short connection to `.socket.sock`, as Hyprland
//! closes it after one reply.

use std::time::Duration;

use anyhow::{Context, Result};
use duckydeck_core::hypr::{self, Event, Workspaces};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::device::DeckEvent;

/// Work for the Hyprland task.
#[derive(Debug)]
pub enum Request {
    /// A Lua dispatcher expression.
    Dispatch(String),
    /// Give all tiles on the active workspace the same size.
    Equalize,
}

/// Wait before reconnecting after Hyprland closed the event socket.
const RECONNECT: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn run(mut dispatches: UnboundedReceiver<Request>, events: UnboundedSender<DeckEvent>) {
    let (Some(cmd), Some(ev)) = (
        hypr::socket_path(".socket.sock"),
        hypr::socket_path(".socket2.sock"),
    ) else {
        tracing::warn!("not running under Hyprland: window actions disabled");
        while dispatches.recv().await.is_some() {}
        return;
    };
    loop {
        match watch(&cmd, &ev, &mut dispatches, &events).await {
            Ok(true) => return,
            Ok(false) => tracing::info!("Hyprland event socket closed"),
            Err(e) => tracing::warn!(error = %e, "Hyprland connection failed"),
        }
        // Keep serving dispatches while waiting; they fail visibly in the log.
        let wait = tokio::time::sleep(RECONNECT);
        tokio::pin!(wait);
        loop {
            tokio::select! {
                () = &mut wait => break,
                d = dispatches.recv() => match d {
                    Some(d) => handle(&cmd, d).await,
                    None => return,
                },
            }
        }
    }
}

/// Returns `Ok(true)` when the daemon is shutting down.
async fn watch(
    cmd: &std::path::Path,
    ev: &std::path::Path,
    dispatches: &mut UnboundedReceiver<Request>,
    events: &UnboundedSender<DeckEvent>,
) -> Result<bool> {
    let stream = UnixStream::connect(ev)
        .await
        .with_context(|| format!("connecting {}", ev.display()))?;
    let mut lines = BufReader::new(stream).lines();
    let mut ws = Workspaces::default();
    refresh(cmd, &mut ws, true).await?;
    tracing::debug!(state = ?ws, "Hyprland connected");
    if events.send(DeckEvent::Workspaces(ws.clone())).is_err() {
        return Ok(true);
    }
    match request(cmd, "j/activewindow")
        .await
        .and_then(|r| Ok(hypr::active_window(&r)?))
    {
        Ok((class, title)) => {
            if events.send(DeckEvent::Window(class, title)).is_err() {
                return Ok(true);
            }
        }
        Err(e) => tracing::debug!(error = %e, "reading active window failed"),
    }
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else { return Ok(false) };
                let before = ws.clone();
                match hypr::parse_event(&line) {
                    Some(Event::Active(id)) => ws.active = Some(id),
                    Some(Event::Changed) => {
                        if let Err(e) = refresh(cmd, &mut ws, false).await {
                            tracing::debug!(error = %e, "reading workspaces failed");
                        }
                    }
                    Some(Event::Window(class, title)) => {
                        if events.send(DeckEvent::Window(class, title)).is_err() {
                            return Ok(true);
                        }
                        continue;
                    }
                    None => continue,
                }
                if ws != before && events.send(DeckEvent::Workspaces(ws.clone())).is_err() {
                    return Ok(true);
                }
            }
            d = dispatches.recv() => match d {
                Some(d) => handle(cmd, d).await,
                None => return Ok(true),
            },
        }
    }
}

async fn refresh(cmd: &std::path::Path, ws: &mut Workspaces, active: bool) -> Result<()> {
    ws.set_occupied(&request(cmd, "j/workspaces").await?)?;
    if active {
        ws.set_active(&request(cmd, "j/activeworkspace").await?)?;
    }
    Ok(())
}

async fn handle(cmd: &std::path::Path, req: Request) {
    match req {
        Request::Dispatch(lua) => dispatch(cmd, &lua).await,
        Request::Equalize => {
            if let Err(e) = equalize(cmd).await {
                tracing::warn!(error = %e, "equalizing windows failed");
            }
        }
    }
}

/// One batch, so Hyprland applies it without other input in between.
async fn equalize(cmd: &std::path::Path) -> Result<()> {
    let active: serde_json::Value =
        serde_json::from_str(&request(cmd, "j/activeworkspace").await?)?;
    let ws = active
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .context("no active workspace")?;
    let focused: serde_json::Value = serde_json::from_str(&request(cmd, "j/activewindow").await?)?;
    let focused = focused.get("address").and_then(serde_json::Value::as_str);
    let Some(plan) = hypr::equalize(&request(cmd, "j/clients").await?, ws, focused)? else {
        tracing::debug!(workspace = ws, "nothing to equalize");
        return Ok(());
    };
    let body: Vec<String> = plan.iter().map(|d| format!("dispatch {d}")).collect();
    let reply = request(cmd, &format!("[[BATCH]]{}", body.join(";"))).await?;
    if reply
        .lines()
        .filter(|l| !l.trim().is_empty())
        .all(hypr::dispatch_ok)
    {
        tracing::info!(workspace = ws, "windows equalized");
    } else {
        tracing::warn!(
            workspace = ws,
            reply = reply.trim(),
            "equalize partly rejected"
        );
    }
    Ok(())
}

pub async fn dispatch(cmd: &std::path::Path, lua: &str) {
    match request(cmd, &format!("dispatch {lua}")).await {
        Ok(r) if hypr::dispatch_ok(&r) => tracing::info!(dispatch = lua, "dispatched"),
        Ok(r) => tracing::warn!(dispatch = lua, reply = r.trim(), "dispatch rejected"),
        Err(e) => tracing::warn!(dispatch = lua, error = %e, "dispatch failed"),
    }
}

async fn request(cmd: &std::path::Path, body: &str) -> Result<String> {
    let io = async {
        let mut s = UnixStream::connect(cmd).await?;
        s.write_all(body.as_bytes()).await?;
        let mut out = String::new();
        s.read_to_string(&mut out).await?;
        anyhow::Ok(out)
    };
    tokio::time::timeout(REQUEST_TIMEOUT, io)
        .await
        .context("Hyprland request timed out")?
}
