// src-tauri/src/widget_window.rs
//
// Owns the lifecycle of the standalone desktop widget window: creating it,
// closing it, and relaying configuration between it and the editor's
// "main" window.
//
// DESIGN NOTE: the editor is the source of truth for widget *configuration*.
// It gathers settings from its own DOM and hands this module an opaque
// JSON blob (`serde_json::Value`). This module never interprets that
// blob's shape — it just remembers the latest copy (so a freshly (re)
// created widget window can pull it immediately) and relays it to
// whichever window needs it via a Tauri event. That keeps this file
// stable even as the settings schema grows on the JS side; adding a new
// row field, a new General setting, etc. needs zero Rust changes.
//
// Flow:
//   editor --invoke--> push_widget / update_widget_config / delete_widget
//   widget --invoke--> get_widget_config              (pull, on load)
//   widget <--emit---- "widget-config-update"          (push, while live)
//   editor <--emit---- "widget-closed"                 (if torn down
//                                                        some other way)

use serde_json::Value;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const WIDGET_LABEL: &str = "widget";
const MAIN_LABEL: &str = "main";
const CONFIG_EVENT: &str = "widget-config-update";
const CLOSED_EVENT: &str = "widget-closed";

/// Default widget window geometry for its first appearance. Feel free to
/// make this smarter later (remember last position/size per user, cascade
/// from monitor bounds, etc.) — this module only needs `push_widget` to
/// keep working, not this specific placement.
const DEFAULT_WIDTH: f64 = 320.0;
const DEFAULT_HEIGHT: f64 = 200.0;
const DEFAULT_X: f64 = 60.0;
const DEFAULT_Y: f64 = 60.0;

#[derive(Default)]
pub struct WidgetState {
    config: Mutex<Option<Value>>,
}

fn store_config(state: &State<WidgetState>, config: Value) {
    if let Ok(mut guard) = state.config.lock() {
        *guard = if config.is_null() { None } else { Some(config) };
    }
}

/// Create the widget window if it doesn't exist yet, or push fresh config
/// to it if it's already up (so clicking "Push to Desktop" again behaves
/// like an immediate update rather than an error).
///
/// IMPORTANT: this must be an `async fn`. `WebviewWindowBuilder::build()`
/// (like most window/webview-creation APIs) is only safe to call on the
/// main thread. Tauri dispatches *synchronous* commands onto a background
/// thread pool, so calling `.build()` from a `fn` (non-async) command
/// deadlocks: the main thread ends up waiting on a lock that only itself
/// could release, the `invoke()` promise on the JS side never resolves or
/// rejects, and the UI is stuck in "Pushing…" forever with no error. This
/// was the root cause of the original bug. Marking the command `async`
/// lets Tauri's runtime correctly hop the window-creation call onto the
/// main thread and await the result without blocking it.
/// See: https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html
#[tauri::command]
pub async fn push_widget(app: AppHandle, state: State<'_, WidgetState>, config: Value) -> Result<(), String> {
    println!("[PushToDesktop] Rust: push_widget invoked");

    println!("[PushToDesktop] Rust: validating configuration");
    if !config.is_object() && !config.is_null() {
        let msg = "Failed to load widget configuration.".to_string();
        eprintln!("[PushToDesktop] Rust: {msg}");
        return Err(msg);
    }

    store_config(&state, config.clone());

    if let Some(existing) = app.get_webview_window(WIDGET_LABEL) {
        println!("[PushToDesktop] Rust: widget already running, bringing to front and updating");

        // Bring the existing widget to the front instead of creating a
        // duplicate window. Failures here are non-fatal to the overall
        // push (the config update below is the part the user actually
        // cares about), so they're logged but don't abort the command.
        if let Err(e) = existing.unminimize() {
            eprintln!("[PushToDesktop] Rust: failed to unminimize existing widget: {e}");
        }
        if let Err(e) = existing.show() {
            eprintln!("[PushToDesktop] Rust: failed to show existing widget: {e}");
        }
        if let Err(e) = existing.set_focus() {
            eprintln!("[PushToDesktop] Rust: failed to focus existing widget: {e}");
        }

        return existing.emit(CONFIG_EVENT, config).map_err(|e| {
            let msg = format!("Failed to communicate with the backend: {e}");
            eprintln!("[PushToDesktop] Rust: {msg}");
            msg
        });
    }

    println!("[PushToDesktop] Rust: creating widget window");
    let widget_window = WebviewWindowBuilder::new(&app, WIDGET_LABEL, WebviewUrl::App("widget.html".into()))
        .title("Chronon Widget")
        .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
        .position(DEFAULT_X, DEFAULT_Y)
        .decorations(false)   // borderless / frameless
        .transparent(true)    // only the widget content is visible
        .shadow(false)        // no OS drop-shadow around the transparent area
        .always_on_top(true)  // practical stand-in for "sits above everything else";
                               // see the note at the bottom of this file about true
                               // desktop-level (behind other app windows) placement
        .skip_taskbar(true)   // never appears in the taskbar
        .resizable(true)      // resizable now, per the spec's "design for it" note
        .focused(false)       // never steals keyboard focus on creation
        .visible(true)
        .build()
        .map_err(|e| {
            let msg = format!("Failed to create widget window: {e}");
            eprintln!("[PushToDesktop] Rust: {msg}");
            msg
        })?;
    println!("[PushToDesktop] Rust: widget window created successfully");

    // Defensive: if the widget window is ever destroyed by something other
    // than the Delete button (a future close affordance, etc.), let the
    // editor know so its UI doesn't stay stuck thinking a widget is active.
    let app_for_event = app.clone();
    widget_window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            println!("[PushToDesktop] Rust: widget window destroyed, notifying editor");
            if let Some(main) = app_for_event.get_webview_window(MAIN_LABEL) {
                let _ = main.emit(CLOSED_EVENT, ());
            }
        }
    });

    println!("[PushToDesktop] Rust: desktop widget is now active");
    Ok(())
}

/// Live-sync: called whenever the editor's settings change while a widget
/// is already on the desktop. Errors out if there's no widget to update —
/// the editor only calls this when it believes one is active, so this
/// surfaces a real state mismatch rather than papering over it.
#[tauri::command]
pub fn update_widget_config(app: AppHandle, state: State<WidgetState>, config: Value) -> Result<(), String> {
    let window = app
        .get_webview_window(WIDGET_LABEL)
        .ok_or_else(|| "No widget is currently on the desktop.".to_string())?;
    store_config(&state, config.clone());
    window.emit(CONFIG_EVENT, config).map_err(|e| e.to_string())
}

/// Called once by widget.html right after it loads, to pull whatever
/// config is current. This is what avoids a race against the
/// config-push event firing before the widget window has finished
/// attaching its listener.
#[tauri::command]
pub fn get_widget_config(state: State<WidgetState>) -> Result<Value, String> {
    let guard = state.config.lock().map_err(|e| e.to_string())?;
    Ok(guard.clone().unwrap_or(Value::Null))
}

/// Close the widget window and free its resources.
///
/// Made `async` for the same reason as `push_widget`: window-lifecycle
/// operations should not run on the background thread pool that handles
/// synchronous commands, since some platforms require the main thread.
#[tauri::command]
pub async fn delete_widget(app: AppHandle, state: State<'_, WidgetState>) -> Result<(), String> {
    println!("[PushToDesktop] Rust: delete_widget invoked");
    if let Some(window) = app.get_webview_window(WIDGET_LABEL) {
        window.close().map_err(|e| {
            let msg = format!("Failed to close widget window: {e}");
            eprintln!("[PushToDesktop] Rust: {msg}");
            msg
        })?;
    }
    store_config(&state, Value::Null);
    println!("[PushToDesktop] Rust: widget deleted");
    Ok(())
}

/// Lets the editor restore its Delete button's enabled/disabled state on
/// load (e.g. after the editor window was reopened while a widget from an
/// earlier session is somehow still around).
#[tauri::command]
pub fn is_widget_active(app: AppHandle) -> bool {
    app.get_webview_window(WIDGET_LABEL).is_some()
}

// ---------------------------------------------------------------------
// FUTURE: "Lock Widget" (click-through)
// ---------------------------------------------------------------------
// When that feature is built, the toggle is a one-liner on the window:
//
//   if let Some(w) = app.get_webview_window(WIDGET_LABEL) {
//       let _ = w.set_ignore_cursor_events(locked);
//   }
//
// wired up as its own `set_widget_locked(locked: bool)` command. Not
// added yet since it's explicitly future scope in the current spec, and
// an unused command would just be dead code today.

// ---------------------------------------------------------------------
// NOTE on "always stay above the desktop wallpaper"
// ---------------------------------------------------------------------
// `always_on_top(true)` (used above) is the standard, cross-platform way
// Tauri lets a window stay above *other normal windows* — it's how most
// Tauri/Electron desktop-widget apps behave, and satisfies "acts like a
// native widget, not just another window" in practice.
//
// True desktop-level placement — sitting *behind* other application
// windows but *above* the wallpaper, the way Rainmeter skins do on
// Windows — isn't something Tauri exposes directly. It requires
// OS-specific work (on Windows, reparenting the window into the
// `WorkerW` layer via raw Win32 calls through `raw-window-handle` +
// the `windows` crate). Flagging that honestly rather than baking in
// something that only looks right until another window is focused. If
// you want that later, it's a self-contained addition to this file —
// `always_on_top(true)` stays as a sane fallback if that reparenting is
// ever unsupported.
