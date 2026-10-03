//! Turns `PropertiesChanged` signals of system services (BlueZ,
//! power-profiles-daemon) into refreshes of status texts and toggle states.
//! Only services that some catalog entry refreshes on are watched.
//! Omarchy reminders are systemd user timers; their creation and removal
//! come from the user manager on the session bus.

use std::collections::BTreeSet;

use anyhow::Result;
use duckydeck_core::catalog::Catalog;
use duckydeck_core::status::Trigger;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::{StreamExt, StreamMap};
use zbus::{Connection, MatchRule, MessageStream};

use crate::texts;

pub async fn run(
    catalog: Catalog,
    texts: UnboundedSender<texts::Msg>,
    toggles: UnboundedSender<String>,
) {
    if refreshes_on(&catalog, Trigger::Reminder) {
        let texts = texts.clone();
        tokio::spawn(async move {
            if let Err(e) = watch_reminders(texts).await {
                tracing::warn!(error = %e, "reminder watcher stopped");
            }
        });
    }
    if let Err(e) = watch(catalog, texts, toggles).await {
        tracing::warn!(error = %e, "system bus watcher stopped");
    }
}

fn refreshes_on(catalog: &Catalog, trigger: Trigger) -> bool {
    catalog.iter().any(|(_, e)| {
        e.text
            .as_ref()
            .is_some_and(|t| t.refresh.contains(&trigger))
    })
}

/// Unit names of Omarchy reminders (`omarchy-reminder-<id>.timer`).
fn is_reminder_unit(name: &str) -> bool {
    name.starts_with("omarchy-reminder-")
}

async fn watch_reminders(texts: UnboundedSender<texts::Msg>) -> Result<()> {
    const PATH: &str = "/org/freedesktop/systemd1";
    const IFACE: &str = "org.freedesktop.systemd1.Manager";
    let conn = Connection::session().await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.systemd1")?
        .path(PATH)?
        .interface(IFACE)?
        .build();
    let mut stream = MessageStream::for_match_rule(rule, &conn, None).await?;
    // The manager only emits unit signals to subscribed clients.
    conn.call_method(
        Some("org.freedesktop.systemd1"),
        PATH,
        Some(IFACE),
        "Subscribe",
        &(),
    )
    .await?;
    while let Some(msg) = stream.next().await {
        let Ok(msg) = msg else { continue };
        let header = msg.header();
        let Some(member) = header.member() else {
            continue;
        };
        if !matches!(member.as_str(), "UnitNew" | "UnitRemoved") {
            continue;
        }
        let Ok((name, _)) = msg
            .body()
            .deserialize::<(String, zbus::zvariant::OwnedObjectPath)>()
        else {
            continue;
        };
        if is_reminder_unit(&name) && texts.send(texts::Msg::Event(Trigger::Reminder)).is_err() {
            break;
        }
    }
    Ok(())
}

async fn watch(
    catalog: Catalog,
    texts: UnboundedSender<texts::Msg>,
    toggles: UnboundedSender<String>,
) -> Result<()> {
    let triggers: BTreeSet<Trigger> = catalog
        .iter()
        .flat_map(|(_, e)| {
            let t = e.text.iter().flat_map(|t| t.refresh.iter());
            t.chain(e.state.iter().flat_map(|s| s.refresh.iter()))
                .copied()
        })
        .filter(|t| t.system_bus_service().is_some())
        .collect();
    if triggers.is_empty() {
        return Ok(());
    }
    let conn = Connection::system().await?;
    let mut streams = StreamMap::new();
    for t in triggers {
        let Some(service) = t.system_bus_service() else {
            continue;
        };
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(service)?
            .interface("org.freedesktop.DBus.Properties")?
            .member("PropertiesChanged")?
            .build();
        let stream = MessageStream::for_match_rule(rule, &conn, None).await?;
        streams.insert(t, stream);
    }
    while let Some((t, _)) = streams.next().await {
        if texts.send(texts::Msg::Event(t)).is_err() {
            break;
        }
        for (id, e) in catalog.iter() {
            if e.state.as_ref().is_some_and(|s| s.refresh.contains(&t)) {
                let _ = toggles.send(id.clone());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminder_units() {
        assert!(is_reminder_unit("omarchy-reminder-1696360000.timer"));
        assert!(is_reminder_unit("omarchy-reminder-1696360000.service"));
        assert!(!is_reminder_unit("duckydeck.service"));
    }
}
