//! Tracks MPRIS players on the session bus and sends media key presses.
//!
//! Event-driven: `PropertiesChanged` for playback state and metadata,
//! `Seeked` for jumps, `NameOwnerChanged` for players appearing and leaving.
//! `Position` sends no signal; it is read once per change and extrapolated.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use duckydeck_core::media::{MPRIS_PREFIX, MediaKey, Player, Players, Status};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::names::{BusName, InterfaceName};
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream};

use crate::device::DeckEvent;

const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";

/// A media key press; `wanted` is the player from the binding's args.
#[derive(Debug, Clone)]
pub struct Press {
    pub key: MediaKey,
    pub wanted: Option<String>,
}

pub async fn run(presses: UnboundedReceiver<Press>, events: UnboundedSender<DeckEvent>) {
    if let Err(e) = watch(presses, events).await {
        tracing::warn!(error = %e, "MPRIS watcher stopped");
    }
}

async fn watch(
    mut presses: UnboundedReceiver<Press>,
    events: UnboundedSender<DeckEvent>,
) -> Result<()> {
    let conn = Connection::session().await?;
    let dbus = DBusProxy::new(&conn).await?;

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path(PATH)?
        .build();
    let mut changes = MessageStream::for_match_rule(rule, &conn, None).await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(PLAYER_IFACE)?
        .member("Seeked")?
        .path(PATH)?
        .build();
    let mut seeks = MessageStream::for_match_rule(rule, &conn, None).await?;
    let mut owners = dbus.receive_name_owner_changed().await?;

    let mut players = Players::default();
    // Unique name (`:1.42`) → well-known name; signals carry the unique one.
    let mut unique: HashMap<String, String> = HashMap::new();
    for name in dbus.list_names().await? {
        if name.starts_with(MPRIS_PREFIX) {
            add(&conn, &dbus, &mut players, &mut unique, name.as_str()).await;
        }
    }
    let _ = events.send(DeckEvent::Media(players.clone()));

    loop {
        tokio::select! {
            Some(press) = presses.recv() => {
                send_key(&conn, &players, &press).await;
                continue;
            }
            Some(msg) = changes.next() => {
                let Ok(msg) = msg else { continue };
                let Some(sender) = msg.header().sender().map(|s| s.to_string()) else { continue };
                let Some(name) = unique.get(&sender) else { continue };
                let Ok((iface, props, _)) = msg
                    .body()
                    .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                else {
                    continue;
                };
                if iface != PLAYER_IFACE {
                    continue;
                }
                let name = name.clone();
                apply(&mut players, &name, &props);
                if props.contains_key("PlaybackStatus") || props.contains_key("Metadata") {
                    read_position(&conn, &mut players, &name).await;
                }
            }
            Some(msg) = seeks.next() => {
                let Ok(msg) = msg else { continue };
                let Some(sender) = msg.header().sender().map(|s| s.to_string()) else { continue };
                let Some(name) = unique.get(&sender) else { continue };
                let Ok(us) = msg.body().deserialize::<i64>() else { continue };
                if let Some(p) = players.get_mut(name) {
                    p.position = Some((micros(us), Instant::now()));
                }
            }
            Some(sig) = owners.next() => {
                let Ok(args) = sig.args() else { continue };
                let name = args.name().to_string();
                if !name.starts_with(MPRIS_PREFIX) {
                    continue;
                }
                unique.retain(|_, n| *n != name);
                players.remove(&name);
                if args.new_owner().is_some() {
                    add(&conn, &dbus, &mut players, &mut unique, &name).await;
                }
            }
            else => return Ok(()),
        }
        if events.send(DeckEvent::Media(players.clone())).is_err() {
            return Ok(());
        }
    }
}

async fn add(
    conn: &Connection,
    dbus: &DBusProxy<'_>,
    players: &mut Players,
    unique: &mut HashMap<String, String>,
    name: &str,
) {
    let Ok(bus) = BusName::try_from(name) else {
        return;
    };
    if let Ok(owner) = dbus.get_name_owner(bus.clone()).await {
        unique.insert(owner.to_string(), name.to_owned());
    }
    players.insert(name);
    let props = async {
        let proxy = PropertiesProxy::builder(conn)
            .destination(bus)?
            .path(PATH)?
            .build()
            .await?;
        proxy
            .get_all(InterfaceName::from_static_str_unchecked(PLAYER_IFACE))
            .await
            .map_err(zbus::Error::from)
    };
    match props.await {
        Ok(p) => apply(players, name, &p),
        Err(e) => tracing::debug!(player = name, error = %e, "reading player failed"),
    }
    tracing::debug!(player = name, "MPRIS player added");
}

fn apply(players: &mut Players, name: &str, props: &HashMap<String, OwnedValue>) {
    if let Some(s) = props
        .get("PlaybackStatus")
        .and_then(|v| String::try_from(v.clone()).ok())
    {
        players.set_status(name, Status::parse(&s));
    }
    if let Some(meta) = props
        .get("Metadata")
        .and_then(|v| HashMap::<String, OwnedValue>::try_from(v.clone()).ok())
    {
        let p: &mut Player = players.insert(name);
        p.length = meta.get("mpris:length").and_then(as_i64).map(micros);
        p.title = meta
            .get("xesam:title")
            .and_then(|v| String::try_from(v.clone()).ok())
            .filter(|s| !s.is_empty());
        p.artist = meta
            .get("xesam:artist")
            .and_then(|v| Vec::<String>::try_from(v.clone()).ok())
            .map(|a| a.join(", "))
            .filter(|s| !s.is_empty());
    }
    let flag = |k: &str| props.get(k).and_then(|v| bool::try_from(v.clone()).ok());
    if let Some(b) = flag("CanGoNext") {
        players.insert(name).can_next = Some(b);
    }
    if let Some(b) = flag("CanGoPrevious") {
        players.insert(name).can_previous = Some(b);
    }
    if let Some(us) = props.get("Position").and_then(as_i64) {
        players.insert(name).position = Some((micros(us), Instant::now()));
    }
}

/// `Position` is not part of `PropertiesChanged`; read it after a change.
async fn read_position(conn: &Connection, players: &mut Players, name: &str) {
    let res = conn
        .call_method(
            Some(name),
            PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(PLAYER_IFACE, "Position"),
        )
        .await;
    let us = res
        .ok()
        .and_then(|m| m.body().deserialize::<OwnedValue>().ok())
        .and_then(|v| as_i64(&v));
    if let Some(p) = players.get_mut(name) {
        p.position = us.map(|us| (micros(us), Instant::now()));
    }
}

/// Players send times as `x` (spec) or `t`.
fn as_i64(v: &OwnedValue) -> Option<i64> {
    i64::try_from(v.clone()).ok().or_else(|| {
        u64::try_from(v.clone())
            .ok()
            .and_then(|n| i64::try_from(n).ok())
    })
}

fn micros(us: i64) -> Duration {
    Duration::from_micros(u64::try_from(us).unwrap_or(0))
}

async fn send_key(conn: &Connection, players: &Players, press: &Press) {
    let Some(p) = players.active(press.wanted.as_deref()) else {
        tracing::info!(key = ?press.key, "no media player running");
        return;
    };
    let res = conn
        .call_method(
            Some(p.name.as_str()),
            PATH,
            Some(PLAYER_IFACE),
            press.key.method(),
            &(),
        )
        .await;
    match res {
        Ok(_) => tracing::info!(player = %p.name, key = ?press.key, "media key sent"),
        Err(e) => tracing::warn!(player = %p.name, error = %e, "media key failed"),
    }
}
