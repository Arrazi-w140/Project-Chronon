// src-tauri/src/desktop_layer.rs
//
// Windows-only. Makes the widget behave like an actual desktop layer
// (Rainmeter/Wallpaper Engine style) instead of a normal application
// window, in two independent parts:
//
//   1. `attach`  — reparents the widget into the "WorkerW" layer that
//      Explorer maintains between the desktop wallpaper and the desktop
//      icons, so it *renders* in the right place.
//   2. `guard_against_show_desktop` — makes the widget immune to the
//      shell's "minimize everything" gesture (the taskbar corner, Win+D,
//      and the three-finger-swipe-down touchpad gesture all trigger the
//      same underlying behavior), so it doesn't get swept away and back
//      like a normal window along with everything else.
//
// Both are independent and each fails soft on its own: if one doesn't pan
// out, the other still applies.
//
// --- Part 1: WorkerW reparenting ---
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
//
// --- Part 2: surviving "Show Desktop" ---
//
// Reparenting into WorkerW alone does *not* make the widget immune to the
// shell's "Show Desktop" gesture. That gesture (whichever of its triggers
// invokes it) walks the windows it considers minimizable and sends each
// one WM_SYSCOMMAND/SC_MINIMIZE — the exact same message a normal window
// gets when you click its taskbar icon or hit its minimize button — and
// then restores them the same way when you swipe/click again. Nothing
// about being a WorkerW child exempts a window from receiving that
// message. So without this part, the widget minimizes and restores right
// alongside every normal window, which is the bug this module now also
// fixes: `guard_against_show_desktop` subclasses the widget's own window
// procedure to swallow SC_MINIMIZE outright (the window simply never
// enters the minimized state, so there's nothing for the matching
// "restore" to undo later), with an immediate self-un-minimize as a
// fallback for any future/alternate path to the minimized state that
// doesn't route through SC_MINIMIZE. Every other message is passed
// through to the original window procedure completely untouched, so
// nothing else about the widget's behavior changes.

use std::sync::atomic::{AtomicIsize, Ordering};
use tauri::WebviewWindow;
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, EnumWindows, FindWindowExW, FindWindowW, GWLP_WNDPROC, SC_MINIMIZE,
    SIZE_MINIMIZED, SW_SHOWNOACTIVATE, SendMessageTimeoutW, SetParent, SetWindowLongPtrW,
    ShowWindow, SMTO_NORMAL, WM_SIZE, WM_SYSCOMMAND, WNDPROC,
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

/// The widget's original window procedure, saved by `guard_against_show_desktop`
/// so the subclass below can forward every message it doesn't care about.
/// A single slot is enough: there is only ever one widget window alive at a
/// time (see `widget_window::WIDGET_LABEL`), and it's re-subclassed fresh
/// each time a new widget window is created.
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

/// Replacement window procedure installed over the widget's own. Intercepts
/// exactly two things and passes every other message straight through to
/// the original proc, unmodified:
///
///   - WM_SYSCOMMAND / SC_MINIMIZE: swallowed outright (return 0 without
///     forwarding), so "Show Desktop" — corner click, Win+D, or the
///     three-finger swipe — can't minimize the widget in the first place.
///   - WM_SIZE / SIZE_MINIMIZED: a fallback net. If the window still ends
///     up iconic through some path that doesn't go through SC_MINIMIZE,
///     immediately un-minimize it (without stealing focus) rather than
///     leaving it hidden.
unsafe extern "system" fn subclass_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // The low four bits of wParam are reserved for the system's own use;
    // MSDN requires masking them off before comparing against SC_ values.
    if msg == WM_SYSCOMMAND && (wparam.0 & 0xFFF0) == SC_MINIMIZE as usize {
        return LRESULT(0);
    }

    let original: WNDPROC = std::mem::transmute(ORIGINAL_WNDPROC.load(Ordering::SeqCst));
    let result = CallWindowProcW(original, hwnd, msg, wparam, lparam);

    if msg == WM_SIZE && wparam.0 == SIZE_MINIMIZED as usize {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }

    result
}

/// Subclasses `window` so it can never be minimized by the shell's "Show
/// Desktop" gesture (or, incidentally, by any other route to SC_MINIMIZE —
/// there's no legitimate reason for a desktop widget to be minimizable at
/// all). Independent of `attach`: this works whether or not the WorkerW
/// reparenting above succeeded, since it only touches the widget's own
/// window procedure, not its parentage. Best-effort like `attach`, for the
/// same reason — a failed guard leaves the widget as a normal (still
/// not-always-on-top) window rather than aborting the push.
pub fn guard_against_show_desktop(window: &WebviewWindow) {
    let hwnd = match window.hwnd() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[DesktopLayer] couldn't get the widget's HWND for the show-desktop guard: {e}");
            return;
        }
    };

    unsafe {
        let previous = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_wndproc as usize as isize);
        if previous == 0 {
            eprintln!(
                "[DesktopLayer] failed to install the show-desktop guard (SetWindowLongPtrW returned 0)"
            );
            return;
        }
        ORIGINAL_WNDPROC.store(previous, Ordering::SeqCst);
    }
}
