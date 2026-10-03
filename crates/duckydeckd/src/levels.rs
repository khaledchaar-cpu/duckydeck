//! Runs dial commands one after another and keeps the shown levels current.
//!
//! Turns arriving while a command runs are summed up, so fast spinning ends
//! in one `omarchy` call instead of a queue. Audio levels follow
//! `pactl subscribe`; brightness has no event source and is read back after
//! each change.

use std::process::Stdio;
use std::time::Duration;

use duckydeck_core::CommandRunner;
use duckydeck_core::dial::{self, Dial, Levels};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::device::DeckEvent;
use crate::texts;
use duckydeck_core::status::Trigger;

/// Pause before restarting a `pactl subscribe` that ended.
const RESTART_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Twist(Dial, i32),
    Press(Dial),
    RefreshAudio,
}

pub async fn worker(
    runner: impl CommandRunner,
    mut jobs: UnboundedReceiver<Job>,
    events: UnboundedSender<DeckEvent>,
) {
    let mut levels = Levels::default();
    dial::read_audio(&runner, &mut levels).await;
    levels.brightness = dial::read_brightness(&runner).await;
    let _ = events.send(DeckEvent::Levels(levels));
    while let Some(first) = jobs.recv().await {
        let mut batch = vec![first];
        while let Ok(j) = jobs.try_recv() {
            batch.push(j);
        }
        let mut audio = false;
        let mut brightness = false;
        for job in coalesce(batch) {
            let spec = match job {
                Job::Twist(d, delta) => d.twist(delta),
                Job::Press(d) => d.press(),
                Job::RefreshAudio => {
                    audio = true;
                    None
                }
            };
            if let Job::Twist(Dial::Brightness { .. }, _) = job {
                brightness = true;
            }
            let Some(spec) = spec else { continue };
            match runner.run(&spec).await {
                Ok(out) if out.success() => {}
                res => tracing::warn!(args = ?spec.args, result = ?res, "dial command failed"),
            }
        }
        let before = levels;
        if audio {
            dial::read_audio(&runner, &mut levels).await;
        }
        if brightness {
            levels.brightness = dial::read_brightness(&runner).await;
        }
        if levels != before {
            let _ = events.send(DeckEvent::Levels(levels));
        }
    }
}

/// Sums consecutive turns of the same dial and drops duplicate refreshes.
fn coalesce(batch: Vec<Job>) -> Vec<Job> {
    let mut out: Vec<Job> = Vec::with_capacity(batch.len());
    let mut refresh = false;
    for job in batch {
        match (out.last_mut(), job) {
            (_, Job::RefreshAudio) => refresh = true,
            (Some(Job::Twist(d0, sum)), Job::Twist(d, delta)) if *d0 == d => *sum += delta,
            _ => out.push(job),
        }
    }
    if refresh {
        out.push(Job::RefreshAudio);
    }
    out
}

/// Feeds `pactl subscribe` events as refresh jobs; restarts it when it ends.
/// Also tells the status texts about each event.
pub async fn watch_audio(jobs: UnboundedSender<Job>, texts: UnboundedSender<texts::Msg>) {
    loop {
        if let Err(e) = subscribe(&jobs, &texts).await {
            tracing::warn!(error = %e, "pactl subscribe failed");
        }
        if jobs.is_closed() {
            return;
        }
        tokio::time::sleep(RESTART_DELAY).await;
    }
}

async fn subscribe(
    jobs: &UnboundedSender<Job>,
    texts: &UnboundedSender<texts::Msg>,
) -> std::io::Result<()> {
    // A stream, not a finished command, so not via `CommandRunner`.
    let mut child = tokio::process::Command::new("pactl")
        .arg("subscribe")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let Some(stdout) = child.stdout.take() else {
        return Ok(());
    };
    // The server may have changed while no subscription was running.
    let _ = jobs.send(Job::RefreshAudio);
    let _ = texts.send(texts::Msg::Event(Trigger::Audio));
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        if dial::is_audio_event(&line) {
            if jobs.send(Job::RefreshAudio).is_err() {
                return Ok(());
            }
            let _ = texts.send(texts::Msg::Event(Trigger::Audio));
        }
    }
    let status = child.wait().await;
    tracing::info!(?status, "pactl subscribe ended");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: Dial = Dial::Volume { step: 5 };
    const B: Dial = Dial::Brightness { step: 5 };

    #[test]
    fn coalesces_turns() {
        let batch = vec![
            Job::Twist(V, 1),
            Job::RefreshAudio,
            Job::Twist(V, 2),
            Job::Twist(B, -1),
            Job::Twist(B, 1),
            Job::Press(V),
            Job::RefreshAudio,
            Job::Twist(V, -1),
        ];
        assert_eq!(
            coalesce(batch),
            [
                Job::Twist(V, 3),
                Job::Twist(B, 0),
                Job::Press(V),
                Job::Twist(V, -1),
                Job::RefreshAudio,
            ]
        );
    }
}
