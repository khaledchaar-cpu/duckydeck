//! User script actions (`script.<name>`): runs the scripts and collects
//! what they want their keys to show.
//!
//! Default scripts get one process per event (the event is their only stdin
//! line). Persistent scripts run as long as a profile uses them, read every
//! event on stdin and send updates whenever they like. Every change goes
//! out as one full map. Like `pactl subscribe` in [`levels`](crate::levels),
//! these are streaming children, so they use `tokio::process` directly.

use std::collections::{BTreeMap, HashMap};
use std::process::Stdio;
use std::time::Duration;

use duckydeck_core::script::{Event, Script, State};
use duckydeck_core::{CommandRunner, CommandSpec, TokioRunner};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::device::DeckEvent;

pub type States = HashMap<String, State>;

/// One-shot scripts are killed after this.
const ONE_SHOT_LIMIT: Duration = Duration::from_secs(10);
/// A one-shot exit with an error within this time is notified.
const QUICK_FAIL: Duration = duckydeck_core::command::QUICK_FAIL;
/// Wait before restarting a persistent script that ended.
const RESTART_DELAY: Duration = Duration::from_secs(5);
/// A persistent script that ends this often within the window stays off.
const MAX_CRASHES: usize = 3;
const CRASH_WINDOW: Duration = Duration::from_secs(60);
/// Longest stdout line kept; longer lines are cut (and fail to parse).
const MAX_LINE: usize = 64 * 1024;

#[derive(Debug)]
pub enum Msg {
    /// Scripts the profiles use, with the args (JSON) of their first binding.
    /// Sends `init` to each; (re)starts persistent ones as needed.
    Sync(Vec<(Script, serde_json::Value)>),
    /// A press or turn on a script action.
    Event(String, Event),
}

/// What processes report back to the task.
enum Out {
    Line(String, String),
    /// A persistent process ended (id, generation).
    Exited(String, u64),
    /// Time to restart a persistent script.
    Restart(String),
}

struct Proc {
    script: Script,
    /// Lines for its stdin.
    stdin: UnboundedSender<String>,
    /// Dropping it kills the process.
    _kill: oneshot::Sender<()>,
    generation: u64,
}

#[derive(Default)]
struct Task {
    used: BTreeMap<String, (Script, serde_json::Value)>,
    procs: HashMap<String, Proc>,
    /// Recent ends of persistent scripts, for the restart limit.
    crashes: HashMap<String, Vec<Instant>>,
    states: States,
    generation: u64,
}

pub async fn run(mut msgs: UnboundedReceiver<Msg>, tx: UnboundedSender<DeckEvent>) {
    let (out_tx, mut out) = mpsc::unbounded_channel();
    let mut t = Task::default();
    loop {
        let before = t.states.clone();
        tokio::select! {
            msg = msgs.recv() => match msg {
                Some(Msg::Sync(scripts)) => t.sync(scripts, &out_tx),
                Some(Msg::Event(id, ev)) => t.event(&id, &ev, &out_tx),
                None => break,
            },
            Some(o) = out.recv() => t.output(o, &out_tx),
        }
        if t.states != before {
            tracing::debug!(states = ?t.states, "script states");
            if tx.send(DeckEvent::Scripts(t.states.clone())).is_err() {
                break;
            }
        }
    }
}

impl Task {
    fn sync(&mut self, scripts: Vec<(Script, serde_json::Value)>, out: &UnboundedSender<Out>) {
        self.used = scripts
            .into_iter()
            .map(|(s, args)| (s.id.clone(), (s, args)))
            .collect();
        // Stop persistent scripts that are gone, changed or no longer
        // persistent.
        self.procs.retain(|id, p| {
            let keep = self
                .used
                .get(id)
                .is_some_and(|(s, _)| s.persistent && *s == p.script);
            if !keep {
                tracing::info!(script = %id, "persistent script stopped");
            }
            keep
        });
        self.states.retain(|id, _| self.used.contains_key(id));
        // A reload gives crashed scripts another chance.
        self.crashes.clear();
        let used: Vec<(Script, serde_json::Value)> = self.used.values().cloned().collect();
        for (s, args) in used {
            if s.persistent && !self.procs.contains_key(&s.id) {
                self.start(&s, out);
            }
            let init = Event::Init { args };
            self.event(&s.id, &init, out);
        }
    }

    fn event(&mut self, id: &str, ev: &Event, out: &UnboundedSender<Out>) {
        let Some((s, _)) = self.used.get(id) else {
            tracing::warn!(script = %id, "unknown script action");
            return;
        };
        tracing::debug!(script = %id, event = ?ev, "script event");
        if s.persistent {
            match self.procs.get(id) {
                Some(p) => {
                    let _ = p.stdin.send(ev.line());
                }
                None => tracing::debug!(script = %id, "persistent script not running"),
            }
        } else {
            one_shot(s.clone(), ev.line(), out.clone());
        }
    }

    fn output(&mut self, o: Out, out: &UnboundedSender<Out>) {
        match o {
            Out::Line(id, line) => {
                if !self.used.contains_key(&id) {
                    return;
                }
                let mut state = self.states.get(&id).cloned().unwrap_or_default();
                match state.apply(&line) {
                    Ok(()) => {
                        self.states.insert(id, state);
                    }
                    Err(e) => {
                        tracing::warn!(script = %id, %line, error = %e, "invalid script output");
                    }
                }
            }
            Out::Exited(id, generation) => {
                if self
                    .procs
                    .get(&id)
                    .is_none_or(|p| p.generation != generation)
                {
                    return;
                }
                let script = self.procs.remove(&id).map(|p| p.script);
                let now = Instant::now();
                let crashes = self.crashes.entry(id.clone()).or_default();
                crashes.retain(|t| now.duration_since(*t) < CRASH_WINDOW);
                crashes.push(now);
                if crashes.len() >= MAX_CRASHES {
                    tracing::warn!(script = %id, "persistent script keeps ending; stays off");
                    let label = script.map_or_else(|| id.clone(), |s| s.label);
                    notify(
                        &label,
                        "ended 3 times within a minute; restarts after a reload",
                    );
                    return;
                }
                tracing::info!(script = %id, "persistent script ended; restarting soon");
                let (out, id) = (out.clone(), id.clone());
                tokio::spawn(async move {
                    tokio::time::sleep(RESTART_DELAY).await;
                    let _ = out.send(Out::Restart(id));
                });
            }
            Out::Restart(id) => {
                // Only if nothing (a sync) started it meanwhile.
                if self.procs.contains_key(&id) {
                    return;
                }
                if let Some((s, args)) = self.used.get(&id).cloned()
                    && s.persistent
                {
                    self.start(&s, out);
                    let init = Event::Init { args };
                    self.event(&id, &init, out);
                }
            }
        }
    }

    /// Starts a persistent script.
    fn start(&mut self, s: &Script, out: &UnboundedSender<Out>) {
        let mut child = match command(s).kill_on_drop(true).spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(script = %s.id, error = %e, "starting script failed");
                notify(&s.label, &e.to_string());
                return;
            }
        };
        self.generation += 1;
        let generation = self.generation;
        tracing::info!(script = %s.id, "persistent script started");
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        if let Some(mut stdin) = child.stdin.take() {
            tokio::spawn(async move {
                while let Some(line) = stdin_rx.recv().await {
                    if stdin.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                }
            });
        }
        read_lines(s.id.clone(), &mut child, out.clone());
        let (id, out) = (s.id.clone(), out.clone());
        tokio::spawn(async move {
            tokio::select! {
                res = child.wait() => {
                    tracing::debug!(script = %id, status = ?res, "script exited");
                    let _ = out.send(Out::Exited(id, generation));
                }
                // Sender dropped: stopped by a sync.
                _ = kill_rx => {
                    let _ = child.kill().await;
                }
            }
        });
        self.procs.insert(
            s.id.clone(),
            Proc {
                script: s.clone(),
                stdin: stdin_tx,
                _kill: kill_tx,
                generation,
            },
        );
    }
}

fn command(s: &Script) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(&s.path);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = s.path.parent() {
        cmd.current_dir(dir);
    }
    cmd
}

/// Forwards stdout lines as updates and logs stderr.
fn read_lines(id: String, child: &mut tokio::process::Child, out: UnboundedSender<Out>) {
    if let Some(stdout) = child.stdout.take() {
        let id = id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                line.truncate(MAX_LINE);
                if out.send(Out::Line(id.clone(), line)).is_err() {
                    break;
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(script = %id, "{line}");
            }
        });
    }
}

/// Runs a script for one event; its output updates the key.
fn one_shot(s: Script, line: String, out: UnboundedSender<Out>) {
    tokio::spawn(async move {
        let mut child = match command(&s).kill_on_drop(true).spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(script = %s.id, error = %e, "starting script failed");
                notify(&s.label, &e.to_string());
                return;
            }
        };
        let started = Instant::now();
        if let Some(mut stdin) = child.stdin.take() {
            // A script that ignores stdin may exit before reading it.
            let _ = stdin.write_all(line.as_bytes()).await;
        }
        read_lines(s.id.clone(), &mut child, out);
        match tokio::time::timeout(ONE_SHOT_LIMIT, child.wait()).await {
            Ok(Ok(st)) if !st.success() && started.elapsed() < QUICK_FAIL => {
                tracing::warn!(script = %s.id, status = %st, "script failed");
                notify(&s.label, &st.to_string());
            }
            Ok(Ok(_)) => {}
            Ok(Err(e)) => tracing::warn!(script = %s.id, error = %e, "waiting for script failed"),
            Err(_) => {
                tracing::warn!(script = %s.id, "script killed after 10 s");
                let _ = child.kill().await;
            }
        }
    });
}

/// Shell notification for a failing script.
fn notify(label: &str, reason: &str) {
    let spec = CommandSpec::omarchy(["notification", "send", "--app-name", "DuckyDeck"])
        .args([format!("{label} failed"), reason.to_owned()]);
    if let Err(e) = TokioRunner.spawn(&spec) {
        tracing::warn!(error = %e, "script failure notification failed");
    }
}
