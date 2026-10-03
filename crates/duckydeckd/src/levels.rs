//! Runs dial commands one after another and keeps the shown levels current.
//!
//! Turns arriving while a command runs are summed up, so fast spinning ends
//! in one `omarchy` call instead of a queue. Audio levels follow
//! `pactl subscribe`; brightness has no event source and is read back after
//! each change. App volume dials arrive with the app picked on the deck
//! and act on the last read streams.

use std::process::Stdio;
use std::time::Duration;

use duckydeck_core::CommandRunner;
use duckydeck_core::app_audio;
use duckydeck_core::dial::{self, Dial, Levels};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::device::DeckEvent;
use crate::texts;
use duckydeck_core::status::Trigger;

/// Pause before restarting a `pactl subscribe` that ended.
const RESTART_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
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
    let _ = events.send(DeckEvent::Levels(levels.clone()));
    while let Some(first) = jobs.recv().await {
        let mut batch = vec![first];
        while let Ok(j) = jobs.try_recv() {
            batch.push(j);
        }
        let mut audio = false;
        let mut brightness = false;
        // App whose new level is shown in the OSD after re-reading.
        let mut osd_app: Option<String> = None;
        for job in coalesce(batch) {
            let specs = match &job {
                Job::Twist(Dial::AppVolume { app, step }, delta) => {
                    osd_app.clone_from(app);
                    audio = true;
                    app_specs(&levels, app, |s| app_audio::twist(s, *step, *delta))
                }
                Job::Press(Dial::AppVolume { app, .. }) => {
                    osd_app.clone_from(app);
                    audio = true;
                    app_specs(&levels, app, app_audio::press)
                }
                Job::Twist(d, delta) => {
                    brightness |= matches!(d, Dial::Brightness { .. });
                    d.twist(*delta).into_iter().collect()
                }
                Job::Press(d) => d.press().into_iter().collect(),
                Job::RefreshAudio => {
                    audio = true;
                    Vec::new()
                }
            };
            for spec in specs {
                match runner.run(&spec).await {
                    Ok(out) if out.success() => {}
                    res => tracing::warn!(args = ?spec.args, result = ?res, "dial command failed"),
                }
            }
        }
        let before = levels.clone();
        if audio {
            dial::read_audio(&runner, &mut levels).await;
        }
        if brightness {
            levels.brightness = dial::read_brightness(&runner).await;
        }
        if let Some(app) = osd_app {
            let streams = app_audio::matching(&levels.streams, &app);
            if let (Some(pct), muted) = app_audio::level(&streams)
                && let Some(label) = streams.first().and_then(|s| s.label()).or(Some(&app))
            {
                let _ = runner.run(&app_audio::osd(label, pct, muted)).await;
            }
        }
        if levels != before {
            let _ = events.send(DeckEvent::Levels(levels.clone()));
        }
    }
}

/// Commands for the picked app's streams; none when no app is picked.
fn app_specs(
    levels: &Levels,
    app: &Option<String>,
    f: impl FnOnce(&[&app_audio::Stream]) -> Vec<duckydeck_core::CommandSpec>,
) -> Vec<duckydeck_core::CommandSpec> {
    app.as_deref()
        .map_or_else(Vec::new, |a| f(&app_audio::matching(&levels.streams, a)))
}

/// Sums consecutive turns of the same dial and drops duplicate refreshes.
fn coalesce(batch: Vec<Job>) -> Vec<Job> {
    let mut out: Vec<Job> = Vec::with_capacity(batch.len());
    let mut refresh = false;
    for job in batch {
        match (out.last_mut(), &job) {
            (_, Job::RefreshAudio) => refresh = true,
            (Some(Job::Twist(d0, sum)), Job::Twist(d, delta)) if d0 == d => *sum += delta,
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
        if dial::is_audio_event(&line) || app_audio::is_stream_event(&line) {
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
