#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::{collections::HashSet, fs, path::PathBuf, sync::Mutex};
use tauri::webview::NewWindowResponse;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    webview::WebviewBuilder,
    AppHandle, Emitter, Manager, Webview, WebviewUrl, WindowBuilder,
};
use url::Url;

#[allow(dead_code)] // Registry operations are intentionally model-only until Mission 002B.1b.
mod tabs;
use tabs::BrowserRegistry;
mod profiles;
use profiles::{BrowserProfile, ProfileStore};

const CHROME_HEIGHT: f64 = 132.0;
const INITIAL_WIDTH: f64 = 1120.0;
const INITIAL_HEIGHT: f64 = 760.0;

fn navigation_log(event: &str, profile_id: &str, details: &str) {
    // Keep navigation logs useful without recording URL paths, queries, or page content.
    eprintln!("[LEVI_DIAG] event={event} profile={profile_id} {details}");
}

fn url_log_fields(url: &Url) -> String {
    format!(
        "scheme={} host={}",
        url.scheme(),
        url.host_str().unwrap_or("unknown")
    )
}

#[derive(Default)]
struct NavigationStore {
    registry: Mutex<BrowserRegistry>,
    closing_profiles: Mutex<std::collections::HashSet<String>>,
    resize_profiles: Mutex<HashSet<String>>,
    native_ops: tokio::sync::Mutex<()>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct NavigationState {
    current_url: Option<String>,
    entries: Vec<String>,
    index: Option<usize>,
    is_loading: bool,
    error: Option<String>,
    pending_history_index: Option<usize>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileNavigationEvent<'a> {
    profile_id: &'a str,
    state: &'a NavigationState,
}

impl From<tabs::TabNavigation> for NavigationState {
    fn from(value: tabs::TabNavigation) -> Self {
        Self {
            current_url: value.current_url,
            entries: value.entries,
            index: value.index,
            is_loading: value.loading,
            error: value.error,
            pending_history_index: value.pending_history_index,
        }
    }
}

impl NavigationStore {
    fn reset_profile(&self, profile_id: &str) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.remove_profile(profile_id);
        }
    }

    fn state_or_default(&self, profile_id: &str) -> NavigationState {
        self.registry
            .lock()
            .ok()
            .and_then(|registry| registry.active_navigation(profile_id).cloned())
            .map(NavigationState::from)
            .unwrap_or_default()
    }

    fn ensure_profile(&self, profile_id: &str) -> Result<(), String> {
        let (window, chrome, _) = browser_labels(profile_id)?;
        self.registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
            .open_profile_generated(profile_id, window, chrome)?;
        Ok(())
    }

    #[cfg(test)]
    fn with_active_navigation<R>(
        &self,
        profile_id: &str,
        update: impl FnOnce(&mut tabs::TabNavigation) -> R,
    ) -> Option<R> {
        let mut registry = self.registry.lock().ok()?;
        registry.active_navigation_mut(profile_id).map(update)
    }

    fn mark_closing(&self, profile_id: &str) -> Result<(), String> {
        if let Ok(mut registry) = self.registry.lock() {
            if registry.profile(profile_id).is_some() {
                registry.begin_profile_close(profile_id)?;
            }
        }
        self.closing_profiles
            .lock()
            .map_err(|_| "Profile lifecycle state is unavailable.".to_owned())?
            .insert(profile_id.to_owned());
        Ok(())
    }

    fn is_closing(&self, profile_id: &str) -> Result<bool, String> {
        self.closing_profiles
            .lock()
            .map(|profiles| profiles.contains(profile_id))
            .map_err(|_| "Profile lifecycle state is unavailable.".to_owned())
    }

    fn finish_close(&self, profile_id: &str) {
        // Clear navigation before opening the lifecycle gate for a future window.
        self.reset_profile(profile_id);
        if let Ok(mut profiles) = self.closing_profiles.lock() {
            profiles.remove(profile_id);
        }
        if let Ok(mut profiles) = self.resize_profiles.lock() {
            profiles.remove(profile_id);
        }
    }

    fn cancel_close(&self, profile_id: &str) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.cancel_profile_close(profile_id);
        }
        if let Ok(mut profiles) = self.closing_profiles.lock() {
            profiles.remove(profile_id);
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileLaunch {
    profile_id: String,
    window_label: String,
    data_directory: String,
    reused_window: bool,
}

fn profile_details(app: &AppHandle, profile_id: &str) -> Result<(String, PathBuf), String> {
    let profile = app.state::<ProfileStore>().get(profile_id)?;
    if !valid_profile_id(profile_id) {
        return Err("That browser profile identifier is invalid.".into());
    }
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Could not resolve LEVI Browser local data directory.".to_owned())?
        .join(if matches!(profile_id, "alpha" | "beta") {
            "profile-isolation-lab"
        } else {
            "profiles"
        })
        .join(&profile.id);
    Ok((format!("profile-{}", profile.id), path))
}

fn valid_profile_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn browser_labels(profile_id: &str) -> Result<(String, String, String), String> {
    if !valid_profile_id(profile_id) {
        return Err("That browser profile identifier is invalid.".into());
    }
    Ok((
        format!("browser-{profile_id}"),
        format!("chrome-{profile_id}"),
        format!("content-{profile_id}"),
    ))
}

fn require_profile_chrome(webview: &Webview, profile_id: &str) -> Result<(), String> {
    let (_, expected_chrome, _) = browser_labels(profile_id)?;
    if webview.label() == expected_chrome {
        Ok(())
    } else {
        Err("Browser controls can only be used by their matching local browser chrome.".into())
    }
}

fn require_local_shell(webview: &Webview, profile_id: Option<&str>) -> Result<(), String> {
    if webview.label() == "main"
        || profile_id
            .and_then(|id| browser_labels(id).ok())
            .is_some_and(|(_, chrome, _)| chrome == webview.label())
    {
        Ok(())
    } else {
        Err("This browser action is not available to website content.".into())
    }
}

fn emit_state(app: &AppHandle, profile_id: &str, state: &NavigationState) {
    let Ok((_, chrome_label, _)) = browser_labels(profile_id) else {
        return;
    };
    match app.emit_to(
        tauri::EventTarget::webview(chrome_label),
        "browser:navigation-state",
        ProfileNavigationEvent { profile_id, state },
    ) {
        Ok(()) => navigation_log(
            "address_state_event_emitted",
            profile_id,
            &format!(
                "has_url={} loading={} error_present={}",
                state.current_url.is_some(),
                state.is_loading,
                state.error.is_some()
            ),
        ),
        Err(error) => navigation_log(
            "address_state_event_emit_failed",
            profile_id,
            &format!("error={error}"),
        ),
    }
}

fn emit_tabs_state(app: &AppHandle, profile_id: &str) {
    let state = app.state::<NavigationStore>();
    let snapshot = state
        .registry
        .lock()
        .ok()
        .and_then(|registry| registry.profile_tabs_snapshot(profile_id).ok());
    if let Some(snapshot) = snapshot {
        if let Ok((_, chrome, _)) = browser_labels(profile_id) {
            let _ = app.emit_to(
                tauri::EventTarget::webview(chrome),
                "browser:tabs-state",
                snapshot,
            );
        }
    }
}

fn navigation_state_for(app: &AppHandle, profile_id: &str, tab_id: &str) -> NavigationState {
    app.state::<NavigationStore>()
        .registry
        .lock()
        .ok()
        .and_then(|registry| registry.navigation_snapshot(profile_id, tab_id).ok())
        .map(NavigationState::from)
        .unwrap_or_default()
}

fn set_tab_loading(
    app: &AppHandle,
    profile_id: &str,
    tab_id: &str,
    generation: u64,
    loading: bool,
) {
    let store = app.state::<NavigationStore>();
    let updated = store
        .registry
        .lock()
        .ok()
        .and_then(|mut registry| {
            registry
                .navigation_mut_generation(profile_id, tab_id, generation)
                .map(|state| state.loading = loading)
        })
        .is_some();
    if updated {
        emit_tabs_state(app, profile_id);
        let active = app
            .state::<NavigationStore>()
            .registry
            .lock()
            .ok()
            .and_then(|registry| {
                registry
                    .profile(profile_id)
                    .map(|p| p.active_tab_id.clone())
            });
        if active.as_deref() == Some(tab_id) {
            emit_state(
                app,
                profile_id,
                &navigation_state_for(app, profile_id, tab_id),
            );
        }
    }
}

fn push_tab_navigation(
    app: &AppHandle,
    profile_id: &str,
    tab_id: &str,
    generation: u64,
    url: &Url,
) {
    let value = url.to_string();
    let updated = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .ok()
        .and_then(|mut registry| {
            registry
                .navigation_mut_generation(profile_id, tab_id, generation)
                .map(|state| {
                    if let Some(history_index) = state.pending_history_index.take() {
                        state.index = Some(history_index);
                    } else if state.current_url.as_deref() != Some(&value) {
                        if let Some(index) = state.index {
                            state.entries.truncate(index + 1);
                        }
                        state.entries.push(value.clone());
                        state.index = Some(state.entries.len() - 1);
                    }
                    state.current_url = Some(value);
                    state.error = None;
                })
        })
        .is_some();
    if updated {
        emit_tabs_state(app, profile_id);
        let active = app
            .state::<NavigationStore>()
            .registry
            .lock()
            .ok()
            .and_then(|registry| {
                registry
                    .profile(profile_id)
                    .map(|p| p.active_tab_id.clone())
            });
        if active.as_deref() == Some(tab_id) {
            emit_state(
                app,
                profile_id,
                &navigation_state_for(app, profile_id, tab_id),
            );
        }
    }
}

fn navigation_state(app: &AppHandle, profile_id: &str) -> NavigationState {
    app.state::<NavigationStore>().state_or_default(profile_id)
}

fn browser_window_on_main_thread(
    app: AppHandle,
    profile_id: String,
) -> Result<ProfileLaunch, String> {
    if app.state::<NavigationStore>().is_closing(&profile_id)? {
        return Err(format!(
            "Profile {} is still closing. Wait a moment, then open it again.",
            profile_id.to_uppercase()
        ));
    }
    let (profile_label, directory) = profile_details(&app, &profile_id)?;
    let (window_label, chrome_label, _) = browser_labels(&profile_id)?;
    fs::create_dir_all(&directory)
        .map_err(|_| "Could not create the isolated profile data directory.".to_owned())?;

    if let Some(window) = app.get_window(&window_label) {
        window
            .show()
            .map_err(|_| "Could not show the existing browser window.".to_owned())?;
        window
            .set_focus()
            .map_err(|_| "Could not focus the existing browser window.".to_owned())?;
        app.state::<NavigationStore>().ensure_profile(&profile_id)?;
        register_resize_handler(&app, &profile_id)?;
        emit_tabs_state(&app, &profile_id);
        return Ok(ProfileLaunch {
            profile_id,
            window_label,
            data_directory: directory.display().to_string(),
            reused_window: true,
        });
    }

    let focus_menu_id = format!("focus-address-{profile_id}");
    let focus_item = MenuItemBuilder::new("Focus Address Bar")
        .id(&focus_menu_id)
        .accelerator("Ctrl+L")
        .build(&app)
        .map_err(|_| "Could not create the address-bar keyboard shortcut.".to_owned())?;
    let browser_menu = SubmenuBuilder::new(&app, "Browser")
        .item(&focus_item)
        .build()
        .map_err(|_| "Could not create the browser keyboard menu.".to_owned())?;
    let menu = MenuBuilder::new(&app)
        .item(&browser_menu)
        .build()
        .map_err(|_| "Could not create the browser window menu.".to_owned())?;
    let menu_chrome_label = chrome_label.clone();
    let menu_id = focus_menu_id.clone();
    let menu = menu.clone();
    let window = WindowBuilder::new(&app, &window_label)
        .title(format!("LEVI BROWSER — Profile {}", profile_id.to_uppercase()))
        .inner_size(INITIAL_WIDTH, INITIAL_HEIGHT)
        .min_inner_size(840.0, 560.0)
        .menu(menu)
        .on_menu_event(move |window, event| {
            if event.id() == menu_id {
                if let Some(chrome) = window.app_handle().get_webview(&menu_chrome_label) {
                    let _ = chrome.eval("document.getElementById('address').focus(); document.getElementById('address').select();");
                }
            }
        })
        .build()
        .map_err(|_| "Could not create the native browser window.".to_owned())?;

    let lifecycle_app = app.clone();
    let lifecycle_profile = profile_id.clone();
    let lifecycle_window_label = window_label.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { .. } => {
            let _ = lifecycle_app
                .state::<NavigationStore>()
                .mark_closing(&lifecycle_profile);
        }
        tauri::WindowEvent::Destroyed => {
            lifecycle_app
                .state::<NavigationStore>()
                .finish_close(&lifecycle_profile);
            navigation_log(
                "profile_window_destroyed_state_cleared",
                &lifecycle_profile,
                &format!("window={lifecycle_window_label}"),
            );
        }
        _ => {}
    });

    #[cfg(debug_assertions)]
    let chrome_url = WebviewUrl::External(
        format!("http://127.0.0.1:4173/browser-profile.html?profile={profile_id}")
            .parse()
            .map_err(|_| "Could not construct the local browser chrome URL.".to_owned())?,
    );
    #[cfg(not(debug_assertions))]
    let chrome_url = WebviewUrl::App(format!("browser-profile.html?profile={profile_id}").into());

    window
        .add_child(
            WebviewBuilder::new(&chrome_label, chrome_url),
            tauri::LogicalPosition::new(0.0, 0.0),
            tauri::LogicalSize::new(INITIAL_WIDTH, INITIAL_HEIGHT),
        )
        .map_err(|_| "Could not create the browser address-bar surface.".to_owned())?;

    // The content webview is created only after a validated navigation request. This
    // leaves the chrome's empty state visible and keeps website content capability-free.
    let _ = profile_label;
    window
        .show()
        .map_err(|_| "Could not show the browser window.".to_owned())?;
    app.state::<NavigationStore>().ensure_profile(&profile_id)?;
    register_resize_handler(&app, &profile_id)?;
    emit_tabs_state(&app, &profile_id);

    Ok(ProfileLaunch {
        profile_id,
        window_label,
        data_directory: directory.display().to_string(),
        reused_window: false,
    })
}

#[tauri::command]
async fn open_profile(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    current_profile_id: Option<String>,
) -> Result<ProfileLaunch, String> {
    require_local_shell(&webview, current_profile_id.as_deref())?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let main_app = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(browser_window_on_main_thread(main_app, profile_id));
    })
    .map_err(|_| "Could not schedule the browser window on the Windows UI thread.".to_owned())?;
    receiver
        .await
        .map_err(|_| "The browser window operation ended before returning a result.".to_owned())?
}

fn resolve_address(input: &str, search_template: &str) -> Result<Url, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter a website address or search query.".into());
    }

    let candidate = if input.contains("://") {
        let parsed = Url::parse(input).map_err(|_| "That address is malformed.".to_owned())?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Only HTTP and HTTPS website addresses are supported.".into());
        }
        if parsed.host_str().is_none() {
            return Err("Enter a valid HTTP or HTTPS website address.".into());
        }
        return Ok(parsed);
    } else if input.chars().any(char::is_whitespace) || !looks_like_domain(input) {
        let encoded = url::form_urlencoded::byte_serialize(input.as_bytes()).collect::<String>();
        let target = search_template.replace("{query}", &encoded);
        let parsed =
            Url::parse(&target).map_err(|_| "The configured search URL is invalid.".to_owned())?;
        if parsed.scheme() != "https" || parsed.host_str().is_none() {
            return Err("The configured search URL must use a valid HTTPS address.".into());
        }
        return Ok(parsed);
    } else {
        format!("https://{input}")
    };

    let parsed =
        Url::parse(&candidate).map_err(|_| "Enter a valid domain or search query.".to_owned())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Enter a valid HTTP or HTTPS website address.".into());
    }
    Ok(parsed)
}

fn looks_like_domain(input: &str) -> bool {
    let host_or_host_path = input.split('/').next().unwrap_or_default();
    host_or_host_path == "localhost"
        || host_or_host_path.starts_with("localhost:")
        || host_or_host_path.contains('.')
        || host_or_host_path.contains(':')
        || host_or_host_path.parse::<std::net::IpAddr>().is_ok()
}

fn register_resize_handler(app: &AppHandle, profile_id: &str) -> Result<(), String> {
    let (window_label, chrome_label, _) = browser_labels(profile_id)?;
    let window = app
        .get_window(&window_label)
        .ok_or_else(|| "The profile window is unavailable.".to_owned())?;
    let inserted = app
        .state::<NavigationStore>()
        .resize_profiles
        .lock()
        .map_err(|_| "Browser resize state is unavailable.".to_owned())?
        .insert(profile_id.to_owned());
    if !inserted {
        return Ok(());
    }
    let resize_app = app.clone();
    let resize_profile = profile_id.to_owned();
    window.on_window_event(move |event| {
        let tauri::WindowEvent::Resized(physical) = event else {
            return;
        };
        let scale = resize_app
            .get_window(&window_label)
            .and_then(|win| win.scale_factor().ok())
            .unwrap_or(1.0);
        let width = physical.width as f64 / scale;
        let height = physical.height as f64 / scale;
        if let Some(chrome) = resize_app.get_webview(&chrome_label) {
            let _ = chrome.set_size(tauri::LogicalSize::new(width, height));
        }
        let labels = resize_app
            .state::<NavigationStore>()
            .registry
            .lock()
            .ok()
            .and_then(|registry| registry.profile_tabs_snapshot(&resize_profile).ok())
            .map(|snapshot| {
                snapshot
                    .tabs
                    .into_iter()
                    .map(|tab| (tab.webview_label, tab.lifecycle == "ready"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (label, ready) in labels {
            if ready {
                if let Some(content) = resize_app.get_webview(&label) {
                    let _ = content.set_position(tauri::LogicalPosition::new(0.0, CHROME_HEIGHT));
                    let _ = content.set_size(tauri::LogicalSize::new(
                        width,
                        (height - CHROME_HEIGHT).max(100.0),
                    ));
                }
            }
        }
    });
    Ok(())
}

fn ensure_content_webview(
    app: &AppHandle,
    profile_id: &str,
    tab_id: &str,
    url: Url,
) -> Result<String, String> {
    let (window_label, _, _) = browser_labels(profile_id)?;
    let (_, directory) = profile_details(app, profile_id)?;
    fs::create_dir_all(&directory)
        .map_err(|_| "Could not create the isolated profile data directory.".to_owned())?;
    let window = app
        .get_window(&window_label)
        .ok_or_else(|| "Open this isolated browser profile before navigating.".to_owned())?;
    let size = window
        .inner_size()
        .map_err(|_| "Could not read browser window size.".to_owned())?;
    let scale = window
        .scale_factor()
        .map_err(|_| "Could not read browser window scaling.".to_owned())?;
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    let reservation = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .reserve_creation(profile_id, tab_id)?;
    navigation_log(
        "content_webview_creation_requested",
        profile_id,
        &format!(
            "tab={} label={} {}",
            reservation.tab_id,
            reservation.webview_label,
            url_log_fields(&url)
        ),
    );
    if let Some(orphan) = app.get_webview(&reservation.webview_label) {
        let _ = orphan.close();
        let _ = app
            .state::<NavigationStore>()
            .registry
            .lock()
            .map(|mut registry| registry.complete_creation(profile_id, &reservation, false));
        return Err("A stale native tab surface was found and closed. Retry navigation.".into());
    }
    let app_navigation = app.clone();
    let profile_navigation = profile_id.to_owned();
    let tab_navigation = reservation.tab_id.clone();
    let generation_navigation = reservation.generation;
    let app_load = app.clone();
    let profile_load = profile_id.to_owned();
    let tab_load = reservation.tab_id.clone();
    let generation_load = reservation.generation;
    let app_title = app.clone();
    let profile_title = profile_id.to_owned();
    let tab_title = reservation.tab_id.clone();
    let generation_title = reservation.generation;
    let builder = WebviewBuilder::new(&reservation.webview_label, WebviewUrl::External(url))
        .data_directory(directory)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_navigation(move |target| {
            if matches!(target.scheme(), "http" | "https") {
                push_tab_navigation(
                    &app_navigation,
                    &profile_navigation,
                    &tab_navigation,
                    generation_navigation,
                    target,
                );
                true
            } else {
                let updated = app_navigation
                    .state::<NavigationStore>()
                    .registry
                    .lock()
                    .ok()
                    .is_some_and(|mut registry| {
                        if let Some(state) = registry.navigation_mut_generation(
                            &profile_navigation,
                            &tab_navigation,
                            generation_navigation,
                        ) {
                            state.error = Some(
                                "Navigation was blocked: only HTTP and HTTPS pages are allowed."
                                    .into(),
                            );
                            true
                        } else {
                            false
                        }
                    });
                if updated {
                    emit_tabs_state(&app_navigation, &profile_navigation);
                    let active = app_navigation
                        .state::<NavigationStore>()
                        .registry
                        .lock()
                        .ok()
                        .and_then(|registry| {
                            registry
                                .profile(&profile_navigation)
                                .map(|p| p.active_tab_id.clone())
                        });
                    if active.as_deref() == Some(tab_navigation.as_str()) {
                        emit_state(
                            &app_navigation,
                            &profile_navigation,
                            &navigation_state_for(
                                &app_navigation,
                                &profile_navigation,
                                &tab_navigation,
                            ),
                        );
                    }
                }
                false
            }
        })
        .on_page_load(move |_webview, payload| {
            let loading = matches!(payload.event(), tauri::webview::PageLoadEvent::Started);
            set_tab_loading(
                &app_load,
                &profile_load,
                &tab_load,
                generation_load,
                loading,
            );
            if !loading {
                if let Ok(target) = payload.url().to_string().parse::<Url>() {
                    push_tab_navigation(
                        &app_load,
                        &profile_load,
                        &tab_load,
                        generation_load,
                        &target,
                    );
                }
            }
        })
        .on_document_title_changed(move |_webview, title| {
            let updated = app_title
                .state::<NavigationStore>()
                .registry
                .lock()
                .ok()
                .is_some_and(|mut registry| {
                    registry.set_title_generation(
                        &profile_title,
                        &tab_title,
                        generation_title,
                        title,
                    )
                });
            if updated {
                emit_tabs_state(&app_title, &profile_title);
            }
        });
    if let Err(error) = window.add_child(
        builder,
        tauri::LogicalPosition::new(0.0, CHROME_HEIGHT),
        tauri::LogicalSize::new(width, (height - CHROME_HEIGHT).max(100.0)),
    ) {
        let _ = app
            .state::<NavigationStore>()
            .registry
            .lock()
            .map(|mut registry| registry.complete_creation(profile_id, &reservation, false));
        if let Some(orphan) = app.get_webview(&reservation.webview_label) {
            let _ = orphan.close();
        }
        navigation_log(
            "content_webview_creation_failed",
            profile_id,
            &format!("stage=add_child error={error}"),
        );
        return Err("Could not create the isolated website WebView2 surface.".to_owned());
    }
    let Some(content) = app.get_webview(&reservation.webview_label) else {
        let _ = app
            .state::<NavigationStore>()
            .registry
            .lock()
            .map(|mut registry| registry.complete_creation(profile_id, &reservation, false));
        return Err("The native tab was not registered after creation. Retry navigation.".into());
    };
    let active_id = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .ok()
        .and_then(|registry| {
            registry
                .profile_tabs_snapshot(profile_id)
                .ok()
                .map(|snapshot| snapshot.active_tab_id)
        });
    if active_id.as_deref() != Some(tab_id) {
        if let Err(error) = content.hide() {
            let _ = content.close();
            let _ = app
                .state::<NavigationStore>()
                .registry
                .lock()
                .map(|mut registry| registry.complete_creation(profile_id, &reservation, false));
            return Err(format!(
                "The inactive website surface could not be hidden: {error}"
            ));
        }
    } else if let Err(error) = content.show() {
        let _ = content.close();
        let _ = app
            .state::<NavigationStore>()
            .registry
            .lock()
            .map(|mut registry| registry.complete_creation(profile_id, &reservation, false));
        return Err(format!("The website surface could not be shown: {error}"));
    }
    let completed = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .complete_creation(profile_id, &reservation, true);
    if !completed {
        if let Some(orphan) = app.get_webview(&reservation.webview_label) {
            let _ = orphan.close();
        }
        return Err("The tab closed before its website surface finished creating.".into());
    }
    register_resize_handler(app, profile_id)?;
    emit_tabs_state(app, profile_id);
    navigation_log(
        "content_webview_creation_succeeded",
        profile_id,
        &format!("tab={} label={}", tab_id, reservation.webview_label),
    );
    Ok(reservation.webview_label)
}

#[tauri::command]
async fn browser_navigate(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    tab_id: String,
    input: String,
    search_template: String,
) -> Result<String, String> {
    navigation_log(
        "navigation_command_invoked",
        &profile_id,
        "source=browser_chrome",
    );
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let target = resolve_address(&input, &search_template).map_err(|error| {
        navigation_log(
            "url_validation_failed",
            &profile_id,
            &format!("error={error}"),
        );
        error
    })?;
    navigation_log(
        "url_validation_succeeded",
        &profile_id,
        &url_log_fields(&target),
    );
    let (label, lifecycle, is_active, generation) = {
        let store = app.state::<NavigationStore>();
        let registry = store
            .registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?;
        let profile = registry
            .profile(&profile_id)
            .ok_or_else(|| "Open this profile before navigating.".to_owned())?;
        let tab = profile
            .tabs
            .get(&tab_id)
            .ok_or_else(|| "The selected tab does not belong to this profile.".to_owned())?;
        (
            tab.webview_label.clone(),
            tab.lifecycle,
            profile.active_tab_id == tab_id,
            tab.generation,
        )
    };
    match lifecycle {
        tabs::TabLifecycle::Ready => {
            let content = app.get_webview(&label).ok_or_else(|| {
                "This tab's native website surface is unavailable. Create a new tab and retry."
                    .to_owned()
            })?;
            if is_active {
                content
                    .show()
                    .map_err(|_| "The selected website surface could not be shown.".to_owned())?;
            }
            content
                .navigate(target.clone())
                .map_err(|_| "The website navigation request could not be sent.".to_owned())?;
            set_tab_loading(&app, &profile_id, &tab_id, generation, true);
        }
        tabs::TabLifecycle::Empty => {
            ensure_content_webview(&app, &profile_id, &tab_id, target.clone())?;
        }
        tabs::TabLifecycle::Creating => {
            return Err("This tab is still opening. Wait for it to finish, then retry.".into())
        }
        tabs::TabLifecycle::Closing => {
            return Err("This tab is closing. Select another tab or retry shortly.".into())
        }
    }
    emit_tabs_state(&app, &profile_id);
    navigation_log(
        "navigation_command_completed",
        &profile_id,
        &url_log_fields(&target),
    );
    Ok(target.to_string())
}

#[tauri::command]
fn browser_state(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
) -> Result<NavigationState, String> {
    require_profile_chrome(&webview, &profile_id)?;
    Ok(navigation_state(&app, &profile_id))
}

#[tauri::command]
fn browser_tabs_state(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
) -> Result<tabs::ProfileTabsSnapshot, String> {
    require_profile_chrome(&webview, &profile_id)?;
    app.state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .profile_tabs_snapshot(&profile_id)
}

#[tauri::command]
async fn browser_new_tab(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
) -> Result<String, String> {
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let (previous_label, previous_ready) = {
        let registry = store
            .registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?;
        let profile = registry
            .profile(&profile_id)
            .ok_or_else(|| "The profile is not open.".to_owned())?;
        let previous = profile
            .tabs
            .get(&profile.active_tab_id)
            .ok_or_else(|| "The active tab is unavailable.".to_owned())?;
        (
            previous.webview_label.clone(),
            previous.lifecycle == tabs::TabLifecycle::Ready,
        )
    };
    if previous_ready {
        if let Some(previous) = app.get_webview(&previous_label) {
            previous.hide().map_err(|_| {
                "Could not hide the current tab before creating a new one.".to_owned()
            })?;
        }
    }
    let tab_result = {
        let store = app.state::<NavigationStore>();
        let mut registry = store
            .registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?;
        registry.add_tab_generated(&profile_id).and_then(|id| {
            registry.select_tab(&profile_id, &id)?;
            Ok(id)
        })
    };
    let tab_id = match tab_result {
        Ok(tab_id) => tab_id,
        Err(error) => {
            if previous_ready {
                if let Some(previous) = app.get_webview(&previous_label) {
                    let _ = previous.show();
                }
            }
            return Err(error);
        }
    };
    emit_tabs_state(&app, &profile_id);
    emit_state(
        &app,
        &profile_id,
        &navigation_state_for(&app, &profile_id, &tab_id),
    );
    Ok(tab_id)
}

#[tauri::command]
async fn browser_select_tab(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    tab_id: String,
) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let (old_id, old_label, old_ready, target_label, target_ready) = {
        let store = app.state::<NavigationStore>();
        let registry = store
            .registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?;
        let profile = registry
            .profile(&profile_id)
            .ok_or_else(|| "The profile is not open.".to_owned())?;
        let old = profile
            .tabs
            .get(&profile.active_tab_id)
            .ok_or_else(|| "The active tab is unavailable.".to_owned())?;
        let target = profile
            .tabs
            .get(&tab_id)
            .ok_or_else(|| "The tab does not belong to this profile.".to_owned())?;
        (
            old.id.clone(),
            old.webview_label.clone(),
            old.lifecycle == tabs::TabLifecycle::Ready,
            target.webview_label.clone(),
            target.lifecycle == tabs::TabLifecycle::Ready,
        )
    };
    if old_id == tab_id {
        return Ok(());
    }
    if old_ready {
        if let Some(old) = app.get_webview(&old_label) {
            old.hide()
                .map_err(|_| "Could not hide the previous tab.".to_owned())?;
        }
    }
    if target_ready {
        let Some(target) = app.get_webview(&target_label) else {
            if old_ready {
                if let Some(old) = app.get_webview(&old_label) {
                    let _ = old.show();
                }
            }
            return Err("The selected tab's website surface is unavailable.".into());
        };
        if let Err(error) = target.show() {
            if old_ready {
                if let Some(old) = app.get_webview(&old_label) {
                    let _ = old.show();
                }
            }
            return Err(format!("Could not show the selected tab: {error}"));
        }
        let _ = target.set_focus();
    }
    app.state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .select_tab(&profile_id, &tab_id)?;
    emit_tabs_state(&app, &profile_id);
    emit_state(
        &app,
        &profile_id,
        &navigation_state_for(&app, &profile_id, &tab_id),
    );
    Ok(())
}

#[tauri::command]
async fn browser_close_tab(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    tab_id: String,
) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let (
        active_id,
        is_active,
        label,
        ready,
        neighbor_id,
        neighbor_label,
        neighbor_ready,
        tab_count,
    ) = {
        let store = app.state::<NavigationStore>();
        let registry = store
            .registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?;
        let profile = registry
            .profile(&profile_id)
            .ok_or_else(|| "The profile is not open.".to_owned())?;
        let tab = profile
            .tabs
            .get(&tab_id)
            .ok_or_else(|| "The tab does not belong to this profile.".to_owned())?;
        let neighbor_id = registry.neighboring_tab(&profile_id, &tab_id)?;
        let neighbor = profile
            .tabs
            .get(&neighbor_id)
            .ok_or_else(|| "The neighboring tab is unavailable.".to_owned())?;
        (
            profile.active_tab_id.clone(),
            profile.active_tab_id == tab_id,
            tab.webview_label.clone(),
            tab.lifecycle == tabs::TabLifecycle::Ready,
            neighbor_id,
            neighbor.webview_label.clone(),
            neighbor.lifecycle == tabs::TabLifecycle::Ready,
            profile.ordered_tab_ids.len(),
        )
    };
    if is_active && ready {
        if let Some(content) = app.get_webview(&label) {
            content
                .hide()
                .map_err(|_| "Could not hide the tab being closed.".to_owned())?;
        }
    }
    if is_active && neighbor_id != tab_id && neighbor_ready {
        let Some(neighbor) = app.get_webview(&neighbor_label) else {
            if ready {
                if let Some(content) = app.get_webview(&label) {
                    let _ = content.show();
                }
            }
            return Err("The neighboring tab's website surface is unavailable.".into());
        };
        if let Err(error) = neighbor.show() {
            if ready {
                if let Some(content) = app.get_webview(&label) {
                    let _ = content.show();
                }
            }
            return Err(format!("Could not show the neighboring tab: {error}"));
        }
    }
    if let Some(content) = app.get_webview(&label) {
        if let Err(error) = content.close() {
            if is_active && neighbor_id != tab_id && neighbor_ready {
                if let Some(next) = app.get_webview(&neighbor_label) {
                    let _ = next.hide();
                }
            }
            if is_active && ready {
                if let Some(current) = app.get_webview(&label) {
                    let _ = current.show();
                }
            }
            return Err(format!("Could not close the native tab surface: {error}"));
        }
    }
    let selected = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .close_tab(&profile_id, &tab_id)?;
    if is_active && tab_count > 1 && neighbor_ready {
        if let Some(next) = app.get_webview(&neighbor_label) {
            let _ = next.set_focus();
        }
    }
    if selected == active_id || selected == tab_id {
        let state = navigation_state_for(&app, &profile_id, &selected);
        emit_state(&app, &profile_id, &state);
    } else {
        emit_state(
            &app,
            &profile_id,
            &navigation_state_for(&app, &profile_id, &selected),
        );
    }
    emit_tabs_state(&app, &profile_id);
    Ok(())
}

#[tauri::command]
async fn browser_back(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    tab_id: String,
) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let content_label = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .tab_label(&profile_id, &tab_id)?;
    let content = app
        .get_webview(&content_label)
        .ok_or_else(|| "No page is open to navigate back.".to_owned())?;
    let update = {
        let store = app.state::<NavigationStore>();
        let result = store
            .registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
            .navigation_mut(&profile_id, &tab_id)
            .map(|state| {
                let index = state
                    .index
                    .ok_or_else(|| "There is no previous page.".to_owned())?;
                if index == 0 {
                    return Err("There is no previous page.".to_owned());
                }
                state.index = Some(index - 1);
                state.pending_history_index = Some(index - 1);
                if let Some(url) = state.entries.get(index - 1) {
                    state.current_url = Some(url.clone());
                }
                Ok(())
            })
            .ok_or_else(|| "Navigation state is unavailable.".to_owned())?;
        result
    };
    update?;
    content
        .eval("history.back()")
        .map_err(|_| "Could not navigate back.".to_owned())?;
    emit_state(
        &app,
        &profile_id,
        &navigation_state_for(&app, &profile_id, &tab_id),
    );
    Ok(())
}

#[tauri::command]
async fn browser_forward(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    tab_id: String,
) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let content_label = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .tab_label(&profile_id, &tab_id)?;
    let content = app
        .get_webview(&content_label)
        .ok_or_else(|| "No page is open to navigate forward.".to_owned())?;
    let update = {
        let store = app.state::<NavigationStore>();
        let result = store
            .registry
            .lock()
            .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
            .navigation_mut(&profile_id, &tab_id)
            .map(|state| {
                let index = state
                    .index
                    .ok_or_else(|| "There is no next page.".to_owned())?;
                if index + 1 >= state.entries.len() {
                    return Err("There is no next page.".to_owned());
                }
                state.index = Some(index + 1);
                state.pending_history_index = Some(index + 1);
                if let Some(url) = state.entries.get(index + 1) {
                    state.current_url = Some(url.clone());
                }
                Ok(())
            })
            .ok_or_else(|| "Navigation state is unavailable.".to_owned())?;
        result
    };
    update?;
    content
        .eval("history.forward()")
        .map_err(|_| "Could not navigate forward.".to_owned())?;
    emit_state(
        &app,
        &profile_id,
        &navigation_state_for(&app, &profile_id, &tab_id),
    );
    Ok(())
}

#[tauri::command]
async fn browser_reload(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    tab_id: String,
) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let label = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .tab_label(&profile_id, &tab_id)?;
    app.get_webview(&label)
        .ok_or_else(|| "No page is open to reload.".to_owned())?
        .reload()
        .map_err(|_| "Could not reload the current page.".into())
}

#[tauri::command]
async fn browser_stop(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    tab_id: String,
) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let label = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .tab_label(&profile_id, &tab_id)?;
    app.get_webview(&label)
        .ok_or_else(|| "No page is loading.".to_owned())?
        .eval("window.stop()")
        .map_err(|_| "Could not stop the current page load.".to_owned())?;
    let generation = app
        .state::<NavigationStore>()
        .registry
        .lock()
        .map_err(|_| "Browser tab registry is unavailable.".to_owned())?
        .tab_snapshot(&profile_id, &tab_id)?
        .generation;
    set_tab_loading(&app, &profile_id, &tab_id, generation, false);
    Ok(())
}

#[tauri::command]
async fn close_profile(app: AppHandle, webview: Webview, profile_id: String) -> Result<(), String> {
    require_local_shell(&webview, Some(&profile_id))?;
    let store = app.state::<NavigationStore>();
    let _operation = store.native_ops.lock().await;
    let (window_label, _, _) = browser_labels(&profile_id)?;
    let store = app.state::<NavigationStore>();
    store.mark_closing(&profile_id)?;
    if let Some(window) = app.get_window(&window_label) {
        if window.close().is_err() {
            store.cancel_close(&profile_id);
            return Err("Could not close the selected browser window.".to_owned());
        }
        store.reset_profile(&profile_id);
    } else {
        store.finish_close(&profile_id);
    }
    Ok(())
}

#[tauri::command]
fn profile_directory(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
) -> Result<String, String> {
    require_local_shell(&webview, Some(&profile_id))?;
    let (_, path) = profile_details(&app, &profile_id)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn list_profiles(
    webview: Webview,
    store: tauri::State<'_, ProfileStore>,
    current_profile_id: Option<String>,
) -> Result<Vec<BrowserProfile>, String> {
    require_local_shell(&webview, current_profile_id.as_deref())?;
    store.list()
}

#[tauri::command]
fn create_profile(
    app: AppHandle,
    webview: Webview,
    store: tauri::State<'_, ProfileStore>,
    current_profile_id: Option<String>,
    name: String,
    avatar: String,
    accent: String,
) -> Result<BrowserProfile, String> {
    require_local_shell(&webview, current_profile_id.as_deref())?;
    let profile = store.create(&name, &avatar, &accent)?;
    let _ = app.emit("browser:profiles-changed", ());
    Ok(profile)
}

#[tauri::command]
fn update_profile(
    app: AppHandle,
    webview: Webview,
    store: tauri::State<'_, ProfileStore>,
    current_profile_id: Option<String>,
    profile_id: String,
    name: String,
    avatar: String,
    accent: String,
) -> Result<BrowserProfile, String> {
    require_local_shell(&webview, current_profile_id.as_deref())?;
    let profile = store.update(&profile_id, &name, &avatar, &accent)?;
    let _ = app.emit("browser:profiles-changed", ());
    Ok(profile)
}

#[tauri::command]
fn show_profile_manager(
    app: AppHandle,
    webview: Webview,
    current_profile_id: Option<String>,
) -> Result<(), String> {
    require_local_shell(&webview, current_profile_id.as_deref())?;
    let landing = app
        .get_window("main")
        .ok_or_else(|| "The profile landing window is unavailable.".to_owned())?;
    landing
        .show()
        .map_err(|_| "Could not show profile management.".to_owned())?;
    landing
        .set_focus()
        .map_err(|_| "Could not focus profile management.".to_owned())
}

fn main() {
    tauri::Builder::default()
        .manage(NavigationStore::default())
        .setup(|app| {
            let catalog = app
                .path()
                .app_local_data_dir()?
                .join("profile-catalog.jsonl");
            let store = ProfileStore::load(catalog)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            update_profile,
            show_profile_manager,
            open_profile,
            close_profile,
            profile_directory,
            browser_navigate,
            browser_state,
            browser_tabs_state,
            browser_new_tab,
            browser_select_tab,
            browser_close_tab,
            browser_back,
            browser_forward,
            browser_reload,
            browser_stop
        ])
        .run(tauri::generate_context!())
        .expect("LEVI Browser failed to start");
}

#[cfg(test)]
mod tests {
    use super::{resolve_address, NavigationState, NavigationStore};

    #[test]
    fn accepts_https_http_and_scheme_less_domains() {
        assert_eq!(
            resolve_address("https://example.com", "https://duckduckgo.com/?q={query}")
                .unwrap()
                .as_str(),
            "https://example.com/"
        );
        assert_eq!(
            resolve_address("http://example.com", "https://duckduckgo.com/?q={query}")
                .unwrap()
                .scheme(),
            "http"
        );
        assert_eq!(
            resolve_address("example.com", "https://duckduckgo.com/?q={query}")
                .unwrap()
                .as_str(),
            "https://example.com/"
        );
    }

    #[test]
    fn sends_plain_queries_to_configured_https_search() {
        let result =
            resolve_address("hello world", "https://www.google.com/search?q={query}").unwrap();
        assert_eq!(result.host_str(), Some("www.google.com"));
        assert_eq!(result.query(), Some("q=hello+world"));
        let single_word = resolve_address("weather", "https://duckduckgo.com/?q={query}").unwrap();
        assert_eq!(single_word.query(), Some("q=weather"));
    }

    #[test]
    fn rejects_unsupported_or_invalid_schemes_and_search_templates() {
        assert!(
            resolve_address("javascript:alert(1)", "https://duckduckgo.com/?q={query}").is_err()
        );
        assert!(
            resolve_address("file:///C:/secret.txt", "https://duckduckgo.com/?q={query}").is_err()
        );
        assert!(resolve_address("not a valid search", "javascript:{query}").is_err());
    }

    #[test]
    fn closing_and_reopening_alpha_clears_only_alpha_navigation_state() {
        let store = NavigationStore::default();
        store.ensure_profile("alpha").unwrap();
        store.ensure_profile("beta").unwrap();
        store.with_active_navigation("alpha", |state| {
            state.current_url = Some("https://old-alpha.example/".into());
            state.entries.push("https://old-alpha.example/".into());
            state.index = Some(0);
        });
        store.with_active_navigation("beta", |state| {
            state.current_url = Some("https://beta.example/".into());
            state.entries.push("https://beta.example/".into());
            state.index = Some(0);
        });

        store.mark_closing("alpha").unwrap();
        assert!(store.is_closing("alpha").unwrap());
        assert!(!store.is_closing("beta").unwrap());
        store.finish_close("alpha");

        assert_eq!(store.state_or_default("alpha"), NavigationState::default());
        assert_eq!(
            store.state_or_default("beta").current_url.as_deref(),
            Some("https://beta.example/")
        );

        assert!(!store.is_closing("alpha").unwrap());
        store.ensure_profile("alpha").unwrap();
        store.with_active_navigation("alpha", |state| {
            state.current_url = Some("https://new-alpha.example/".into());
            state.entries.push("https://new-alpha.example/".into());
            state.index = Some(0);
        });
        assert_eq!(
            store.state_or_default("alpha").current_url.as_deref(),
            Some("https://new-alpha.example/")
        );
        assert_eq!(
            store.state_or_default("beta").current_url.as_deref(),
            Some("https://beta.example/")
        );
    }

    #[test]
    fn closing_and_reopening_beta_clears_only_beta_navigation_state() {
        let store = NavigationStore::default();
        store.ensure_profile("alpha").unwrap();
        store.ensure_profile("beta").unwrap();
        store.with_active_navigation("alpha", |state| {
            state.current_url = Some("https://alpha.example/".into());
            state.entries.push("https://alpha.example/".into());
            state.index = Some(0);
        });
        store.with_active_navigation("beta", |state| {
            state.current_url = Some("https://old-beta.example/".into());
            state.entries.push("https://old-beta.example/".into());
            state.index = Some(0);
        });

        store.mark_closing("beta").unwrap();
        store.finish_close("beta");
        assert_eq!(store.state_or_default("beta"), NavigationState::default());
        assert_eq!(
            store.state_or_default("alpha").current_url.as_deref(),
            Some("https://alpha.example/")
        );

        assert!(!store.is_closing("beta").unwrap());
        store.ensure_profile("beta").unwrap();
        store.with_active_navigation("beta", |state| {
            state.current_url = Some("https://new-beta.example/".into());
            state.entries.push("https://new-beta.example/".into());
            state.index = Some(0);
        });
        assert_eq!(
            store.state_or_default("beta").current_url.as_deref(),
            Some("https://new-beta.example/")
        );
        assert_eq!(
            store.state_or_default("alpha").current_url.as_deref(),
            Some("https://alpha.example/")
        );
    }
}
