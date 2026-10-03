//! Label font: the Omarchy system font (`omarchy font current`), resolved to
//! a file with fontconfig so the renderer loads just that one face.

use crate::command::{CommandRunner, CommandSpec};

/// Font file bytes of the current system font, `None` if anything fails.
pub async fn system_font(runner: &dyn CommandRunner) -> Option<Vec<u8>> {
    let family = run(runner, CommandSpec::omarchy(["font", "current"])).await?;
    let file = run(
        runner,
        CommandSpec::new("fc-match").args(["-f", "%{file}", family.as_str()]),
    )
    .await?;
    match std::fs::read(&file) {
        Ok(data) => Some(data),
        Err(e) => {
            tracing::warn!(file, error = %e, "reading system font failed");
            None
        }
    }
}

async fn run(runner: &dyn CommandRunner, spec: CommandSpec) -> Option<String> {
    match runner.run(&spec).await {
        Ok(out) if out.success() => {
            let s = out.stdout.trim().to_owned();
            (!s.is_empty()).then_some(s)
        }
        Ok(out) => {
            tracing::warn!(
                program = spec.program,
                stderr = out.stderr.trim(),
                "font lookup failed"
            );
            None
        }
        Err(e) => {
            tracing::warn!(program = spec.program, error = %e, "font lookup failed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutput, RecordingRunner};

    #[tokio::test]
    async fn resolves_family_then_file() {
        let runner = RecordingRunner::with_response(CommandOutput {
            status: Some(0),
            stdout: "/nonexistent/Font.ttf\n".into(),
            stderr: String::new(),
        });
        assert!(system_font(&runner).await.is_none());
        assert_eq!(
            runner.command_lines(),
            [
                "omarchy font current",
                "fc-match -f %{file} /nonexistent/Font.ttf"
            ]
        );
    }

    #[tokio::test]
    async fn stops_on_failure() {
        let runner = RecordingRunner::with_response(CommandOutput {
            status: Some(1),
            ..CommandOutput::default()
        });
        assert!(system_font(&runner).await.is_none());
        assert_eq!(runner.command_lines().len(), 1);
    }
}
