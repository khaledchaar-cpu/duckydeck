//! Structure actions that wrap other bindings: `structure.multi` (a sequence
//! with delays), `structure.toggle` (alternates between two bindings) and
//! `structure.profile` (switches the profile).

use std::time::Duration;

use crate::config::Binding;

pub const MULTI_ACTION: &str = "structure.multi";
pub const TOGGLE_ACTION: &str = "structure.toggle";
/// Args: `profile = "<id>"`.
pub const PROFILE_ACTION: &str = "structure.profile";

/// Longest allowed `delay_ms` in a multi action.
const MAX_DELAY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Run(Binding),
    Delay(Duration),
}

/// `args.steps`: `{ action, args }` or `{ delay_ms }` tables.
pub fn steps(b: &Binding) -> Result<Vec<Step>, String> {
    let steps = array(b, "steps")?;
    if steps.is_empty() {
        return Err(format!("{MULTI_ACTION}: args.steps is empty"));
    }
    steps
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let t = v
                .as_table()
                .ok_or_else(|| format!("{MULTI_ACTION}: step {} is not a table", i + 1))?;
            if let Some(ms) = t.get("delay_ms") {
                let ms = ms
                    .as_integer()
                    .and_then(|ms| u64::try_from(ms).ok())
                    .map(Duration::from_millis)
                    .filter(|d| *d <= MAX_DELAY && t.len() == 1)
                    .ok_or_else(|| {
                        format!(
                            "{MULTI_ACTION}: step {}: delay_ms must be 0-10000 alone",
                            i + 1
                        )
                    })?;
                return Ok(Step::Delay(ms));
            }
            nested(t)
                .map(Step::Run)
                .map_err(|e| format!("{MULTI_ACTION}: step {}: {e}", i + 1))
        })
        .collect()
}

/// `args.states`: exactly two bindings, run alternately starting with the first.
pub fn states(b: &Binding) -> Result<[Binding; 2], String> {
    let states = array(b, "states")?
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_table()
                .ok_or_else(|| "not a table".to_owned())
                .and_then(nested)
                .map_err(|e| format!("{TOGGLE_ACTION}: state {}: {e}", i + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;
    <[Binding; 2]>::try_from(states)
        .map_err(|_| format!("{TOGGLE_ACTION}: args.states needs exactly 2 entries"))
}

/// `args.profile` of a profile action.
pub fn profile(b: &Binding) -> Result<&str, String> {
    b.args
        .get("profile")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{PROFILE_ACTION} needs args.profile"))
}

/// Structural check of a compound binding; other actions pass.
pub fn validate(b: &Binding) -> Result<(), String> {
    match b.action.as_str() {
        MULTI_ACTION => steps(b).map(drop),
        TOGGLE_ACTION => states(b).map(drop),
        PROFILE_ACTION => profile(b).map(drop),
        _ => Ok(()),
    }
}

fn array<'a>(b: &'a Binding, key: &str) -> Result<&'a Vec<toml::Value>, String> {
    b.args
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("{} needs args.{key} (array)", b.action))
}

/// A binding inside a compound; it may not be a compound or folder itself,
/// except a profile switch.
fn nested(t: &toml::Table) -> Result<Binding, String> {
    let str_of = |k: &str| match t.get(k) {
        None => Ok(None),
        Some(v) => v
            .as_str()
            .map(|s| Some(s.to_owned()))
            .ok_or_else(|| format!("{k} must be a string")),
    };
    if let Some(k) = t
        .keys()
        .find(|k| !["action", "args", "label", "icon"].contains(&k.as_str()))
    {
        return Err(format!("unknown field {k:?}"));
    }
    let b = Binding {
        action: str_of("action")?.ok_or("missing action")?,
        args: match t.get("args") {
            None => toml::Table::new(),
            Some(v) => v.as_table().cloned().ok_or("args must be a table")?,
        },
        label: str_of("label")?,
        icon: str_of("icon")?,
    };
    if b.action.starts_with("structure.") && b.action != PROFILE_ACTION {
        return Err(format!("{:?} cannot be nested", b.action));
    }
    validate(&b)?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(src: &str) -> Binding {
        let t: toml::Table = toml::from_str(src).unwrap();
        Binding {
            action: t["action"].as_str().unwrap().to_owned(),
            args: t["args"].as_table().unwrap().clone(),
            label: None,
            icon: None,
        }
    }

    #[test]
    fn parses_steps() {
        let b = binding(
            r#"action = "structure.multi"
            args = { steps = [{ action = "system.lock" }, { delay_ms = 300 }, { action = "window.workspace", args = { n = 2 } }] }"#,
        );
        let s = steps(&b).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s[1], Step::Delay(Duration::from_millis(300)));
        assert!(matches!(&s[2], Step::Run(b) if b.args["n"].as_integer() == Some(2)));
    }

    #[test]
    fn rejects_bad_steps() {
        for (src, msg) in [
            (r#"args = { steps = [] }"#, "empty"),
            (r#"args = { steps = [{ delay_ms = 20000 }] }"#, "delay_ms"),
            (
                r#"args = { steps = [{ action = "structure.multi", args = {} }] }"#,
                "nested",
            ),
            (
                r#"args = { steps = [{ action = "x", foo = 1 }] }"#,
                "unknown field",
            ),
            (r#"args = {}"#, "needs args.steps"),
        ] {
            let b = binding(&format!("action = \"structure.multi\"\n{src}"));
            let e = steps(&b).unwrap_err();
            assert!(e.contains(msg), "{src}: {e}");
        }
    }

    #[test]
    fn parses_states() {
        let b = binding(
            r#"action = "structure.toggle"
            args = { states = [{ action = "a", icon = "x" }, { action = "structure.profile", args = { profile = "p" } }] }"#,
        );
        let [a, p] = states(&b).unwrap();
        assert_eq!(a.icon.as_deref(), Some("x"));
        assert_eq!(profile(&p), Ok("p"));
        let one = binding(
            r#"action = "structure.toggle"
            args = { states = [{ action = "a" }] }"#,
        );
        assert!(states(&one).unwrap_err().contains("exactly 2"));
    }
}
