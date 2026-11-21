//! LEVI ecosystem bridge, Browser side.
//!
//! - Publishes `%LOCALAPPDATA%\LEVI\ecosystem\browser.json` so other LEVI apps can
//!   find this executable and its Spaces (profiles). Written atomically at startup
//!   and whenever a profile is created or renamed.
//! - Accepts `--space <id> --open <url>` on the command line. Only local processes
//!   can pass arguments, so no website can trigger it. A second launch is forwarded
//!   to the running instance by tauri-plugin-single-instance, and the URL waits in
//!   [`LaunchQueue`] until that Space's chrome takes it.

use crate::profiles::BrowserProfile;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

pub const MANIFEST_SCHEMA: u32 = 1;
pub const MANIFEST_FILE: &str = "browser.json";
const MAX_URL_LEN: usize = 2048;
const MAX_QUEUED_PER_SPACE: usize = 20;

/// `%LOCALAPPDATA%\LEVI\ecosystem`, shared by every LEVI desktop app.
pub fn manifest_dir(local_data_dir: &Path) -> PathBuf {
    local_data_dir.join("LEVI").join("ecosystem")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    pub space: String,
    pub url: Url,
}

fn valid_space_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

/// Accepts only plain `http`/`https` page addresses without embedded credentials.
pub fn validate_launch_url(raw: &str) -> Result<Url, String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > MAX_URL_LEN {
        return Err("The page address is empty or too long.".into());
    }
    let url = Url::parse(raw).map_err(|_| "The page address is malformed.".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Only http and https page addresses can be opened.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Page addresses cannot contain a user name or password.".into());
    }
    Ok(url)
}

/// Parses `--space <id> --open <url>` (also `--space=<id>` and `--open=<url>`).
/// `args` is a full argv: the first item, the program path, is skipped. Returns
/// `Ok(None)` when neither flag is present; other arguments are ignored.
pub fn parse_launch_args<I, S>(args: I) -> Result<Option<LaunchRequest>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args
        .into_iter()
        .skip(1)
        .map(|arg| arg.as_ref().to_owned())
        .collect();
    let mut space = None;
    let mut open = None;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag == "--space" || flag == "--open" => (flag, Some(value)),
            _ => (arg, None),
        };
        if flag == "--space" || flag == "--open" {
            let value = match inline {
                Some(value) => value.to_owned(),
                None => {
                    index += 1;
                    args.get(index)
                        .cloned()
                        .ok_or_else(|| format!("{flag} needs a value."))?
                }
            };
            if flag == "--space" {
                space = Some(value);
            } else {
                open = Some(value);
            }
        }
        index += 1;
    }
    match (space, open) {
        (None, None) => Ok(None),
        (Some(space), Some(open)) => {
            if !valid_space_id(&space) {
                return Err("That Space identifier is invalid.".into());
            }
            Ok(Some(LaunchRequest {
                space,
                url: validate_launch_url(&open)?,
            }))
        }
        _ => Err("--space and --open must be used together.".into()),
    }
}

/// Pages waiting to open, per Space, until that Space's chrome takes them.
#[derive(Default)]
pub struct LaunchQueue(Mutex<HashMap<String, Vec<String>>>);

impl LaunchQueue {
    pub fn push(&self, request: &LaunchRequest) -> Result<(), String> {
        let mut queue = self
            .0
            .lock()
            .map_err(|_| "The launch queue is unavailable.".to_owned())?;
        let pending = queue.entry(request.space.clone()).or_default();
        if pending.len() >= MAX_QUEUED_PER_SPACE {
            return Err("Too many pages are already waiting to open in this Space.".into());
        }
        pending.push(request.url.to_string());
        Ok(())
    }

    pub fn take(&self, space: &str) -> Vec<String> {
        self.0
            .lock()
            .ok()
            .and_then(|mut queue| queue.remove(space))
            .unwrap_or_default()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest<'a> {
    schema: u32,
    app: &'static str,
    name: &'static str,
    version: &'a str,
    executable: String,
    updated_at_unix: u64,
    launch: LaunchSyntax,
    spaces: Vec<ManifestSpace<'a>>,
}

#[derive(Serialize)]
struct LaunchSyntax {
    space: &'static str,
    open: &'static str,
}

#[derive(Serialize)]
struct ManifestSpace<'a> {
    id: &'a str,
    name: &'a str,
    avatar: &'a str,
    accent: &'a str,
}

/// Writes `browser.json` atomically (temporary file, then rename).
pub fn write_manifest(
    dir: &Path,
    version: &str,
    executable: &Path,
    profiles: &[BrowserProfile],
) -> Result<PathBuf, String> {
    fs::create_dir_all(dir)
        .map_err(|error| format!("Could not create {}: {error}", dir.display()))?;
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        app: "browser",
        name: "LEVI Browser",
        version,
        executable: executable.display().to_string(),
        updated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default(),
        launch: LaunchSyntax {
            space: "--space",
            open: "--open",
        },
        spaces: profiles
            .iter()
            .map(|profile| ManifestSpace {
                id: &profile.id,
                name: &profile.name,
                avatar: &profile.avatar,
                accent: &profile.accent,
            })
            .collect(),
    };
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("Could not serialize the ecosystem manifest: {error}"))?;
    let target = dir.join(MANIFEST_FILE);
    let temporary = dir.join(format!("{MANIFEST_FILE}.tmp"));
    fs::write(&temporary, json)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, &target)
        .map_err(|error| format!("Could not replace {}: {error}", target.display()))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "C:\\LEVI\\levi-browser.exe";

    #[test]
    fn no_flags_means_no_request() {
        assert_eq!(parse_launch_args([EXE]).unwrap(), None);
        assert_eq!(parse_launch_args([EXE, "--verbose"]).unwrap(), None);
    }

    #[test]
    fn reads_space_and_page_in_both_spellings() {
        let request = parse_launch_args([
            EXE,
            "--space",
            "alpha",
            "--open",
            "https://claude.ai/chat/abc",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(request.space, "alpha");
        assert_eq!(request.url.as_str(), "https://claude.ai/chat/abc");
        let inline = parse_launch_args([
            EXE,
            "--open=http://localhost:3000/dashboard",
            "--space=p-18f2a-1",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            (inline.space.as_str(), inline.url.port()),
            ("p-18f2a-1", Some(3000))
        );
    }

    #[test]
    fn rejects_incomplete_or_unsafe_requests() {
        assert!(parse_launch_args([EXE, "--space", "alpha"]).is_err());
        assert!(parse_launch_args([EXE, "--open", "https://example.com"]).is_err());
        assert!(parse_launch_args([EXE, "--space"]).is_err());
        assert!(
            parse_launch_args([EXE, "--space", "Alpha!", "--open", "https://example.com"]).is_err()
        );
        for page in [
            "javascript:alert(1)",
            "file:///C:/Windows/win.ini",
            "levi-browser://open",
            "https://user:secret@example.com",
            "not a url",
        ] {
            assert!(
                parse_launch_args([EXE, "--space", "alpha", "--open", page]).is_err(),
                "{page} must be rejected"
            );
        }
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL_LEN));
        assert!(validate_launch_url(&long).is_err());
    }

    #[test]
    fn queue_hands_each_page_to_its_own_space_once() {
        let queue = LaunchQueue::default();
        let alpha = LaunchRequest {
            space: "alpha".into(),
            url: Url::parse("https://example.com/a").unwrap(),
        };
        let beta = LaunchRequest {
            space: "beta".into(),
            url: Url::parse("https://example.com/b").unwrap(),
        };
        queue.push(&alpha).unwrap();
        queue.push(&beta).unwrap();
        assert_eq!(queue.take("alpha"), ["https://example.com/a"]);
        assert!(queue.take("alpha").is_empty());
        assert_eq!(queue.take("beta"), ["https://example.com/b"]);
        for _ in 0..MAX_QUEUED_PER_SPACE {
            queue.push(&alpha).unwrap();
        }
        assert!(queue.push(&alpha).is_err());
    }

    #[test]
    fn manifest_lists_spaces_and_replaces_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "levi-ecosystem-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profiles = vec![BrowserProfile {
            id: "alpha".into(),
            name: "Claude · Nova".into(),
            avatar: "N".into(),
            accent: "#E3C36B".into(),
            created_at: "legacy".into(),
        }];
        let path = write_manifest(&dir, "0.1.0", Path::new(EXE), &profiles).unwrap();
        write_manifest(&dir, "0.1.1", Path::new(EXE), &profiles).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["schema"], 1);
        assert_eq!(value["app"], "browser");
        assert_eq!(value["version"], "0.1.1");
        assert_eq!(value["executable"], EXE);
        assert_eq!(value["launch"]["open"], "--open");
        assert_eq!(value["spaces"][0]["id"], "alpha");
        assert_eq!(value["spaces"][0]["name"], "Claude · Nova");
        assert!(!dir.join(format!("{MANIFEST_FILE}.tmp")).exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
