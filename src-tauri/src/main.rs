#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Mutex,
};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    webview::WebviewBuilder,
    AppHandle, Emitter, Manager, Webview, WebviewUrl, WindowBuilder,
};
use url::Url;

const CHROME_HEIGHT: f64 = 96.0;
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
    states: Mutex<HashMap<String, NavigationState>>,
    closing_profiles: Mutex<HashSet<String>>,
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

impl NavigationStore {
    fn reset_profile(&self, profile_id: &str) {
        if let Ok(mut states) = self.states.lock() {
            states.remove(profile_id);
        }
    }

    fn state_or_default(&self, profile_id: &str) -> NavigationState {
        self.states
            .lock()
            .ok()
            .and_then(|states| states.get(profile_id).cloned())
            .unwrap_or_default()
    }

    fn mark_closing(&self, profile_id: &str) -> Result<(), String> {
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
    }

    fn cancel_close(&self, profile_id: &str) {
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
    let profile = match profile_id {
        "alpha" => "alpha",
        "beta" => "beta",
        _ => return Err("Choose one of the two isolated test profiles.".into()),
    };
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Could not resolve LEVI Browser local data directory.".to_owned())?
        .join("profile-isolation-lab")
        .join(profile);
    Ok((format!("profile-{profile}"), path))
}

fn browser_labels(profile_id: &str) -> Result<(String, String, String), String> {
    if !matches!(profile_id, "alpha" | "beta") {
        return Err("Choose one of the two isolated test profiles.".into());
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
        state,
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

fn set_loading(app: &AppHandle, profile_id: &str, loading: bool) {
    let store = app.state::<NavigationStore>();
    if let Ok(mut states) = store.states.lock() {
        let state = states.entry(profile_id.to_owned()).or_default();
        state.is_loading = loading;
        emit_state(app, profile_id, state);
    };
}

fn push_navigation(app: &AppHandle, profile_id: &str, url: &Url) {
    let store = app.state::<NavigationStore>();
    if let Ok(mut states) = store.states.lock() {
        let state = states.entry(profile_id.to_owned()).or_default();
        let value = url.to_string();
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
        emit_state(app, profile_id, state);
    };
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
        format!("http://127.0.0.1:4173/browser-{profile_id}.html")
            .parse()
            .map_err(|_| "Could not construct the local browser chrome URL.".to_owned())?,
    );
    #[cfg(not(debug_assertions))]
    let chrome_url = WebviewUrl::App(format!("browser-{profile_id}.html").into());

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
) -> Result<ProfileLaunch, String> {
    require_local_shell(&webview, None)?;
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

fn ensure_content_webview(app: &AppHandle, profile_id: &str, url: Url) -> Result<(), String> {
    let (window_label, chrome_label, content_label) = browser_labels(profile_id)?;
    navigation_log(
        "content_webview_creation_requested",
        profile_id,
        &format!("label={content_label} {}", url_log_fields(&url)),
    );
    let (_, directory) = profile_details(app, profile_id)?;
    fs::create_dir_all(&directory)
        .map_err(|_| "Could not create the isolated profile data directory.".to_owned())?;
    let window = app.get_window(&window_label).ok_or_else(|| {
        navigation_log(
            "content_webview_creation_failed",
            profile_id,
            "reason=parent_window_unavailable",
        );
        "Open this isolated browser profile before navigating.".to_owned()
    })?;
    if let Some(content) = app.get_webview(&content_label) {
        content.show().map_err(|error| {
            navigation_log(
                "content_webview_show_failed",
                profile_id,
                &format!("stage=reuse error={error}"),
            );
            "The website surface could not be shown.".to_owned()
        })?;
        navigation_log(
            "content_webview_reused_and_shown",
            profile_id,
            &format!(
                "label={content_label} parent_visible={:?}",
                window.is_visible()
            ),
        );
        content.navigate(url.clone()).map_err(|error| {
            navigation_log(
                "navigation_request_failed",
                profile_id,
                &format!("{} error={error}", url_log_fields(&url)),
            );
            "The website navigation request could not be sent.".to_owned()
        })?;
        navigation_log("navigation_request_sent", profile_id, &url_log_fields(&url));
        return Ok(());
    }

    let size = window
        .inner_size()
        .map_err(|_| "Could not read browser window size.".to_owned())?;
    let scale = window
        .scale_factor()
        .map_err(|_| "Could not read browser window scaling.".to_owned())?;
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    let app_for_navigation = app.clone();
    let navigation_profile = profile_id.to_owned();
    let app_for_load = app.clone();
    let load_profile = profile_id.to_owned();
    let builder = WebviewBuilder::new(&content_label, WebviewUrl::External(url))
        .data_directory(directory)
        .on_navigation(move |target| {
            if matches!(target.scheme(), "http" | "https") {
                navigation_log(
                    "navigation_started",
                    &navigation_profile,
                    &url_log_fields(target),
                );
                push_navigation(&app_for_navigation, &navigation_profile, target);
                true
            } else {
                navigation_log(
                    "navigation_blocked",
                    &navigation_profile,
                    &format!("scheme={}", target.scheme()),
                );
                if let Ok(mut states) = app_for_navigation.state::<NavigationStore>().states.lock()
                {
                    let state = states.entry(navigation_profile.clone()).or_default();
                    state.error = Some(
                        "Navigation was blocked: only HTTP and HTTPS pages are allowed.".into(),
                    );
                    emit_state(&app_for_navigation, &navigation_profile, state);
                }
                false
            }
        })
        .on_page_load(move |_webview, payload| {
            let event_name = match payload.event() {
                tauri::webview::PageLoadEvent::Started => "page_load_started",
                tauri::webview::PageLoadEvent::Finished => "page_load_finished",
            };
            let event_url = payload.url().to_string();
            let fields = event_url
                .parse::<Url>()
                .map(|url| url_log_fields(&url))
                .unwrap_or_else(|_| "url=unparseable".to_owned());
            navigation_log(event_name, &load_profile, &fields);
            set_loading(
                &app_for_load,
                &load_profile,
                matches!(payload.event(), tauri::webview::PageLoadEvent::Started),
            );
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                if let Ok(current_url) = payload.url().to_string().parse::<Url>() {
                    push_navigation(&app_for_load, &load_profile, &current_url);
                }
            }
        });

    navigation_log(
        "content_webview_add_child_begin",
        profile_id,
        &format!(
            "label={content_label} x=0 y={CHROME_HEIGHT} width={width:.0} height={:.0}",
            (height - CHROME_HEIGHT).max(100.0)
        ),
    );
    window
        .add_child(
            builder,
            tauri::LogicalPosition::new(0.0, CHROME_HEIGHT),
            tauri::LogicalSize::new(width, (height - CHROME_HEIGHT).max(100.0)),
        )
        .map_err(|error| {
            navigation_log(
                "content_webview_creation_failed",
                profile_id,
                &format!("stage=add_child error={error}"),
            );
            "Could not create the isolated website WebView2 surface.".to_owned()
        })?;

    match app.get_webview(&content_label) {
        Some(content) => {
            content.show().map_err(|error| {
                navigation_log(
                    "content_webview_show_failed",
                    profile_id,
                    &format!("stage=created error={error}"),
                );
                "The website surface could not be shown.".to_owned()
            })?;
            navigation_log(
                "content_webview_creation_succeeded_and_shown",
                profile_id,
                &format!(
                    "label={content_label} parent_visible={:?}",
                    window.is_visible()
                ),
            );
        }
        None => navigation_log(
            "content_webview_creation_succeeded_but_lookup_missing",
            profile_id,
            &format!("label={content_label}"),
        ),
    }

    let resize_app = app.clone();
    let resize_content_label = content_label.clone();
    let resize_chrome_label = chrome_label;
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(physical) = event {
            let Some(content) = resize_app.get_webview(&resize_content_label) else {
                return;
            };
            let Some(chrome) = resize_app.get_webview(&resize_chrome_label) else {
                return;
            };
            let scale = resize_app
                .get_window(&window_label)
                .and_then(|win| win.scale_factor().ok())
                .unwrap_or(1.0);
            let width = physical.width as f64 / scale;
            let height = physical.height as f64 / scale;
            let _ = chrome.set_size(tauri::LogicalSize::new(width, height));
            let _ = content.set_position(tauri::LogicalPosition::new(0.0, CHROME_HEIGHT));
            let _ = content.set_size(tauri::LogicalSize::new(
                width,
                (height - CHROME_HEIGHT).max(100.0),
            ));
        }
    });
    Ok(())
}

#[tauri::command]
async fn browser_navigate(
    app: AppHandle,
    webview: Webview,
    profile_id: String,
    input: String,
    search_template: String,
) -> Result<String, String> {
    navigation_log(
        "navigation_command_invoked",
        &profile_id,
        "source=browser_chrome",
    );
    require_profile_chrome(&webview, &profile_id)?;
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
    ensure_content_webview(&app, &profile_id, target.clone()).map_err(|error| {
        navigation_log(
            "navigation_command_failed",
            &profile_id,
            &format!("stage=webview_setup error={error}"),
        );
        error
    })?;
    set_loading(&app, &profile_id, true);
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
    let (_, _, content_label) = browser_labels(&profile_id)?;
    if app.get_webview(&content_label).is_none() {
        // A newly opened chrome has no website child until its first navigation.
        // Never expose history left behind by a previous window lifecycle.
        app.state::<NavigationStore>().reset_profile(&profile_id);
        return Ok(NavigationState::default());
    }
    Ok(navigation_state(&app, &profile_id))
}

#[tauri::command]
fn browser_back(app: AppHandle, webview: Webview, profile_id: String) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let (_, _, content_label) = browser_labels(&profile_id)?;
    let store = app.state::<NavigationStore>();
    let mut states = store
        .states
        .lock()
        .map_err(|_| "Navigation state is unavailable.".to_owned())?;
    let state = states.entry(profile_id.clone()).or_default();
    let Some(index) = state.index else {
        return Err("There is no previous page.".into());
    };
    if index == 0 {
        return Err("There is no previous page.".into());
    }
    state.index = Some(index - 1);
    state.pending_history_index = Some(index - 1);
    if let Some(url) = state.entries.get(index - 1) {
        state.current_url = Some(url.clone());
    }
    if let Some(content) = app.get_webview(&content_label) {
        content
            .eval("history.back()")
            .map_err(|_| "Could not navigate back.".to_owned())?;
    }
    emit_state(&app, &profile_id, state);
    Ok(())
}

#[tauri::command]
fn browser_forward(app: AppHandle, webview: Webview, profile_id: String) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let (_, _, content_label) = browser_labels(&profile_id)?;
    let store = app.state::<NavigationStore>();
    let mut states = store
        .states
        .lock()
        .map_err(|_| "Navigation state is unavailable.".to_owned())?;
    let state = states.entry(profile_id.clone()).or_default();
    let Some(index) = state.index else {
        return Err("There is no next page.".into());
    };
    if index + 1 >= state.entries.len() {
        return Err("There is no next page.".into());
    }
    state.index = Some(index + 1);
    state.pending_history_index = Some(index + 1);
    if let Some(url) = state.entries.get(index + 1) {
        state.current_url = Some(url.clone());
    }
    if let Some(content) = app.get_webview(&content_label) {
        content
            .eval("history.forward()")
            .map_err(|_| "Could not navigate forward.".to_owned())?;
    }
    emit_state(&app, &profile_id, state);
    Ok(())
}

#[tauri::command]
fn browser_reload(app: AppHandle, webview: Webview, profile_id: String) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let (_, _, label) = browser_labels(&profile_id)?;
    app.get_webview(&label)
        .ok_or_else(|| "No page is open to reload.".to_owned())?
        .reload()
        .map_err(|_| "Could not reload the current page.".into())
}

#[tauri::command]
fn browser_stop(app: AppHandle, webview: Webview, profile_id: String) -> Result<(), String> {
    require_profile_chrome(&webview, &profile_id)?;
    let (_, _, label) = browser_labels(&profile_id)?;
    app.get_webview(&label)
        .ok_or_else(|| "No page is loading.".to_owned())?
        .eval("window.stop()")
        .map_err(|_| "Could not stop the current page load.".to_owned())?;
    set_loading(&app, &profile_id, false);
    Ok(())
}

#[tauri::command]
fn close_profile(app: AppHandle, webview: Webview, profile_id: String) -> Result<(), String> {
    require_local_shell(&webview, Some(&profile_id))?;
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

fn main() {
    tauri::Builder::default()
        .manage(NavigationStore::default())
        .invoke_handler(tauri::generate_handler![
            open_profile,
            close_profile,
            profile_directory,
            browser_navigate,
            browser_state,
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
        {
            let mut states = store.states.lock().unwrap();
            states.insert(
                "alpha".into(),
                NavigationState {
                    current_url: Some("https://old-alpha.example/".into()),
                    entries: vec!["https://old-alpha.example/".into()],
                    index: Some(0),
                    is_loading: false,
                    error: None,
                    pending_history_index: None,
                },
            );
            states.insert(
                "beta".into(),
                NavigationState {
                    current_url: Some("https://beta.example/".into()),
                    entries: vec!["https://beta.example/".into()],
                    index: Some(0),
                    is_loading: false,
                    error: None,
                    pending_history_index: None,
                },
            );
        }

        store.mark_closing("alpha").unwrap();
        assert!(store.is_closing("alpha").unwrap());
        assert!(!store.is_closing("beta").unwrap());
        store.reset_profile("alpha");

        assert_eq!(store.state_or_default("alpha"), NavigationState::default());
        assert_eq!(
            store.state_or_default("beta").current_url.as_deref(),
            Some("https://beta.example/")
        );

        store.finish_close("alpha");
        assert!(!store.is_closing("alpha").unwrap());
        {
            let mut states = store.states.lock().unwrap();
            states.insert(
                "alpha".into(),
                NavigationState {
                    current_url: Some("https://new-alpha.example/".into()),
                    entries: vec!["https://new-alpha.example/".into()],
                    index: Some(0),
                    is_loading: false,
                    error: None,
                    pending_history_index: None,
                },
            );
        }
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
        {
            let mut states = store.states.lock().unwrap();
            states.insert(
                "alpha".into(),
                NavigationState {
                    current_url: Some("https://alpha.example/".into()),
                    entries: vec!["https://alpha.example/".into()],
                    index: Some(0),
                    is_loading: false,
                    error: None,
                    pending_history_index: None,
                },
            );
            states.insert(
                "beta".into(),
                NavigationState {
                    current_url: Some("https://old-beta.example/".into()),
                    entries: vec!["https://old-beta.example/".into()],
                    index: Some(0),
                    is_loading: false,
                    error: None,
                    pending_history_index: None,
                },
            );
        }

        store.mark_closing("beta").unwrap();
        store.reset_profile("beta");
        assert_eq!(store.state_or_default("beta"), NavigationState::default());
        assert_eq!(
            store.state_or_default("alpha").current_url.as_deref(),
            Some("https://alpha.example/")
        );

        store.finish_close("beta");
        assert!(!store.is_closing("beta").unwrap());
        {
            let mut states = store.states.lock().unwrap();
            states.insert(
                "beta".into(),
                NavigationState {
                    current_url: Some("https://new-beta.example/".into()),
                    entries: vec!["https://new-beta.example/".into()],
                    index: Some(0),
                    is_loading: false,
                    error: None,
                    pending_history_index: None,
                },
            );
        }
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
