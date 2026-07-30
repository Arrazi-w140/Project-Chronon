// src-tauri/src/desktop_layer.rs
//
// Windows-only. Reparents the widget window into the "WorkerW" layer that
// Explorer maintains between the desktop wallpaper and the desktop icons.
// This is the same undocumented trick tools like Rainmeter and Wallpaper
// Engine use to render "on the desktop" instead of as a normal floating
// window — there is no public, supported API for it.
//
// Z-order this produces, front (top) to back (bottom):
//   normal application windows  →  desktop icons  →  our widget  →  wallpaper
//
// The technique, in four steps:
//   1. Find "Progman" (Program Manager), the window that owns the wallpaper.
//   2. Send it the undocumented message 0x052C, which tells it to spawn a
//      "WorkerW" window behind the desktop icons if one doesn't exist yet.
//   3. Walk the top-level windows to find the one hosting the icons (it has
//      a "SHELLDLL_DefView" child), then grab *its* sibling "WorkerW" — an
//      empty window sitting directly behind the icons, in front of the
//      wallpaper. That's the layer we want to live in.
//   4. SetParent() our widget into that WorkerW.
//
// This is inherently fragile: 0x052C and the WorkerW arrangement are
// undocumented Explorer implementation details that could change between
// Windows versions, and an Explorer restart (crash, "Restart" from Task
// Manager, some shell extensions) destroys the WorkerW our widget was
// parented into, which silently strands it. None of that is fixable from
// here without a persistent watchdog re-attaching on shell restart — a
// reasonable follow-up, but out of scope for this pass. Every step below
// fails soft: if any part of the dance doesn't pan out, `attach` just
// leaves the widget as a normal (still not-always-on-top) window instead
// of panicking or erroring the whole push.

use tauri::WebviewWindow;
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, SendMessageTimeoutW, SetParent, SMTO_NORMAL,
};

/// Undocumented message that tells Progman to spawn the icon-layer WorkerW.
/// Ignored (no-op) if that WorkerW already exists.
const SPAWN_WORKERW: u32 = 0x052C;

/// `EnumWindows` callback: for each top-level window, check whether it owns
/// the desktop icons (a "SHELLDLL_DefView" child). The one that does is not
/// itself the layer we want — its *next sibling* "WorkerW" is the empty
/// layer sitting just behind the icons, which is what gets written into
/// `lparam` (a pointer to an `HWND` living on the caller's stack).
unsafe extern "system" fn find_icon_layer_sibling(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let owns_icons =
        FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok();

    if owns_icons {
        if let Ok(worker) = FindWindowExW(None, Some(hwnd), w!("WorkerW"), PCWSTR::null()) {
            let out = &mut *(lparam.0 as *mut HWND);
            *out = worker;
            return BOOL(0); // found it — stop enumerating
        }
    }

    BOOL(1) // keep looking
}

/// Reparents `window` into the desktop's WorkerW layer. Best-effort: logs
/// and returns on any failure, leaving the window as a normal (but still
/// not-always-on-top, per the caller) window rather than propagating an
/// error that would abort the whole "push to desktop" flow over what is,
/// after all, a cosmetic layering nicety.
pub fn attach(window: &WebviewWindow) {
    let hwnd = match window.hwnd() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[DesktopLayer] couldn't get the widget's HWND: {e}");
            return;
        }
    };

    unsafe {
        let progman = match FindWindowW(w!("Progman"), PCWSTR::null()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[DesktopLayer] Progman window not found: {e}");
                return;
            }
        };

        // Ask Explorer to spawn the icon-layer WorkerW. A one-second timeout
        // keeps a wedged/slow shell from hanging widget creation; failure
        // here isn't fatal on its own since the WorkerW may already exist
        // from a previous run.
        let _ = SendMessageTimeoutW(
            progman,
            SPAWN_WORKERW,
            WPARAM(0),
            LPARAM(0),
            SMTO_NORMAL,
            1000,
            None,
        );

        let mut target = HWND::default();
        if EnumWindows(
            Some(find_icon_layer_sibling),
            LPARAM(&mut target as *mut HWND as isize),
        )
        .is_err()
        {
            eprintln!("[DesktopLayer] EnumWindows failed while searching for the WorkerW layer");
            return;
        }

        if target.0.is_null() {
            eprintln!(
                "[DesktopLayer] couldn't locate the desktop's WorkerW layer; leaving the widget as a normal window"
            );
            return;
        }

        if let Err(e) = SetParent(hwnd, Some(target)) {
            eprintln!("[DesktopLayer] SetParent into the WorkerW layer failed: {e}");
        }
    }
}
