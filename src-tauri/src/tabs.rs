use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileLifecycle {
    Open,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabLifecycle {
    Empty,
    Creating,
    Ready,
    Closing,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabNavigation {
    pub current_url: Option<String>,
    pub entries: Vec<String>,
    pub index: Option<usize>,
    pub loading: bool,
    pub error: Option<String>,
    pub pending_history_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabState {
    pub id: String,
    pub webview_label: String,
    pub lifecycle: TabLifecycle,
    pub title: Option<String>,
    pub favicon_uri: Option<String>,
    pub navigation: TabNavigation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileState {
    pub window_label: String,
    pub chrome_label: String,
    pub lifecycle: ProfileLifecycle,
    pub ordered_tab_ids: Vec<String>,
    pub active_tab_id: String,
    pub tabs: HashMap<String, TabState>,
}

#[derive(Default)]
pub struct BrowserRegistry {
    profiles: HashMap<String, ProfileState>,
    next_tab_id: u64,
    webview_labels: HashSet<String>,
}

impl BrowserRegistry {
    fn new_tab(&mut self, profile_id: &str, webview_label: String) -> TabState {
        self.next_tab_id += 1;
        TabState {
            id: format!("{profile_id}-tab-{}", self.next_tab_id),
            webview_label,
            lifecycle: TabLifecycle::Empty,
            title: None,
            favicon_uri: None,
            navigation: TabNavigation::default(),
        }
    }

    /// Idempotently registers a profile and its initial empty tab.
    pub fn open_profile(
        &mut self,
        profile_id: &str,
        window_label: String,
        chrome_label: String,
        initial_webview_label: String,
    ) -> Result<String, String> {
        if let Some(profile) = self.profiles.get(profile_id) {
            if profile.lifecycle == ProfileLifecycle::Closing {
                return Err(format!("Profile {profile_id} is closing."));
            }
            return Ok(profile.active_tab_id.clone());
        }
        if self.webview_labels.contains(&initial_webview_label)
            || self.webview_labels.contains(&chrome_label)
            || initial_webview_label == chrome_label
        {
            return Err("That WebView label is already registered.".into());
        }
        let tab = self.new_tab(profile_id, initial_webview_label.clone());
        let id = tab.id.clone();
        let mut tabs = HashMap::new();
        tabs.insert(id.clone(), tab);
        self.webview_labels.insert(chrome_label.clone());
        self.webview_labels.insert(initial_webview_label);
        self.profiles.insert(
            profile_id.to_owned(),
            ProfileState {
                window_label,
                chrome_label,
                lifecycle: ProfileLifecycle::Open,
                ordered_tab_ids: vec![id.clone()],
                active_tab_id: id.clone(),
                tabs,
            },
        );
        Ok(id)
    }

    pub fn add_tab(&mut self, profile_id: &str, webview_label: String) -> Result<String, String> {
        if self.webview_labels.contains(&webview_label) {
            return Err("That WebView label is already registered.".into());
        }
        self.open_profile_mut(profile_id)?;
        let tab = self.new_tab(profile_id, webview_label.clone());
        let id = tab.id.clone();
        let profile = self.open_profile_mut(profile_id)?;
        profile.ordered_tab_ids.push(id.clone());
        profile.tabs.insert(id.clone(), tab);
        self.webview_labels.insert(webview_label);
        Ok(id)
    }

    pub fn select_tab(&mut self, profile_id: &str, tab_id: &str) -> Result<(), String> {
        let profile = self.open_profile_mut(profile_id)?;
        if !profile.tabs.contains_key(tab_id) {
            return Err("The tab does not belong to this profile.".into());
        }
        profile.active_tab_id = tab_id.to_owned();
        Ok(())
    }

    /// Marks a tab as closing, removes it, then selects the tab now occupying its
    /// position (or the preceding final tab). The final tab remains as an empty tab.
    pub fn close_tab(&mut self, profile_id: &str, tab_id: &str) -> Result<String, String> {
        let (active_id, removed_label) = {
            let profile = self.open_profile_mut(profile_id)?;
            let position = profile
                .ordered_tab_ids
                .iter()
                .position(|id| id == tab_id)
                .ok_or_else(|| "The tab does not belong to this profile.".to_owned())?;
            if profile.ordered_tab_ids.len() == 1 {
                let tab = profile
                    .tabs
                    .get_mut(tab_id)
                    .expect("tab index is consistent");
                tab.lifecycle = TabLifecycle::Empty;
                tab.title = None;
                tab.favicon_uri = None;
                tab.navigation = TabNavigation::default();
                return Ok(tab.id.clone());
            }
            if let Some(tab) = profile.tabs.get_mut(tab_id) {
                tab.lifecycle = TabLifecycle::Closing;
            }
            let removed = profile
                .tabs
                .remove(tab_id)
                .expect("tab index is consistent");
            profile.ordered_tab_ids.remove(position);
            if profile.active_tab_id == tab_id {
                let next = position.min(profile.ordered_tab_ids.len() - 1);
                profile.active_tab_id = profile.ordered_tab_ids[next].clone();
            }
            (profile.active_tab_id.clone(), removed.webview_label)
        };
        self.webview_labels.remove(&removed_label);
        Ok(active_id)
    }

    pub fn begin_tab_creation(&mut self, profile_id: &str, tab_id: &str) -> Result<(), String> {
        let tab = self.open_tab_mut(profile_id, tab_id)?;
        if tab.lifecycle != TabLifecycle::Empty {
            return Err("The tab is not in the empty state.".into());
        }
        tab.lifecycle = TabLifecycle::Creating;
        Ok(())
    }

    pub fn finish_tab_creation(
        &mut self,
        profile_id: &str,
        tab_id: &str,
        succeeded: bool,
    ) -> Result<(), String> {
        let tab = self.open_tab_mut(profile_id, tab_id)?;
        if tab.lifecycle != TabLifecycle::Creating {
            return Err("The tab is not being created.".into());
        }
        tab.lifecycle = if succeeded {
            TabLifecycle::Ready
        } else {
            TabLifecycle::Empty
        };
        Ok(())
    }

    pub fn begin_profile_close(&mut self, profile_id: &str) -> Result<(), String> {
        let profile = self
            .profiles
            .get_mut(profile_id)
            .ok_or_else(|| "The profile is not registered.".to_owned())?;
        profile.lifecycle = ProfileLifecycle::Closing;
        Ok(())
    }

    pub fn cancel_profile_close(&mut self, profile_id: &str) {
        if let Some(profile) = self.profiles.get_mut(profile_id) {
            profile.lifecycle = ProfileLifecycle::Open;
        }
    }

    pub fn remove_profile(&mut self, profile_id: &str) {
        if let Some(profile) = self.profiles.remove(profile_id) {
            self.webview_labels.remove(&profile.chrome_label);
            for tab in profile.tabs.values() {
                self.webview_labels.remove(&tab.webview_label);
            }
        }
    }

    pub fn profile(&self, profile_id: &str) -> Option<&ProfileState> {
        self.profiles.get(profile_id)
    }

    pub fn active_navigation(&self, profile_id: &str) -> Option<&TabNavigation> {
        let profile = self.profiles.get(profile_id)?;
        if profile.lifecycle == ProfileLifecycle::Closing {
            return None;
        }
        profile
            .tabs
            .get(&profile.active_tab_id)
            .map(|tab| &tab.navigation)
    }

    pub fn active_navigation_mut(&mut self, profile_id: &str) -> Option<&mut TabNavigation> {
        let profile = self.open_profile_mut(profile_id).ok()?;
        let active_id = profile.active_tab_id.clone();
        profile
            .tabs
            .get_mut(&active_id)
            .map(|tab| &mut tab.navigation)
    }

    /// Resolves callbacks by stable tab identity. A late native callback for a
    /// closed tab returns `None` and cannot mutate whichever tab is now active.
    pub fn navigation_mut(&mut self, profile_id: &str, tab_id: &str) -> Option<&mut TabNavigation> {
        self.open_tab_mut(profile_id, tab_id)
            .ok()
            .map(|tab| &mut tab.navigation)
    }

    fn open_profile_mut(&mut self, profile_id: &str) -> Result<&mut ProfileState, String> {
        let profile = self
            .profiles
            .get_mut(profile_id)
            .ok_or_else(|| "The profile is not open.".to_owned())?;
        if profile.lifecycle == ProfileLifecycle::Closing {
            return Err("The profile is closing.".into());
        }
        Ok(profile)
    }

    fn open_tab_mut(&mut self, profile_id: &str, tab_id: &str) -> Result<&mut TabState, String> {
        self.open_profile_mut(profile_id)?
            .tabs
            .get_mut(tab_id)
            .ok_or_else(|| "The tab does not belong to this profile.".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{BrowserRegistry, ProfileLifecycle, TabLifecycle};

    fn open(registry: &mut BrowserRegistry, profile: &str) -> String {
        registry
            .open_profile(
                profile,
                format!("window-{profile}"),
                format!("chrome-{profile}"),
                format!("content-{profile}"),
            )
            .unwrap()
    }

    #[test]
    fn ids_are_unique_and_opening_creates_one_initial_tab_idempotently() {
        let mut registry = BrowserRegistry::default();
        let first = open(&mut registry, "alpha");
        assert_eq!(open(&mut registry, "alpha"), first);
        let second = registry.add_tab("alpha", "content-alpha-2".into()).unwrap();
        assert_ne!(first, second);
        assert_eq!(registry.profile("alpha").unwrap().ordered_tab_ids.len(), 2);
    }

    #[test]
    fn profiles_and_navigation_are_isolated_and_ownership_is_enforced() {
        let mut registry = BrowserRegistry::default();
        let _alpha = open(&mut registry, "alpha");
        let beta = open(&mut registry, "beta");
        registry
            .active_navigation_mut("alpha")
            .unwrap()
            .entries
            .push("https://alpha.test".into());
        registry
            .active_navigation_mut("beta")
            .unwrap()
            .entries
            .push("https://beta.test".into());
        assert!(registry.select_tab("alpha", &beta).is_err());
        assert_eq!(
            registry.active_navigation("alpha").unwrap().entries,
            ["https://alpha.test"]
        );
        assert_eq!(
            registry.active_navigation("beta").unwrap().entries,
            ["https://beta.test"]
        );
        registry.remove_profile("alpha");
        assert!(registry.profile("alpha").is_none());
        assert!(registry.profile("beta").is_some());
    }

    #[test]
    fn selection_close_and_neighbor_choice_work_for_active_and_inactive_tabs() {
        let mut registry = BrowserRegistry::default();
        let first = open(&mut registry, "alpha");
        let second = registry.add_tab("alpha", "content-alpha-2".into()).unwrap();
        let third = registry.add_tab("alpha", "content-alpha-3".into()).unwrap();
        registry.select_tab("alpha", &second).unwrap();
        assert_eq!(registry.close_tab("alpha", &first).unwrap(), second);
        assert_eq!(registry.close_tab("alpha", &second).unwrap(), third);
        assert_eq!(registry.profile("alpha").unwrap().active_tab_id, third);
    }

    #[test]
    fn each_tab_keeps_independent_navigation_and_stale_closed_tab_callbacks_are_ignored() {
        let mut registry = BrowserRegistry::default();
        let first = open(&mut registry, "alpha");
        let second = registry.add_tab("alpha", "content-alpha-2".into()).unwrap();
        registry
            .navigation_mut("alpha", &first)
            .unwrap()
            .entries
            .push("https://first.test".into());
        registry
            .navigation_mut("alpha", &second)
            .unwrap()
            .entries
            .push("https://second.test".into());
        assert_eq!(
            registry.navigation_mut("alpha", &first).unwrap().entries,
            ["https://first.test"]
        );
        assert_eq!(
            registry.navigation_mut("alpha", &second).unwrap().entries,
            ["https://second.test"]
        );
        registry.close_tab("alpha", &first).unwrap();
        assert!(registry.navigation_mut("alpha", &first).is_none());
        assert_eq!(
            registry.navigation_mut("alpha", &second).unwrap().entries,
            ["https://second.test"]
        );
    }

    #[test]
    fn closing_inactive_tab_keeps_active_tab_and_final_tab_becomes_empty() {
        let mut registry = BrowserRegistry::default();
        let first = open(&mut registry, "alpha");
        let second = registry.add_tab("alpha", "content-alpha-2".into()).unwrap();
        registry.select_tab("alpha", &second).unwrap();
        assert_eq!(registry.close_tab("alpha", &first).unwrap(), second);
        assert_eq!(registry.close_tab("alpha", &second).unwrap(), second);
        assert_eq!(
            registry.profile("alpha").unwrap().tabs[&second].lifecycle,
            TabLifecycle::Empty
        );
    }

    #[test]
    fn lifecycle_transitions_reject_duplicates_and_failed_creation_can_retry() {
        let mut registry = BrowserRegistry::default();
        let tab = open(&mut registry, "alpha");
        assert!(registry.add_tab("alpha", "content-alpha".into()).is_err());
        registry.begin_tab_creation("alpha", &tab).unwrap();
        assert!(registry.begin_tab_creation("alpha", &tab).is_err());
        registry.finish_tab_creation("alpha", &tab, false).unwrap();
        registry.begin_tab_creation("alpha", &tab).unwrap();
        registry.finish_tab_creation("alpha", &tab, true).unwrap();
        assert_eq!(
            registry.profile("alpha").unwrap().tabs[&tab].lifecycle,
            TabLifecycle::Ready
        );
        assert!(registry
            .open_profile(
                "beta",
                "window-beta".into(),
                "chrome-alpha".into(),
                "content-beta".into()
            )
            .is_err());
    }

    #[test]
    fn closing_profile_blocks_mutations_and_removal_only_cleans_its_tabs() {
        let mut registry = BrowserRegistry::default();
        let _alpha = open(&mut registry, "alpha");
        let _beta = open(&mut registry, "beta");
        registry.begin_profile_close("alpha").unwrap();
        assert_eq!(
            registry.profile("alpha").unwrap().lifecycle,
            ProfileLifecycle::Closing
        );
        assert!(registry.active_navigation("alpha").is_none());
        assert!(registry.add_tab("alpha", "extra".into()).is_err());
        registry.remove_profile("alpha");
        assert!(registry.profile("beta").is_some());
        assert!(registry.add_tab("beta", "content-beta-2".into()).is_ok());
    }
}
