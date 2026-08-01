// ================================================================
// src-tauri/src/updater.rs
// ----------------------------------------------------------------
// GitHub-release auto-update, built on tauri-plugin-updater +
// tauri-plugin-process. Plays the same role here that electron-updater
// plays in Project Playnck's main.js: a background check that keeps
// running for as long as the app process is alive (which, for Chronon,
// means as long as the desktop widget window exists — not just while
// the editor/settings window happens to be open), plus a couple of
// commands the Settings > Updates UI calls directly.
//
// Frontend counterpart: src/updater.js.
// ================================================================

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Holds the `Update` handle from the most recent successful check, so
/// `install_update_now` can act on it without checking again first.
/// electron-updater keeps this kind of state internally; the Tauri
/// plugin expects the app to hold onto it explicitly.
#[derive(Default)]
pub struct PendingUpdate(pub Mutex<Option<Update>>);

/// Whether the background loop below should actually check. Mirrors
/// Settings > Updates > "Automatically check for updates" — the
/// frontend syncs its saved (localStorage) preference into this on
/// startup and on every toggle change. Defaults to on.
pub struct AutoCheckEnabled(pub AtomicBool);
impl Default for AutoCheckEnabled {
    fn default() -> Self {
        Self(AtomicBool::new(true))
    }
}

/// Pushed to the frontend as the "update-status" event. `state` is the
/// discriminant JS switches on; the extra fields are only present on
/// the variants that need them.
#[derive(Clone, Serialize)]
#[serde(tag = "state")]
pub enum UpdateStatus {
    #[serde(rename = "checking")]
    Checking,
    #[serde(rename = "available")]
    Available { version: String },
    #[serde(rename = "downloading")]
    Downloading { percent: u8 },
    #[serde(rename = "up-to-date")]
    UpToDate { version: String },
    #[serde(rename = "error")]
    Error { message: String },
}

fn emit_status(app: &AppHandle, status: UpdateStatus) {
    let _ = app.emit("update-status", status);
}

/// Shared by the manual "Check for Updates Now" button and the
/// background loop, so both behave identically and the UI only ever
/// has to listen for one event stream.
async fn run_check(app: &AppHandle) {
    emit_status(app, UpdateStatus::Checking);

    let updater = match app.updater() {
        Ok(u) => u,
        Err(err) => {
            eprintln!("Updater unavailable: {err}");
            emit_status(
                app,
                UpdateStatus::Error {
                    message: "Couldn't check for updates. Check your internet connection and try again.".into(),
                },
            );
            return;
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            if let Some(pending) = app.try_state::<PendingUpdate>() {
                *pending.0.lock().unwrap() = Some(update);
            }
            emit_status(app, UpdateStatus::Available { version });
        }
        Ok(None) => {
            let version = app.package_info().version.to_string();
            emit_status(app, UpdateStatus::UpToDate { version });
        }
        Err(err) => {
            // Log the real cause for debugging but keep the message the
            // user sees generic and non-alarming — same call Playnck's
            // main.js makes about not forwarding raw error text to the
            // renderer.
            eprintln!("Update check failed: {err}");
            emit_status(
                app,
                UpdateStatus::Error {
                    message: "Couldn't check for updates. Check your internet connection and try again.".into(),
                },
            );
        }
    }
}

/// Runs once, 45 minutes apart, for as long as the app process is
/// alive. Skips the actual check in dev builds (`npm run tauri dev`)
/// and whenever the user has turned the Settings toggle off.
pub fn spawn_background_checks(app: AppHandle) {
    std::thread::spawn(move || {
        // Small delay so this can't possibly fire before updater.js has
        // registered its "update-status" listener on first launch.
        std::thread::sleep(Duration::from_secs(5));
        loop {
            let should_check = app
                .try_state::<AutoCheckEnabled>()
                .map(|s| s.0.load(Ordering::Relaxed))
                .unwrap_or(true);

            if should_check && !cfg!(debug_assertions) {
                tauri::async_runtime::block_on(run_check(&app));
            }

            std::thread::sleep(Duration::from_secs(45 * 60));
        }
    });
}

#[derive(Serialize)]
pub struct CheckStarted {
    pub started: bool,
    pub reason: Option<String>,
}

/// Manual "Check for Updates Now" button.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> CheckStarted {
    if cfg!(debug_assertions) {
        return CheckStarted {
            started: false,
            reason: Some("Updates only run in the installed app, not in `npm run tauri dev`.".into()),
        };
    }
    run_check(&app).await;
    CheckStarted {
        started: true,
        reason: None,
    }
}

/// "Download & Install" button — only enabled once a check has left an
/// `Update` sitting in `PendingUpdate`.
///
/// Note: on Windows, Tauri exits the process partway through
/// `download_and_install` in order to hand off to the NSIS installer — a
/// limitation of Windows installers, not something this code can avoid
/// (see Tauri's updater docs). Tauri's own updater→NSIS integration is
/// what relaunches Chronon once that silent install finishes; the
/// `app.restart()` call at the end of this function is the equivalent
/// explicit relaunch for any platform where the process *doesn't* exit
/// during the call above (this matches Tauri's own official example for
/// this command).
#[tauri::command]
pub async fn install_update_now(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<(), String> {
    let update = pending.0.lock().unwrap().take();
    let Some(update) = update else {
        return Err("No update is ready to install yet. Check for updates first.".into());
    };

    let downloaded = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let progress_app = app.clone();
    let progress_downloaded = downloaded.clone();
    let progress_total = total.clone();

    let result = update
        .download_and_install(
            move |chunk_length, content_length| {
                if let Some(len) = content_length {
                    progress_total.store(len, Ordering::Relaxed);
                }
                let now = progress_downloaded.fetch_add(chunk_length as u64, Ordering::Relaxed) + chunk_length as u64;
                let known_total = progress_total.load(Ordering::Relaxed);
                let percent = if known_total > 0 {
                    ((now as f64 / known_total as f64) * 100.0).min(100.0) as u8
                } else {
                    0
                };
                emit_status(&progress_app, UpdateStatus::Downloading { percent });
            },
            || {
                // Download finished; Tauri moves straight into installing
                // as soon as this returns. On Windows the process exits
                // here, so nothing after `.await` below runs in practice
                // on that platform.
            },
        )
        .await;

    if let Err(err) = result {
        eprintln!("Update install failed: {err}");
        emit_status(
            &app,
            UpdateStatus::Error {
                message: "Couldn't download or install the update. Check your internet connection and try again.".into(),
            },
        );
        return Err("Couldn't download or install the update.".into());
    }

    // Only reached on platforms where installing doesn't already exit
    // the process for us (Windows exits during the call above).
    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn set_auto_check_updates(enabled: bool, state: State<'_, AutoCheckEnabled>) {
    state.0.store(enabled, Ordering::Relaxed);
}
