//! Automatic profile switching: which profile fits the active window.

use regex_lite::Regex;

use crate::config::{Loaded, WindowMatch};

/// One profile's compiled `match`; all given regexes must match.
#[derive(Debug)]
pub struct Rule {
    profile: String,
    class: Option<Regex>,
    title: Option<Regex>,
}

impl Rule {
    pub fn new(profile: &str, m: &WindowMatch) -> Result<Self, String> {
        let compile = |field: &str, re: &Option<String>| {
            re.as_deref()
                .map(Regex::new)
                .transpose()
                .map_err(|e| format!("match.{field}: {e}"))
        };
        Ok(Self {
            profile: profile.to_owned(),
            class: compile("class", &m.class)?,
            title: compile("title", &m.title)?,
        })
    }

    fn matches(&self, class: &str, title: &str) -> bool {
        (self.class.is_some() || self.title.is_some())
            && self.class.as_ref().is_none_or(|r| r.is_match(class))
            && self.title.as_ref().is_none_or(|r| r.is_match(title))
    }
}

/// Rules of all profiles, in profile id order.
#[derive(Debug, Default)]
pub struct Rules(Vec<Rule>);

impl Rules {
    /// Invalid regexes are rejected by [`Loaded::load`] and skipped here.
    pub fn new(loaded: &Loaded) -> Self {
        Self(
            loaded
                .profiles
                .iter()
                .filter_map(|(id, p)| Rule::new(id, p.matcher.as_ref()?).ok())
                .collect(),
        )
    }

    /// First profile whose rule matches the window.
    pub fn profile_for(&self, class: &str, title: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|r| r.matches(class, title))
            .map(|r| r.profile.as_str())
    }
}

/// Picks the profile to show and remembers the one chosen by hand.
#[derive(Debug)]
pub struct Context {
    rules: Rules,
    /// Shown when no rule matches: `config.profile` or the last manual choice.
    manual: String,
    /// Active window class and title, empty before the first event.
    window: (String, String),
}

impl Context {
    pub fn new(loaded: &Loaded) -> Self {
        Self {
            rules: Rules::new(loaded),
            manual: loaded.config.profile.clone(),
            window: Default::default(),
        }
    }

    /// After a config reload; `reset` when `config.profile` changed.
    pub fn reload(&mut self, loaded: &Loaded, reset: bool) {
        self.rules = Rules::new(loaded);
        if reset || !loaded.profiles.contains_key(&self.manual) {
            self.manual = loaded.config.profile.clone();
        }
    }

    /// A profile chosen via CLI or panel stays until the next window change.
    pub fn set_manual(&mut self, profile: &str) {
        self.manual = profile.to_owned();
    }

    /// The window got focus; returns the profile it wants.
    pub fn focus(&mut self, class: &str, title: &str) -> &str {
        self.window = (class.to_owned(), title.to_owned());
        self.wanted()
    }

    /// Profile for the current window (after a reload, for example).
    pub fn wanted(&self) -> &str {
        let (class, title) = &self.window;
        self.rules.profile_for(class, title).unwrap_or(&self.manual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() -> Loaded {
        let mut l = Loaded::load(std::path::Path::new("/nonexistent")).unwrap();
        let mut p = l.profiles["omarchy"].clone();
        p.matcher = Some(WindowMatch {
            class: Some("^(Alacritty|kitty)$".into()),
            title: None,
        });
        l.profiles.insert("term".into(), p.clone());
        p.matcher = Some(WindowMatch {
            class: Some("firefox".into()),
            title: Some("YouTube".into()),
        });
        l.profiles.insert("video".into(), p);
        l
    }

    #[test]
    fn matches_class_and_title() {
        let r = Rules::new(&loaded());
        assert_eq!(r.profile_for("kitty", "x"), Some("term"));
        assert_eq!(r.profile_for("kitty2", "x"), None);
        assert_eq!(
            r.profile_for("org.mozilla.firefox", "YouTube - Mozilla"),
            Some("video")
        );
        assert_eq!(r.profile_for("org.mozilla.firefox", "News"), None);
    }

    #[test]
    fn falls_back_to_manual_choice() {
        let l = loaded();
        let mut c = Context::new(&l);
        assert_eq!(c.focus("kitty", ""), "term");
        assert_eq!(c.focus("code", ""), "omarchy");
        c.set_manual("video");
        assert_eq!(c.wanted(), "video");
        assert_eq!(c.focus("kitty", ""), "term");
        assert_eq!(c.focus("code", ""), "video");
        c.reload(&l, true);
        assert_eq!(c.wanted(), "omarchy");
    }

    #[test]
    fn rejects_bad_regex() {
        let m = WindowMatch {
            class: Some("(".into()),
            title: None,
        };
        assert!(Rule::new("x", &m).unwrap_err().starts_with("match.class"));
    }
}
