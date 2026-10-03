//! Unix socket server for the `duckydeck` CLI (protocol: `docs/ipc.md`).
//! Requests go to the main loop, which owns all state; `subscribe`
//! connections then receive status events until the client hangs up.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use duckydeck_core::ipc::{Command, Event, Request, Response, VERSION};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::device::DeckEvent;

pub type Reply = oneshot::Sender<Response>;

/// Binds the socket; fails if another daemon already listens on it.
pub async fn bind(path: &Path) -> Result<UnixListener> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    if UnixStream::connect(path).await.is_ok() {
        bail!("another duckydeckd is running ({})", path.display());
    }
    // Left over from a crashed daemon.
    let _ = std::fs::remove_file(path);
    UnixListener::bind(path).with_context(|| format!("bind {}", path.display()))
}

pub async fn serve(
    listener: UnixListener,
    path: PathBuf,
    tx: mpsc::UnboundedSender<DeckEvent>,
    events: broadcast::Sender<Event>,
) {
    tracing::info!(socket = %path.display(), "IPC listening");
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let (tx, events) = (tx.clone(), events.clone());
                tokio::spawn(async move {
                    if let Err(e) = client(stream, tx, events).await {
                        tracing::debug!(error = %e, "IPC client ended");
                    }
                });
            }
            Err(e) => tracing::warn!(error = %e, "IPC accept failed"),
        }
    }
}

async fn client(
    stream: UnixStream,
    tx: mpsc::UnboundedSender<DeckEvent>,
    events: broadcast::Sender<Event>,
) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines.next_line().await? {
        let req = match serde_json::from_str::<Request>(&line) {
            Ok(r) if r.v == VERSION => r,
            Ok(r) => {
                send(
                    &mut write,
                    &Response::error(format!("unsupported version {}", r.v)),
                )
                .await?;
                continue;
            }
            Err(e) => {
                send(
                    &mut write,
                    &Response::error(format!("invalid request: {e}")),
                )
                .await?;
                continue;
            }
        };
        // Subscribe first so no change between status and stream is lost.
        let sub = (req.cmd == Command::Subscribe).then(|| events.subscribe());
        let (reply, answer) = oneshot::channel();
        tx.send(DeckEvent::Ipc(req.cmd, reply))?;
        let resp = answer.await?;
        let ok = resp.ok;
        send(&mut write, &resp).await?;
        if let (Some(mut sub), true) = (sub, ok) {
            loop {
                tokio::select! {
                    ev = sub.recv() => match ev {
                        Ok(ev) => send(&mut write, &ev).await?,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!(skipped = n, "IPC subscriber lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    },
                    // Further input is ignored; EOF means the client left.
                    line = lines.next_line() => if line?.is_none() {
                        return Ok(());
                    },
                }
            }
        }
    }
    Ok(())
}

async fn send(
    write: &mut tokio::net::unix::OwnedWriteHalf,
    msg: &impl serde::Serialize,
) -> Result<()> {
    let mut buf = serde_json::to_vec(msg)?;
    buf.push(b'\n');
    write.write_all(&buf).await?;
    Ok(())
}
