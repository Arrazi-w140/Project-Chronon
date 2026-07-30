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
// Position is the one exception to the "opaque blob" rule above: it's
// inherently a native window property (an OS-level coordinate), not
// something widget-renderer.js can apply to the DOM, so it travels as its
// own explicitly-typed `x`/`y` pair instead of living inside `config`.
//
// Flow:
//   editor --invoke--> push_widget(config, x, y) / update_widget_config
//   editor --invoke--> set_widget_position(x, y)      (live, while active)
//   editor --invoke--> delete_widget
//   widget --invoke--> get_widget_config              (pull, on load)
//   widget <--emit---- "widget-config-update"          (push, while live)
//   editor <--emit---- "widget-closed"                 (if torn down
//                                                        some other way)

use serde_json::Value;
use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
#[cfg(target_os = "windows")]
use crate::desktop_layer;

pub const WIDGET_LABEL: &str = "widget";
const MAIN_LABEL: &str = "main";
const CONFIG_EVENT: &str = "widget-config-update";
const CLOSED_EVENT: &str = "widget-closed";

/// Default widget window *size* for its first appearance. Feel free to make
/// this smarter later (remember last size per user, cascade from monitor
/// bounds, etc.) — this module only needs `push_widget` to keep working,
/// not this specific size. Initial *position* is no longer defaulted here;
/// it always comes from whatever the editor's Position fields say (see
/// `push_widget`'s `x`/`y` params below), which in turn default to (0, 0)
/// on the JS side.
const DEFAULT_WIDTH: f64 = 320.0;
const DEFAULT_HEIGHT: f64 = 200.0;

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
pub async fn push_widget(
    app: AppHandle,
    state: State<'_, WidgetState>,
    config: Value,
    x: f64,
    y: f64,
) -> Result<(), String> {
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
        // Deliberately NOT calling set_focus() here: a desktop widget
        // should never steal keyboard focus, including on a re-push.
        if let Err(e) = existing.set_position(LogicalPosition::new(x, y)) {
            eprintln!("[PushToDesktop] Rust: failed to reposition existing widget: {e}");
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
        .position(x, y)
        .decorations(false)     // borderless / frameless
        .transparent(true)      // only the widget content is visible
        .shadow(false)          // no OS drop-shadow around the transparent area
        .always_on_bottom(true) // cross-platform baseline: never covers a normal app
                                 // window. On Windows this is superseded a few lines
                                 // down by the real desktop-layer placement (see
                                 // desktop_layer.rs); on macOS/Linux it's the
                                 // approximation Tauri exposes for "sits behind
                                 // everything else" (see the note at the bottom of
                                 // this file).
        .skip_taskbar(true)     // never appears in the taskbar
        .resizable(true)        // resizable now, per the spec's "design for it" note
        .focused(false)         // never steals keyboard focus on creation
        .visible(true)
        .build()
        .map_err(|e| {
            let msg = format!("Failed to create widget window: {e}");
            eprintln!("[PushToDesktop] Rust: {msg}");
            msg
        })?;
    println!("[PushToDesktop] Rust: widget window created successfully");

    // Windows only: reparent into the desktop's WorkerW layer so the widget
    // sits *behind* the desktop icons instead of merely behind other app
    // windows. See desktop_layer.rs for the full explanation and its
    // (soft) failure modes.
    #[cfg(target_os = "windows")]
    desktop_layer::attach(&widget_window);

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

/// Live-sync for the Position fields: called whenever the editor's X/Y
/// inputs change while a widget is already on the desktop. Mirrors
/// `update_widget_config`'s "no widget, no-op-with-an-error" shape rather
/// than silently swallowing a state mismatch.
///
/// This is deliberately its own command instead of riding along inside
/// `config` in `update_widget_config`: position is a native window
/// property that this module has to act on directly (there's no DOM for
/// it to land in on the widget side), whereas everything in `config` stays
/// opaque JSON this module never looks at. See the module doc comment.
#[tauri::command]
pub fn set_widget_position(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    let window = app
        .get_webview_window(WIDGET_LABEL)
        .ok_or_else(|| "No widget is currently on the desktop.".to_string())?;
    window
        .set_position(LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())
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
// NOTE on "sits above the wallpaper, below the icons, below every app"
// ---------------------------------------------------------------------
// The widget is never `always_on_top`. On Windows, `desktop_layer::attach`
// (called right after window creation, above) reparents it into the same
// "WorkerW" layer Rainmeter/Wallpaper Engine use, which genuinely produces
// the requested stacking: wallpaper < widget < desktop icons < every
// normal app window. See desktop_layer.rs for the mechanics and its known
// fragility (undocumented Explorer behavior; an Explorer restart can
// strand the widget until the app is relaunched).
//
// On macOS and Linux there's no equivalent public concept of "the WorkerW
// behind the icons" to reparent into, so those platforms fall back to
// Tauri's cross-platform `always_on_bottom(true)`: the widget stays below
// other app windows, but isn't guaranteed to sit behind desktop icons
// specifically. Genuinely correct per-platform layering there (an NSWindow
// level on macOS between the desktop and Finder's icon layer; a
// `_NET_WM_STATE_BELOW`-style approach on Linux, which itself varies by
// desktop environment) is a reasonable follow-up but out of scope here.
