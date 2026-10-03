//! External process execution.
//!
//! Every external process (omarchy, pactl, notify-send, apps) goes through
//! [`CommandRunner`]. Production uses [`TokioRunner`]; tests use
//! [`RecordingRunner`], which never spawns anything.

use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Default timeout for a single command.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// A program plus its argument list. Never a shell string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Convenience for `omarchy <args…>`.
    pub fn omarchy<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new("omarchy").args(args)
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Result of a finished command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandOutput {
    /// Exit code; `None` if the process was killed by a signal.
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status == Some(0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("failed to spawn `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{program}` timed out after {timeout:?}")]
    Timeout { program: String, timeout: Duration },
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Runs external processes.
pub trait CommandRunner: Send + Sync {
    /// Runs the command to completion and captures its output.
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
    ) -> BoxFuture<'a, Result<CommandOutput, CommandError>>;

    /// Starts the command detached (apps, long-running tools) without waiting for it.
    fn spawn(&self, spec: &CommandSpec) -> Result<(), CommandError>;
}

/// Production runner based on `tokio::process::Command`.
#[derive(Debug, Default, Clone)]
pub struct TokioRunner;

impl TokioRunner {
    fn command(spec: &CommandSpec) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&spec.program);
        cmd.args(&spec.args).stdin(Stdio::null());
        cmd
    }
}

impl CommandRunner for TokioRunner {
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
    ) -> BoxFuture<'a, Result<CommandOutput, CommandError>> {
        Box::pin(async move {
            let mut cmd = Self::command(spec);
            cmd.kill_on_drop(true);
            let output = tokio::time::timeout(spec.timeout, cmd.output())
                .await
                .map_err(|_| CommandError::Timeout {
                    program: spec.program.clone(),
                    timeout: spec.timeout,
                })?
                .map_err(|source| CommandError::Spawn {
                    program: spec.program.clone(),
                    source,
                })?;
            Ok(CommandOutput {
                status: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        })
    }

    fn spawn(&self, spec: &CommandSpec) -> Result<(), CommandError> {
        let mut child = Self::command(spec)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| CommandError::Spawn {
                program: spec.program.clone(),
                source,
            })?;
        let program = spec.program.clone();
        // Reap the child so it does not linger as a zombie.
        tokio::spawn(async move {
            if let Err(err) = child.wait().await {
                tracing::warn!(%program, %err, "waiting for detached process failed");
            }
        });
        Ok(())
    }
}

/// How a [`RecordingRunner`] call was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Run,
    Spawn,
}

/// Test runner: records calls and returns a canned output. Never spawns processes.
#[derive(Debug, Clone, Default)]
pub struct RecordingRunner {
    calls: Arc<Mutex<Vec<(CallKind, CommandSpec)>>>,
    response: Arc<Mutex<CommandOutput>>,
}

impl RecordingRunner {
    pub fn new() -> Self {
        Self::with_response(CommandOutput {
            status: Some(0),
            ..CommandOutput::default()
        })
    }

    pub fn with_response(response: CommandOutput) -> Self {
        Self {
            calls: Arc::default(),
            response: Arc::new(Mutex::new(response)),
        }
    }

    /// All recorded calls in order.
    pub fn calls(&self) -> Vec<(CallKind, CommandSpec)> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Recorded calls as `program arg1 arg2 …` strings, handy for assertions.
    pub fn command_lines(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .map(|(_, spec)| {
                std::iter::once(spec.program)
                    .chain(spec.args)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    fn record(&self, kind: CallKind, spec: &CommandSpec) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push((kind, spec.clone()));
        }
    }
}

impl CommandRunner for RecordingRunner {
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
    ) -> BoxFuture<'a, Result<CommandOutput, CommandError>> {
        self.record(CallKind::Run, spec);
        let response = self.response.lock().map(|r| r.clone()).unwrap_or_default();
        Box::pin(async move { Ok(response) })
    }

    fn spawn(&self, spec: &CommandSpec) -> Result<(), CommandError> {
        self.record(CallKind::Spawn, spec);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recording_runner_records_without_spawning() {
        let runner = RecordingRunner::new();
        let out = runner
            .run(&CommandSpec::omarchy(["audio", "output", "volume", "+5"]))
            .await
            .unwrap();
        assert!(out.success());
        runner.spawn(&CommandSpec::new("firefox")).unwrap();
        assert_eq!(
            runner.command_lines(),
            ["omarchy audio output volume +5", "firefox"]
        );
        assert_eq!(runner.calls()[1].0, CallKind::Spawn);
    }

    #[tokio::test]
    async fn tokio_runner_passes_args_verbatim() {
        let out = TokioRunner
            .run(&CommandSpec::new("printf").args(["%s|", "a b", "$HOME"]))
            .await
            .unwrap();
        assert!(out.success());
        assert_eq!(out.stdout, "a b|$HOME|");
    }

    #[tokio::test]
    async fn tokio_runner_times_out() {
        let err = TokioRunner
            .run(
                &CommandSpec::new("sleep")
                    .arg("5")
                    .timeout(Duration::from_millis(50)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CommandError::Timeout { .. }));
    }

    #[tokio::test]
    async fn tokio_runner_reports_missing_program() {
        let err = TokioRunner
            .run(&CommandSpec::new("duckydeck-does-not-exist"))
            .await
            .unwrap_err();
        assert!(matches!(err, CommandError::Spawn { .. }));
    }
}
