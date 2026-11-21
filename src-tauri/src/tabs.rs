use serde::Serialize;
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
    pub generation: u64,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabCreationReservation {
    pub tab_id: String,
    pub webview_label: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSnapshot {
    pub id: String,
    pub webview_label: String,
    pub generation: u64,
    pub title: Option<String>,
    pub favicon_uri: Option<String>,
    pub current_url: Option<String>,
    pub lifecycle: String,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTabsSnapshot {
    pub profile_id: String,
    pub active_tab_id: String,
    pub tabs: Vec<TabSnapshot>,
}

#[derive(Default)]
pub struct BrowserRegistry {
    profiles: HashMap<String, ProfileState>,
    next_tab_id: u64,
    webview_labels: HashSet<String>,
}

impl BrowserRegistry {
    fn new_tab(&mut self, profile_id: &str) -> Result<TabState, String> {
        loop {
            self.next_tab_id = self
                .next_tab_id
                .checked_add(1)
                .ok_or_else(|| "The browser tab identifier space is exhausted.".to_owned())?;
            let id = format!("{profile_id}-tab-{}", self.next_tab_id);
            let webview_label = format!("content-{id}");
            if !self.webview_labels.contains(&webview_label) {
                return Ok(TabState {
                    webview_label,
                    id,
                    lifecycle: TabLifecycle::Empty,
                    generation: 0,
                    title: None,
                    favicon_uri: None,
                    navigation: TabNavigation::default(),
                });
            }
        }
    }

    fn new_tab_with_label(
        &mut self,
        profile_id: &str,
        webview_label: String,
    ) -> Result<TabState, String> {
        let mut tab = self.new_tab(profile_id)?;
        tab.webview_label = webview_label;
        Ok(tab)
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
        if self.webview_labels.contains(&chrome_label)
            || self.webview_labels.contains(&initial_webview_label)
            || chrome_label == initial_webview_label
        {
            return Err("That WebView label is already registered.".into());
        }
        let tab = self.new_tab_with_label(profile_id, initial_webview_label)?;
        let id = tab.id.clone();
        let content_label = tab.webview_label.clone();
        let mut tabs = HashMap::new();
        tabs.insert(id.clone(), tab);
        self.webview_labels.insert(chrome_label.clone());
        self.webview_labels.insert(content_label);
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

    /// Registers a profile with a generated, stable content label for its initial tab.
    pub fn open_profile_generated(
        &mut self,
        profile_id: &str,
        window_label: String,
        chrome_label: String,
    ) -> Result<String, String> {
        if let Some(profile) = self.profiles.get(profile_id) {
            if profile.window_label != window_label || profile.chrome_label != chrome_label {
                return Err(
                    "The profile is already registered with different native labels.".into(),
                );
            }
            return Ok(profile.active_tab_id.clone());
        }
        if self.webview_labels.contains(&chrome_label) {
            return Err("That WebView label is already registered.".into());
        }
        let tab = self.new_tab(profile_id)?;
        if tab.webview_label == chrome_label {
            return Err(
                "The generated content WebView label conflicts with the chrome label.".into(),
            );
        }
        let id = tab.id.clone();
        let content_label = tab.webview_label.clone();
        let mut tabs = HashMap::new();
        tabs.insert(id.clone(), tab);
        self.webview_labels.insert(chrome_label.clone());
        self.webview_labels.insert(content_label);
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

    /// Preserves the earlier API for callers that provide their own unique label.
    pub fn add_tab(&mut self, profile_id: &str, webview_label: String) -> Result<String, String> {
        if self.webview_labels.contains(&webview_label) {
            return Err("That WebView label is already registered.".into());
        }
        self.open_profile_mut(profile_id)?;
        let tab = self.new_tab_with_label(profile_id, webview_label.clone())?;
        let id = tab.id.clone();
        let profile = self.open_profile_mut(profile_id)?;
        profile.ordered_tab_ids.push(id.clone());
        profile.tabs.insert(id.clone(), tab);
        self.webview_labels.insert(webview_label);
        Ok(id)
    }

    pub fn add_tab_generated(&mut self, profile_id: &str) -> Result<String, String> {
        self.open_profile_mut(profile_id)?;
        let tab = self.new_tab(profile_id)?;
        let id = tab.id.clone();
        let label = tab.webview_label.clone();
        if self.webview_labels.contains(&label) {
            return Err("A generated WebView label is already registered.".into());
        }
        let profile = self.open_profile_mut(profile_id)?;
        profile.ordered_tab_ids.push(id.clone());
        profile.tabs.insert(id.clone(), tab);
        self.webview_labels.insert(label);
        Ok(id)
    }

    pub fn reserve_creation(
        &mut self,
        profile_id: &str,
        tab_id: &str,
    ) -> Result<TabCreationReservation, String> {
        let tab = self.open_tab_mut(profile_id, tab_id)?;
        if tab.lifecycle != TabLifecycle::Empty {
            return Err("This tab is already being created or has a website open.".into());
        }
        tab.generation = tab
            .generation
            .checked_add(1)
            .ok_or_else(|| "Tab creation generation is exhausted.".to_owned())?;
        tab.lifecycle = TabLifecycle::Creating;
        Ok(TabCreationReservation {
            tab_id: tab.id.clone(),
            webview_label: tab.webview_label.clone(),
            generation: tab.generation,
        })
    }

    /// Returns false when the reservation became stale while native creation ran.
    pub fn complete_creation(
        &mut self,
        profile_id: &str,
        reservation: &TabCreationReservation,
        succeeded: bool,
    ) -> bool {
        let Some(profile) = self.profiles.get_mut(profile_id) else {
            return false;
        };
        if profile.lifecycle != ProfileLifecycle::Open {
            return false;
        }
        let Some(tab) = profile.tabs.get_mut(&reservation.tab_id) else {
            return false;
        };
        if tab.generation != reservation.generation || tab.lifecycle != TabLifecycle::Creating {
            return false;
        }
        tab.lifecycle = if succeeded {
            TabLifecycle::Ready
        } else {
            tab.title = None;
            tab.favicon_uri = None;
            tab.navigation = TabNavigation::default();
            TabLifecycle::Empty
        };
        true
    }

    pub fn tab_label(&self, profile_id: &str, tab_id: &str) -> Result<String, String> {
        self.profiles
            .get(profile_id)
            .filter(|profile| profile.lifecycle == ProfileLifecycle::Open)
            .and_then(|profile| profile.tabs.get(tab_id))
            .map(|tab| tab.webview_label.clone())
            .ok_or_else(|| "The tab does not belong to this open profile.".to_owned())
    }

    pub fn tab_snapshot(&self, profile_id: &str, tab_id: &str) -> Result<TabSnapshot, String> {
        let tab = self
            .profiles
            .get(profile_id)
            .filter(|profile| profile.lifecycle == ProfileLifecycle::Open)
            .and_then(|profile| profile.tabs.get(tab_id))
            .ok_or_else(|| "The tab does not belong to this open profile.".to_owned())?;
        Ok(snapshot(tab))
    }

    pub fn navigation_snapshot(
        &self,
        profile_id: &str,
        tab_id: &str,
    ) -> Result<TabNavigation, String> {
        let tab = self
            .profiles
            .get(profile_id)
            .filter(|profile| profile.lifecycle == ProfileLifecycle::Open)
            .and_then(|profile| profile.tabs.get(tab_id))
            .ok_or_else(|| "The tab does not belong to this open profile.".to_owned())?;
        Ok(tab.navigation.clone())
    }

    pub fn profile_tabs_snapshot(&self, profile_id: &str) -> Result<ProfileTabsSnapshot, String> {
        let profile = self
            .profiles
            .get(profile_id)
            .filter(|profile| profile.lifecycle == ProfileLifecycle::Open)
            .ok_or_else(|| "The profile is not open.".to_owned())?;
        let tabs = profile
            .ordered_tab_ids
            .iter()
            .filter_map(|id| profile.tabs.get(id).map(snapshot))
            .collect();
        Ok(ProfileTabsSnapshot {
            profile_id: profile_id.to_owned(),
            active_tab_id: profile.active_tab_id.clone(),
            tabs,
        })
    }

    pub fn select_tab(&mut self, profile_id: &str, tab_id: &str) -> Result<(), String> {
        let profile = self.open_profile_mut(profile_id)?;
        if !profile.tabs.contains_key(tab_id) {
            return Err("The tab does not belong to this profile.".into());
        }
        profile.active_tab_id = tab_id.to_owned();
        Ok(())
    }

    pub fn neighboring_tab(&self, profile_id: &str, tab_id: &str) -> Result<String, String> {
        let profile = self
            .profiles
            .get(profile_id)
            .filter(|profile| profile.lifecycle == ProfileLifecycle::Open)
            .ok_or_else(|| "The profile is not open.".to_owned())?;
        let position = profile
            .ordered_tab_ids
            .iter()
            .position(|id| id == tab_id)
            .ok_or_else(|| "The tab does not belong to this profile.".to_owned())?;
        if profile.ordered_tab_ids.len() == 1 {
            return Ok(tab_id.to_owned());
        }
        Ok(
            profile.ordered_tab_ids[if position == profile.ordered_tab_ids.len() - 1 {
                position - 1
            } else {
                position + 1
            }]
            .clone(),
        )
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
                tab.generation = tab.generation.saturating_add(1);
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
        self.reserve_creation(profile_id, tab_id)?;
        Ok(())
    }

    pub fn finish_tab_creation(
        &mut self,
        profile_id: &str,
        tab_id: &str,
        succeeded: bool,
    ) -> Result<(), String> {
        let reservation = {
            let tab = self.open_tab_mut(profile_id, tab_id)?;
            if tab.lifecycle != TabLifecycle::Creating {
                return Err("The tab is not being created.".into());
            }
            TabCreationReservation {
                tab_id: tab.id.clone(),
                webview_label: tab.webview_label.clone(),
                generation: tab.generation,
            }
        };
        self.complete_creation(profile_id, &reservation, succeeded)
            .then_some(())
            .ok_or_else(|| "The tab creation reservation is stale.".to_owned())
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

    pub fn navigation_mut_generation(
        &mut self,
        profile_id: &str,
        tab_id: &str,
        generation: u64,
    ) -> Option<&mut TabNavigation> {
        let tab = self.open_tab_mut(profile_id, tab_id).ok()?;
        (tab.generation == generation
            && matches!(tab.lifecycle, TabLifecycle::Creating | TabLifecycle::Ready))
        .then_some(&mut tab.navigation)
    }

    pub fn set_title_generation(
        &mut self,
        profile_id: &str,
        tab_id: &str,
        generation: u64,
        title: String,
    ) -> bool {
        let Some(tab) = self
            .profiles
            .get_mut(profile_id)
            .filter(|profile| profile.lifecycle == ProfileLifecycle::Open)
            .and_then(|profile| profile.tabs.get_mut(tab_id))
        else {
            return false;
        };
        if tab.generation != generation
            || !matches!(tab.lifecycle, TabLifecycle::Creating | TabLifecycle::Ready)
        {
            return false;
        }
        tab.title = Some(title);
        true
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

fn snapshot(tab: &TabState) -> TabSnapshot {
    TabSnapshot {
        id: tab.id.clone(),
        webview_label: tab.webview_label.clone(),
        generation: tab.generation,
        title: tab.title.clone(),
        favicon_uri: tab.favicon_uri.clone(),
        current_url: tab.navigation.current_url.clone(),
        lifecycle: match tab.lifecycle {
            TabLifecycle::Empty => "empty",
            TabLifecycle::Creating => "creating",
            TabLifecycle::Ready => "ready",
            TabLifecycle::Closing => "closing",
        }
        .into(),
        loading: tab.navigation.loading,
        error: tab.navigation.error.clone(),
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

    fn add(registry: &mut BrowserRegistry, profile: &str) -> String {
        registry.add_tab_generated(profile).unwrap()
    }

    #[test]
    fn ids_are_unique_and_opening_creates_one_initial_tab_idempotently() {
        let mut registry = BrowserRegistry::default();
        let first = open(&mut registry, "alpha");
        assert_eq!(open(&mut registry, "alpha"), first);
        let second = add(&mut registry, "alpha");
        assert_ne!(first, second);
        assert_eq!(registry.profile("alpha").unwrap().ordered_tab_ids.len(), 2);
    }

    #[test]
    fn generated_profiles_and_tabs_have_collision_checked_native_labels() {
        let mut registry = BrowserRegistry::default();
        let alpha = registry
            .open_profile_generated("alpha", "browser-alpha".into(), "chrome-alpha".into())
            .unwrap();
        let beta = registry
            .open_profile_generated("beta", "browser-beta".into(), "chrome-beta".into())
            .unwrap();
        let alpha_label = registry.tab_label("alpha", &alpha).unwrap();
        let beta_label = registry.tab_label("beta", &beta).unwrap();
        assert_ne!(alpha, beta);
        assert_ne!(alpha_label, beta_label);
        let added = registry.add_tab_generated("alpha").unwrap();
        assert_ne!(registry.tab_label("alpha", &added).unwrap(), alpha_label);
        assert_eq!(
            registry.profile_tabs_snapshot("alpha").unwrap().profile_id,
            "alpha"
        );
        assert_eq!(
            registry.profile_tabs_snapshot("beta").unwrap().profile_id,
            "beta"
        );
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
        let second = add(&mut registry, "alpha");
        let third = add(&mut registry, "alpha");
        registry.select_tab("alpha", &second).unwrap();
        assert_eq!(registry.close_tab("alpha", &first).unwrap(), second);
        assert_eq!(registry.close_tab("alpha", &second).unwrap(), third);
        assert_eq!(registry.profile("alpha").unwrap().active_tab_id, third);
    }

    #[test]
    fn each_tab_keeps_independent_navigation_and_stale_closed_tab_callbacks_are_ignored() {
        let mut registry = BrowserRegistry::default();
        let first = open(&mut registry, "alpha");
        let second = add(&mut registry, "alpha");
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
        let second = add(&mut registry, "alpha");
        registry.select_tab("alpha", &second).unwrap();
        assert_eq!(registry.close_tab("alpha", &first).unwrap(), second);
        assert_eq!(registry.close_tab("alpha", &second).unwrap(), second);
        assert_eq!(
            registry.profile("alpha").unwrap().tabs[&second].lifecycle,
            TabLifecycle::Empty
        );
    }

    #[test]
    fn closing_final_tab_invalidates_callbacks_from_its_previous_webview() {
        let mut registry = BrowserRegistry::default();
        let tab = open(&mut registry, "alpha");
        let creation = registry.reserve_creation("alpha", &tab).unwrap();
        assert!(registry.complete_creation("alpha", &creation, true));
        registry.close_tab("alpha", &tab).unwrap();
        assert!(registry
            .navigation_mut_generation("alpha", &tab, creation.generation)
            .is_none());
        assert!(!registry.set_title_generation("alpha", &tab, creation.generation, "stale".into()));
        assert_eq!(
            registry.tab_snapshot("alpha", &tab).unwrap().current_url,
            None
        );
    }

    #[test]
    fn lifecycle_transitions_reject_duplicates_and_failed_creation_can_retry() {
        let mut registry = BrowserRegistry::default();
        let tab = open(&mut registry, "alpha");
        let failed = registry.reserve_creation("alpha", &tab).unwrap();
        assert!(registry.reserve_creation("alpha", &tab).is_err());
        assert!(registry.complete_creation("alpha", &failed, false));
        let retry = registry.reserve_creation("alpha", &tab).unwrap();
        assert_ne!(failed.generation, retry.generation);
        assert!(!registry.complete_creation("alpha", &failed, true));
        assert!(registry.complete_creation("alpha", &retry, true));
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
        assert!(registry.add_tab_generated("alpha").is_err());
        registry.remove_profile("alpha");
        assert!(registry.profile("beta").is_some());
        assert!(registry.add_tab_generated("beta").is_ok());
    }

    #[test]
    fn profile_closing_during_creation_rejects_stale_completion() {
        let mut registry = BrowserRegistry::default();
        let tab = open(&mut registry, "alpha");
        let reservation = registry.reserve_creation("alpha", &tab).unwrap();
        registry.begin_profile_close("alpha").unwrap();
        assert!(!registry.complete_creation("alpha", &reservation, true));
    }

    #[test]
    fn stale_generation_callbacks_cannot_update_retried_tab_navigation_or_title() {
        let mut registry = BrowserRegistry::default();
        let tab = open(&mut registry, "alpha");
        let first = registry.reserve_creation("alpha", &tab).unwrap();
        assert!(registry.complete_creation("alpha", &first, false));
        let retry = registry.reserve_creation("alpha", &tab).unwrap();
        assert!(registry.complete_creation("alpha", &retry, true));

        assert!(registry
            .navigation_mut_generation("alpha", &tab, first.generation)
            .is_none());
        assert!(!registry.set_title_generation(
            "alpha",
            &tab,
            first.generation,
            "Stale title".into()
        ));
        assert!(registry
            .navigation_mut_generation("alpha", &tab, retry.generation)
            .is_some());
        assert!(registry.set_title_generation(
            "alpha",
            &tab,
            retry.generation,
            "Current title".into()
        ));
        assert_eq!(
            registry.profile("alpha").unwrap().tabs[&tab]
                .title
                .as_deref(),
            Some("Current title")
        );
    }
}
