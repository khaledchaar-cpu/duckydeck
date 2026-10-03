//! Which profile, page and folder the deck shows, and what a key press on
//! a structure action does. Pure state, no device or rendering.

use crate::config::{Binding, DIALS, FOLDER_ACTION, KEYS, Loaded, Page, Profile};

pub const BACK_ACTION: &str = "structure.back";
/// Args: `n = <1-based page>` or `to = "next" | "prev"`.
pub const PAGE_ACTION: &str = "structure.page";
/// Dial: twist pages through the profile, press opens page 1.
pub const PAGE_SCROLL_ACTION: &str = "structure.page_scroll";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nav {
    pub profile: String,
    /// 0-based index into `pages`.
    pub page: usize,
    /// Open folder, shown instead of the page.
    pub folder: Option<String>,
}

/// What a key press means after structure actions are handled.
#[derive(Debug, Clone, PartialEq)]
pub enum Press {
    /// Navigation changed; redraw everything.
    Navigated,
    /// A regular action to run.
    Run(Binding),
    /// Empty slot or no-op navigation.
    Nothing,
}

impl Nav {
    pub fn new(loaded: &Loaded) -> Self {
        Self {
            profile: loaded.config.profile.clone(),
            page: 0,
            folder: None,
        }
    }

    /// Keeps the position after a reload where it still exists.
    pub fn reconcile(&mut self, loaded: &Loaded) {
        let Some(profile) = loaded.profiles.get(&self.profile) else {
            *self = Self::new(loaded);
            return;
        };
        if self.page >= profile.pages.len() {
            self.page = 0;
        }
        if self
            .folder
            .as_ref()
            .is_some_and(|f| !profile.folders.contains_key(f))
        {
            self.folder = None;
        }
    }

    fn current<'a>(&self, loaded: &'a Loaded) -> Option<(&'a Profile, &'a Page)> {
        let profile = loaded.profiles.get(&self.profile)?;
        let page = match &self.folder {
            Some(f) => profile.folders.get(f)?,
            None => profile.pages.get(self.page)?,
        };
        Some((profile, page))
    }

    /// The 8 keys of the current view; folders get the back key last.
    pub fn keys(&self, loaded: &Loaded) -> [Option<Binding>; KEYS] {
        let mut keys: [Option<Binding>; KEYS] = Default::default();
        if let Some((_, page)) = self.current(loaded) {
            for (slot, b) in keys.iter_mut().zip(&page.keys) {
                slot.clone_from(&b.0);
            }
        }
        if self.folder.is_some() {
            keys[KEYS - 1] = Some(Binding {
                action: BACK_ACTION.to_owned(),
                args: toml::Table::new(),
                label: None,
                icon: None,
            });
        }
        keys
    }

    pub fn dials(&self, loaded: &Loaded) -> [Option<Binding>; DIALS] {
        let mut dials: [Option<Binding>; DIALS] = Default::default();
        if let Some((_, page)) = self.current(loaded) {
            for (slot, b) in dials.iter_mut().zip(&page.dials) {
                slot.clone_from(&b.0);
            }
        }
        dials
    }

    pub fn press_key(&mut self, loaded: &Loaded, key: usize) -> Press {
        match self.keys(loaded).get(key).cloned().flatten() {
            Some(b) => self.apply(loaded, b),
            None => Press::Nothing,
        }
    }

    /// Strip swipe: positive = next page, negative = previous (wraps).
    pub fn step_page(&mut self, loaded: &Loaded, delta: isize) -> Press {
        let Some(count) = loaded.profiles.get(&self.profile).map(|p| p.pages.len()) else {
            return Press::Nothing;
        };
        if count < 2 && self.folder.is_none() {
            return Press::Nothing;
        }
        let page = (self.page as isize + delta).rem_euclid(count as isize) as usize;
        self.go(page)
    }

    fn apply(&mut self, loaded: &Loaded, b: Binding) -> Press {
        match b.action.as_str() {
            FOLDER_ACTION => {
                // Validated by the parser.
                self.folder = b
                    .args
                    .get("folder")
                    .and_then(|v| v.as_str())
                    .map(Into::into);
                Press::Navigated
            }
            BACK_ACTION if self.folder.is_some() => {
                self.folder = None;
                Press::Navigated
            }
            BACK_ACTION => Press::Nothing,
            PAGE_ACTION => {
                if let Some(n) = b.args.get("n").and_then(|v| v.as_integer()) {
                    let count = loaded
                        .profiles
                        .get(&self.profile)
                        .map_or(0, |p| p.pages.len());
                    match usize::try_from(n - 1) {
                        Ok(page) if page < count => self.go(page),
                        _ => Press::Nothing,
                    }
                } else {
                    match b.args.get("to").and_then(|v| v.as_str()) {
                        Some("prev") => self.step_page(loaded, -1),
                        _ => self.step_page(loaded, 1),
                    }
                }
            }
            _ => Press::Run(b),
        }
    }

    /// Page-scroll dial pressed: back to the first page.
    pub fn first_page(&mut self) -> Press {
        self.go(0)
    }

    /// Opens the 0-based `page` of the active profile (IPC `set_page`).
    pub fn set_page(&mut self, loaded: &Loaded, page: usize) -> Result<Press, String> {
        let count = loaded
            .profiles
            .get(&self.profile)
            .map_or(0, |p| p.pages.len());
        if page >= count {
            return Err(format!("page {} out of range 1-{count}", page + 1));
        }
        Ok(self.go(page))
    }

    /// Switches to the first page of `profile` (IPC `set_profile`).
    pub fn set_profile(&mut self, loaded: &Loaded, profile: &str) -> Result<Press, String> {
        if !loaded.profiles.contains_key(profile) {
            return Err(format!("unknown profile {profile:?}"));
        }
        if self.profile == profile && self.page == 0 && self.folder.is_none() {
            return Ok(Press::Nothing);
        }
        *self = Self {
            profile: profile.to_owned(),
            page: 0,
            folder: None,
        };
        Ok(Press::Navigated)
    }

    fn go(&mut self, page: usize) -> Press {
        if self.page == page && self.folder.is_none() {
            return Press::Nothing;
        }
        self.page = page;
        self.folder = None;
        Press::Navigated
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::config::Config;

    const SRC: &str = r#"
        name = "T"
        [[pages]]
        keys = [
          { action = "structure.folder", args = { folder = "f" } },
          { action = "structure.page", args = { to = "next" } },
          {},
          { action = "system.lock" },
        ]
        dials = [{ action = "media.volume" }]
        [[pages]]
        keys = [{ action = "structure.page", args = { n = 1 } }]
        [folders.f]
        keys = [{ action = "system.suspend" }]
    "#;

    fn loaded(src: &str) -> anyhow::Result<Loaded> {
        Ok(Loaded {
            config: Config {
                profile: "t".into(),
                ..Config::default()
            },
            profiles: BTreeMap::from([("t".into(), Profile::parse(src, Path::new("t.toml"))?)]),
        })
    }

    fn action(b: &Option<Binding>) -> Option<&str> {
        b.as_ref().map(|b| b.action.as_str())
    }

    #[test]
    fn folder_and_back() -> anyhow::Result<()> {
        let l = loaded(SRC)?;
        let mut nav = Nav::new(&l);
        assert_eq!(action(&nav.dials(&l)[0]), Some("media.volume"));
        assert_eq!(nav.press_key(&l, 2), Press::Nothing);
        assert!(matches!(nav.press_key(&l, 3), Press::Run(b) if b.action == "system.lock"));

        assert_eq!(nav.press_key(&l, 0), Press::Navigated);
        let keys = nav.keys(&l);
        assert_eq!(action(&keys[0]), Some("system.suspend"));
        assert_eq!(action(&keys[7]), Some(BACK_ACTION));
        assert!(nav.dials(&l).iter().all(Option::is_none));
        assert_eq!(nav.press_key(&l, 7), Press::Navigated);
        assert_eq!(nav.folder, None);
        Ok(())
    }

    #[test]
    fn pages() -> anyhow::Result<()> {
        let l = loaded(SRC)?;
        let mut nav = Nav::new(&l);
        assert_eq!(nav.press_key(&l, 1), Press::Navigated);
        assert_eq!(nav.page, 1);
        assert_eq!(nav.press_key(&l, 0), Press::Navigated);
        assert_eq!(nav.page, 0);
        assert_eq!(nav.step_page(&l, -1), Press::Navigated);
        assert_eq!(nav.page, 1);
        assert_eq!(nav.step_page(&l, 1), Press::Navigated);
        assert_eq!(nav.page, 0);
        Ok(())
    }

    #[test]
    fn reconcile_after_reload() -> anyhow::Result<()> {
        let mut nav = Nav {
            profile: "t".into(),
            page: 1,
            folder: Some("f".into()),
        };
        let smaller = loaded("name = \"T\"\n[[pages]]")?;
        nav.reconcile(&smaller);
        assert_eq!((nav.page, nav.folder.as_deref()), (0, None));

        nav.profile = "gone".into();
        nav.reconcile(&smaller);
        assert_eq!(nav.profile, "t");
        Ok(())
    }

    #[test]
    fn ipc_page_and_profile() -> anyhow::Result<()> {
        let l = loaded(SRC)?;
        let mut nav = Nav::new(&l);
        nav.press_key(&l, 0);
        assert_eq!(nav.set_page(&l, 0), Ok(Press::Navigated));
        assert_eq!(nav.folder, None);
        assert_eq!(nav.set_page(&l, 1), Ok(Press::Navigated));
        assert!(nav.set_page(&l, 2).is_err());
        assert!(nav.set_profile(&l, "nope").is_err());
        assert_eq!(nav.set_profile(&l, "t"), Ok(Press::Navigated));
        assert_eq!(nav.page, 0);
        assert_eq!(nav.set_profile(&l, "t"), Ok(Press::Nothing));
        Ok(())
    }
}
