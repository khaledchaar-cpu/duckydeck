//! Turns `PropertiesChanged` signals of system services (BlueZ,
//! power-profiles-daemon) into refreshes of status texts and toggle states.
//! Only services that some catalog entry refreshes on are watched.

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
    if let Err(e) = watch(catalog, texts, toggles).await {
        tracing::warn!(error = %e, "system bus watcher stopped");
    }
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
