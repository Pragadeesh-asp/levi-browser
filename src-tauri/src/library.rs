//! Per-Space browsing library: history, bookmarks and the tabs of the last session.
//!
//! One JSON file per Space under `<app local data>/library/`, written atomically
//! (temp file, then rename). Only http(s) pages are recorded; nothing leaves the machine.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const HISTORY_LIMIT: usize = 2000;
const SESSION_LIMIT: usize = 40;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    /// Unix seconds.
    pub visited_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub added_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    pub history: Vec<HistoryEntry>,
    pub bookmarks: Vec<Bookmark>,
    pub session: Vec<String>,
}

pub struct LibraryStore {
    dir: PathBuf,
    cache: Mutex<HashMap<String, Library>>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn recordable(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

impl LibraryStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, cache: Mutex::new(HashMap::new()) }
    }

    fn path(&self, profile_id: &str) -> PathBuf {
        self.dir.join(format!("{profile_id}.json"))
    }

    fn with<R>(&self, profile_id: &str, save: bool, change: impl FnOnce(&mut Library) -> R) -> Result<R, String> {
        let mut cache = self.cache.lock().map_err(|_| "The browsing library is unavailable.".to_owned())?;
        if !cache.contains_key(profile_id) {
            let loaded = fs::read_to_string(self.path(profile_id))
                .ok()
                .and_then(|text| serde_json::from_str::<Library>(&text).ok())
                .unwrap_or_default();
            cache.insert(profile_id.to_owned(), loaded);
        }
        let library = cache.get_mut(profile_id).expect("library was just inserted");
        let result = change(library);
        if save {
            fs::create_dir_all(&self.dir).map_err(|e| format!("Could not create the library folder: {e}"))?;
            let text = serde_json::to_string(library).map_err(|e| e.to_string())?;
            let target = self.path(profile_id);
            let temp = target.with_extension("json.tmp");
            fs::write(&temp, text).map_err(|e| format!("Could not save the browsing library: {e}"))?;
            fs::rename(&temp, &target).map_err(|e| format!("Could not save the browsing library: {e}"))?;
        }
        Ok(result)
    }

    /// Records a finished page load. A reload of the same page doesn't add a second row.
    pub fn record_visit(&self, profile_id: &str, url: &str) -> Result<(), String> {
        if !recordable(url) {
            return Ok(());
        }
        self.with(profile_id, true, |library| {
            if let Some(last) = library.history.last_mut() {
                if last.url == url {
                    last.visited_at = now();
                    return;
                }
            }
            library.history.push(HistoryEntry { url: url.to_owned(), title: String::new(), visited_at: now() });
            let excess = library.history.len().saturating_sub(HISTORY_LIMIT);
            library.history.drain(..excess);
        })
    }

    /// Gives the most recent visit of `url` its page title (titles arrive after the load).
    pub fn set_title(&self, profile_id: &str, url: &str, title: &str) -> Result<(), String> {
        if !recordable(url) || title.trim().is_empty() {
            return Ok(());
        }
        self.with(profile_id, true, |library| {
            if let Some(entry) = library.history.iter_mut().rev().find(|entry| entry.url == url) {
                entry.title = title.trim().to_owned();
            }
            for bookmark in library.bookmarks.iter_mut().filter(|b| b.url == url && b.title.is_empty()) {
                bookmark.title = title.trim().to_owned();
            }
        })
    }

    /// Newest first; `query` matches the title or address, case-insensitively.
    pub fn history(&self, profile_id: &str, query: &str, limit: usize) -> Result<Vec<HistoryEntry>, String> {
        let needle = query.trim().to_lowercase();
        self.with(profile_id, false, |library| {
            library
                .history
                .iter()
                .rev()
                .filter(|entry| needle.is_empty() || entry.url.to_lowercase().contains(&needle) || entry.title.to_lowercase().contains(&needle))
                .take(limit)
                .cloned()
                .collect()
        })
    }

    pub fn clear_history(&self, profile_id: &str) -> Result<(), String> {
        self.with(profile_id, true, |library| library.history.clear())
    }

    /// Adds or removes a bookmark; returns whether the page is now bookmarked.
    pub fn toggle_bookmark(&self, profile_id: &str, url: &str, title: &str) -> Result<bool, String> {
        if !recordable(url) {
            return Err("Only web pages can be bookmarked.".into());
        }
        self.with(profile_id, true, |library| {
            if let Some(position) = library.bookmarks.iter().position(|b| b.url == url) {
                library.bookmarks.remove(position);
                false
            } else {
                library.bookmarks.push(Bookmark { url: url.to_owned(), title: title.trim().to_owned(), added_at: now() });
                true
            }
        })
    }

    pub fn is_bookmarked(&self, profile_id: &str, url: &str) -> Result<bool, String> {
        self.with(profile_id, false, |library| library.bookmarks.iter().any(|b| b.url == url))
    }

    /// Newest first.
    pub fn bookmarks(&self, profile_id: &str) -> Result<Vec<Bookmark>, String> {
        self.with(profile_id, false, |library| library.bookmarks.iter().rev().cloned().collect())
    }

    /// Remembers the open tabs. An empty set never replaces a saved session, so closing
    /// the window (which closes tabs one by one) can't erase what should be restored.
    pub fn set_session(&self, profile_id: &str, urls: &[String]) -> Result<(), String> {
        let pages: Vec<String> = urls.iter().filter(|url| recordable(url)).take(SESSION_LIMIT).cloned().collect();
        if pages.is_empty() {
            return Ok(());
        }
        let unchanged = self.with(profile_id, false, |library| library.session == pages)?;
        if unchanged {
            return Ok(());
        }
        self.with(profile_id, true, |library| library.session = pages)
    }

    pub fn session(&self, profile_id: &str) -> Result<Vec<String>, String> {
        self.with(profile_id, false, |library| library.session.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::LibraryStore;

    fn store() -> (LibraryStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("levi-library-test-{}-{}", std::process::id(), rand_suffix()));
        (LibraryStore::new(dir.clone()), dir)
    }
    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[test]
    fn records_web_pages_only_and_collapses_reloads() {
        let (library, dir) = store();
        library.record_visit("alpha", "https://example.com/").unwrap();
        library.record_visit("alpha", "https://example.com/").unwrap();
        library.record_visit("alpha", "about:blank").unwrap();
        library.record_visit("alpha", "http://127.0.0.1:4173/browser-profile.html").unwrap();
        let history = library.history("alpha", "", 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].url, "https://example.com/");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn titles_arrive_later_and_search_matches_them() {
        let (library, dir) = store();
        library.record_visit("alpha", "https://docs.rs/").unwrap();
        library.set_title("alpha", "https://docs.rs/", "Docs.rs").unwrap();
        assert_eq!(library.history("alpha", "docs.RS", 10).unwrap()[0].title, "Docs.rs");
        assert!(library.history("alpha", "nothing", 10).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn spaces_are_separate_and_survive_a_restart() {
        let (library, dir) = store();
        assert!(library.toggle_bookmark("alpha", "https://a.test/", "A").unwrap());
        library.set_session("alpha", &["https://a.test/".into(), "about:blank".into()]).unwrap();
        let reopened = LibraryStore::new(dir.clone());
        assert!(reopened.is_bookmarked("alpha", "https://a.test/").unwrap());
        assert!(!reopened.is_bookmarked("beta", "https://a.test/").unwrap());
        assert_eq!(reopened.session("alpha").unwrap(), vec!["https://a.test/".to_owned()]);
        assert!(!reopened.toggle_bookmark("alpha", "https://a.test/", "A").unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_empty_tab_list_never_erases_the_saved_session() {
        let (library, dir) = store();
        library.set_session("alpha", &["https://a.test/".into(), "https://b.test/".into()]).unwrap();
        library.set_session("alpha", &[]).unwrap();
        library.set_session("alpha", &["about:blank".into()]).unwrap();
        assert_eq!(library.session("alpha").unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
