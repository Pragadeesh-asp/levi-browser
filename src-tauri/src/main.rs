#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileLaunch {
    profile_id: String,
    window_label: String,
    data_directory: String,
    requested_url: String,
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

fn open_profile_on_main_thread(
    app: AppHandle,
    profile_id: String,
) -> Result<ProfileLaunch, String> {
    let (label, directory) = profile_details(&app, &profile_id)?;
    fs::create_dir_all(&directory)
        .map_err(|_| "Could not create the isolated profile data directory.".to_owned())?;

    if let Some(window) = app.get_webview_window(&label) {
        window
            .show()
            .map_err(|_| "Could not show the existing profile window.".to_owned())?;
        window
            .set_focus()
            .map_err(|_| "Could not focus the existing profile window.".to_owned())?;
        return Ok(ProfileLaunch {
            profile_id,
            window_label: label,
            data_directory: directory.display().to_string(),
            requested_url: window
                .url()
                .map_err(|_| "Could not read the existing profile window URL.".to_owned())?
                .to_string(),
            reused_window: true,
        });
    }

    #[cfg(debug_assertions)]
    let target_url: tauri::Url = format!("http://127.0.0.1:4173/profile-{profile_id}.html")
        .parse()
        .map_err(|_| "Could not construct the local profile test URL.".to_owned())?;
    #[cfg(debug_assertions)]
    let page = WebviewUrl::External(target_url.clone());
    #[cfg(not(debug_assertions))]
    let page = WebviewUrl::App(format!("profile-{profile_id}.html").into());
    let window: WebviewWindow = WebviewWindowBuilder::new(&app, &label, page)
        .title(format!(
            "LEVI BROWSER — isolated profile {}",
            profile_id.to_uppercase()
        ))
        .inner_size(920.0, 680.0)
        .data_directory(directory.clone())
        .build()
        .map_err(|_| "Tauri could not create the isolated WebView2 profile window.".to_owned())?;

    #[cfg(debug_assertions)]
    let requested_url = target_url.to_string();
    #[cfg(not(debug_assertions))]
    let requested_url = format!("profile-{profile_id}.html");

    #[cfg(debug_assertions)]
    window.navigate(target_url).map_err(|_| {
        "The profile window was created but its local page navigation failed.".to_owned()
    })?;

    window.show().map_err(|_| {
        "The isolated profile window was created but could not be shown.".to_owned()
    })?;

    Ok(ProfileLaunch {
        profile_id,
        window_label: label,
        data_directory: directory.display().to_string(),
        requested_url,
        reused_window: false,
    })
}

#[tauri::command]
async fn open_profile(app: AppHandle, profile_id: String) -> Result<ProfileLaunch, String> {
    let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
    let app_for_main_thread = app.clone();
    app.run_on_main_thread(move || {
        let result = open_profile_on_main_thread(app_for_main_thread, profile_id);
        let _ = result_sender.send(result);
    })
    .map_err(|_| "Could not schedule the profile window on the Windows UI thread.".to_owned())?;

    result_receiver
        .await
        .map_err(|_| "The profile window operation ended before returning a result.".to_owned())?
}

#[tauri::command]
fn close_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let (label, _) = profile_details(&app, &profile_id)?;
    if let Some(window) = app.get_webview_window(&label) {
        window
            .close()
            .map_err(|_| "Could not close the selected LEVI test profile window.".to_owned())?;
    }
    Ok(())
}

#[tauri::command]
fn profile_directory(app: AppHandle, profile_id: String) -> Result<String, String> {
    let (_, path) = profile_details(&app, &profile_id)?;
    Ok(path.display().to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            open_profile,
            close_profile,
            profile_directory
        ])
        .run(tauri::generate_context!())
        .expect("LEVI Browser isolation lab failed to start");
}
