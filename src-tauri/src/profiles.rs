use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_PROFILE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub avatar: String,
    pub accent: String,
    pub created_at: String,
}

pub struct ProfileStore {
    path: PathBuf,
    profiles: Mutex<Vec<BrowserProfile>>,
}

impl ProfileStore {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not open the profile catalog folder: {error}"))?;
        }

        let mut profiles = Vec::new();
        if path.exists() {
            let file = fs::File::open(&path)
                .map_err(|error| format!("Could not read the profile catalog: {error}"))?;
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if let Ok(snapshot) = serde_json::from_str::<Vec<BrowserProfile>>(&line) {
                    profiles = validate_catalog(snapshot);
                }
            }
        }

        if profiles.is_empty() {
            profiles = default_profiles();
        } else {
            ensure_legacy_profiles(&mut profiles);
        }

        Ok(Self {
            path,
            profiles: Mutex::new(profiles),
        })
    }

    fn append_snapshot(&self, profiles: &[BrowserProfile]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| format!("Could not save profile settings: {error}"))?;
        serde_json::to_writer(&mut file, profiles)
            .map_err(|error| format!("Could not serialize profile settings: {error}"))?;
        file.write_all(b"\n")
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("Could not persist profile settings: {error}"))
    }

    pub fn list(&self) -> Result<Vec<BrowserProfile>, String> {
        self.profiles
            .lock()
            .map(|items| items.clone())
            .map_err(|_| "The profile catalog is unavailable.".into())
    }

    pub fn get(&self, id: &str) -> Result<BrowserProfile, String> {
        self.profiles
            .lock()
            .map_err(|_| "The profile catalog is unavailable.".to_owned())?
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or_else(|| "That browser profile does not exist.".into())
    }

    pub fn create(&self, name: &str, avatar: &str, accent: &str) -> Result<BrowserProfile, String> {
        let name = validate_name(name)?;
        let avatar = validate_avatar(avatar)?;
        validate_accent(accent)?;

        let mut current = self
            .profiles
            .lock()
            .map_err(|_| "The profile catalog is unavailable.".to_owned())?;
        if current
            .iter()
            .any(|profile| profile.name.eq_ignore_ascii_case(&name))
        {
            return Err("A profile with that name already exists.".into());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System time is unavailable.".to_owned())?
            .as_nanos();
        let profile = BrowserProfile {
            id: format!(
                "p-{now:x}-{:x}",
                NEXT_PROFILE_ID.fetch_add(1, Ordering::Relaxed)
            ),
            name,
            avatar,
            accent: accent.to_ascii_uppercase(),
            created_at: now.to_string(),
        };
        let mut next = current.clone();
        next.push(profile.clone());
        self.append_snapshot(&next)?;
        *current = next;
        Ok(profile)
    }

    pub fn update(
        &self,
        id: &str,
        name: &str,
        avatar: &str,
        accent: &str,
    ) -> Result<BrowserProfile, String> {
        let name = validate_name(name)?;
        let avatar = validate_avatar(avatar)?;
        validate_accent(accent)?;

        let mut current = self
            .profiles
            .lock()
            .map_err(|_| "The profile catalog is unavailable.".to_owned())?;
        if current
            .iter()
            .any(|profile| profile.id != id && profile.name.eq_ignore_ascii_case(&name))
        {
            return Err("A profile with that name already exists.".into());
        }

        let mut next = current.clone();
        let profile = next
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or_else(|| "That browser profile does not exist.".to_owned())?;
        profile.name = name;
        profile.avatar = avatar;
        profile.accent = accent.to_ascii_uppercase();
        let result = profile.clone();
        self.append_snapshot(&next)?;
        *current = next;
        Ok(result)
    }
}

fn default_profiles() -> Vec<BrowserProfile> {
    vec![
        BrowserProfile {
            id: "alpha".into(),
            name: "Profile Alpha".into(),
            avatar: "A".into(),
            accent: "#C7F000".into(),
            created_at: "legacy".into(),
        },
        BrowserProfile {
            id: "beta".into(),
            name: "Profile Beta".into(),
            avatar: "B".into(),
            accent: "#F4F4F0".into(),
            created_at: "legacy".into(),
        },
    ]
}

fn validate_catalog(profiles: Vec<BrowserProfile>) -> Vec<BrowserProfile> {
    let mut valid = Vec::<BrowserProfile>::new();
    for profile in profiles {
        if !valid_profile_id(&profile.id)
            || validate_name(&profile.name).is_err()
            || validate_avatar(&profile.avatar).is_err()
            || validate_accent(&profile.accent).is_err()
            || valid.iter().any(|existing| {
                existing.id == profile.id || existing.name.eq_ignore_ascii_case(&profile.name)
            })
        {
            continue;
        }
        valid.push(profile);
    }
    valid
}

fn ensure_legacy_profiles(profiles: &mut Vec<BrowserProfile>) {
    for default in default_profiles() {
        if !profiles.iter().any(|profile| profile.id == default.id) {
            profiles.insert(
                usize::from(default.id == "beta").min(profiles.len()),
                default,
            );
        }
    }
}

fn valid_profile_id(id: &str) -> bool {
    if matches!(id, "alpha" | "beta") {
        return true;
    }
    let Some(rest) = id.strip_prefix("p-") else {
        return false;
    };
    let Some((timestamp, sequence)) = rest.split_once('-') else {
        return false;
    };
    !timestamp.is_empty()
        && !sequence.is_empty()
        && timestamp.len() <= 32
        && sequence.len() <= 32
        && timestamp
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        && sequence
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn validate_name(input: &str) -> Result<String, String> {
    let name = input.trim();
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
        return Err("Profile names must contain 1–40 visible characters.".into());
    }
    Ok(name.to_owned())
}

fn validate_avatar(input: &str) -> Result<String, String> {
    let avatar = input.trim();
    if avatar.is_empty() || avatar.chars().count() > 3 || avatar.chars().any(char::is_control) {
        return Err("Choose an avatar initial or symbol (1–3 characters).".into());
    }
    Ok(avatar.to_owned())
}

fn validate_accent(input: &str) -> Result<(), String> {
    let Some(hex) = input.strip_prefix('#') else {
        return Err("Accent must be a six-digit hex color such as #C7F000.".into());
    };
    if hex.len() != 6 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err("Accent must be a six-digit hex color such as #C7F000.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn store() -> (ProfileStore, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "levi-profile-catalog-{}.jsonl",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        (ProfileStore::load(path.clone()).unwrap(), path)
    }

    #[test]
    fn existing_alpha_beta_are_seeded_and_paths_remain_stable_ids() {
        let (store, _) = store();
        let rows = store.list().unwrap();
        assert_eq!(
            rows.iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
    }

    #[test]
    fn creates_valid_profiles_and_rejects_duplicate_or_invalid_values() {
        let (store, _) = store();
        let profile = store.create("Studio", "S", "#C7F000").unwrap();
        assert!(profile.id.starts_with("p-"));
        assert_eq!(store.list().unwrap().len(), 3);
        assert!(store.create("studio", "X", "#FFFFFF").is_err());
        assert!(store.create(" ", "X", "#FFFFFF").is_err());
        assert!(store.create("Valid", "X", "red").is_err());
    }

    #[test]
    fn updates_are_persisted_and_loaded_without_changing_ids() {
        let (store, path) = store();
        let profile = store.create("Studio", "S", "#C7F000").unwrap();
        store
            .update(&profile.id, "Studio Two", "2", "#F4F4F0")
            .unwrap();
        let loaded = ProfileStore::load(path.clone()).unwrap();
        assert_eq!(loaded.get(&profile.id).unwrap().name, "Studio Two");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_malformed_persisted_ids_instead_of_accepting_path_segments() {
        let valid = BrowserProfile {
            id: "p-abcdef-1".into(),
            name: "Good".into(),
            avatar: "G".into(),
            accent: "#C7F000".into(),
            created_at: "1".into(),
        };
        let mut traversal = valid.clone();
        traversal.id = "p-../../outside".into();
        let result = validate_catalog(vec![traversal, valid.clone()]);
        assert_eq!(result, vec![valid]);
    }
}
