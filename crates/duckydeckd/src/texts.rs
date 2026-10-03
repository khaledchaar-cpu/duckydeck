//! Status texts of catalog actions (`text` in the catalog).
//!
//! Each text is read at startup, shortly after a press of its action and on
//! the events in its `refresh` list (audio from `pactl subscribe`, media
//! from MPRIS). Every change goes out as one full map.

use std::collections::HashMap;
use std::time::Duration;

use duckydeck_core::TokioRunner;
use duckydeck_core::catalog::Catalog;
use duckydeck_core::status::{TextSource, Trigger};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::device::DeckEvent;

pub type Texts = HashMap<String, String>;

/// Reads after a press: the change may take a moment to show.
const RECHECKS: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(2500)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// An action was pressed (its id).
    Press(String),
    Event(Trigger),
}

pub async fn run(
    catalog: Catalog,
    mut msgs: UnboundedReceiver<Msg>,
    tx: UnboundedSender<DeckEvent>,
) {
    let sources: Vec<(String, TextSource)> = catalog
        .iter()
        .filter_map(|(id, e)| Some((id.clone(), e.text.clone()?)))
        .collect();
    if sources.is_empty() {
        return;
    }
    let mut texts = Texts::new();
    for (id, s) in &sources {
        set(&mut texts, id, s.read(&TokioRunner).await);
    }
    let _ = tx.send(DeckEvent::Texts(texts.clone()));

    let (due_tx, mut due) = mpsc::unbounded_channel::<String>();
    loop {
        let mut ids: Vec<&str> = Vec::new();
        tokio::select! {
            msg = msgs.recv() => {
                let Some(first) = msg else { break };
                // Events come in bursts (a volume turn); read once per burst.
                let mut batch = vec![first];
                while let Ok(m) = msgs.try_recv() {
                    batch.push(m);
                }
                for m in batch {
                    match m {
                        Msg::Press(id) => {
                            if sources.iter().any(|(s, _)| *s == id) {
                                for d in RECHECKS {
                                    let (due_tx, id) = (due_tx.clone(), id.clone());
                                    tokio::spawn(async move {
                                        tokio::time::sleep(d).await;
                                        let _ = due_tx.send(id);
                                    });
                                }
                            }
                        }
                        Msg::Event(t) => ids.extend(
                            sources
                                .iter()
                                .filter(|(_, s)| s.refresh.contains(&t))
                                .map(|(id, _)| id.as_str()),
                        ),
                    }
                }
            }
            Some(id) = due.recv() => {
                if let Some((id, _)) = sources.iter().find(|(s, _)| *s == id) {
                    ids.push(id);
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        let mut next = texts.clone();
        for id in ids {
            if let Some((_, s)) = sources.iter().find(|(s, _)| s == id) {
                set(&mut next, id, s.read(&TokioRunner).await);
            }
        }
        if next != texts {
            texts = next;
            tracing::debug!(?texts, "status texts");
            if tx.send(DeckEvent::Texts(texts.clone())).is_err() {
                break;
            }
        }
    }
}

fn set(t: &mut Texts, id: &str, text: Option<String>) {
    match text {
        Some(s) => t.insert(id.to_owned(), s),
        None => t.remove(id),
    };
}
