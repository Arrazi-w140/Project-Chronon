// src-tauri/src/desktop_layer.rs
//
// Windows-only. Makes the widget behave like an actual desktop layer
// (Rainmeter/Wallpaper Engine style) instead of a normal application
// window.
//
// ---------------------------------------------------------------------
// WHY THE PREVIOUS FIX (blocking SC_MINIMIZE) DIDN'T WORK
// ---------------------------------------------------------------------
// "Show Desktop" -- the taskbar corner button, Win+D, and the three-finger
// touchpad swipe -- does NOT send WM_SYSCOMMAND/SC_MINIMIZE to the windows
// it's hiding. That message only exists for a window's *own* system menu,
// title-bar button, or Alt+Space -> Minimize. Explorer's Show Desktop
// instead walks its own tracked window list and calls ShowWindow()/
// SetWindowPlacement() on each HWND directly from outside the process --
// which drives the same window straight through WM_WINDOWPOSCHANGING,
// WM_WINDOWPOSCHANGED, and WM_SIZE(SIZE_MINIMIZED), but *never* generates
// a WM_SYSCOMMAND at all. That's why swallowing SC_MINIMIZE in our own
// WndProc was a no-op: it was intercepting a message that was never being
// sent in this scenario. This file no longer does that -- see
// `install_message_logger` below, which replaces it with an *unmodified*
// passthrough logger so we can see, from the field, exactly which
// messages really do arrive.
//
// ---------------------------------------------------------------------
// THE ACTUAL FIX: make the widget structurally NOT a top-level window
// ---------------------------------------------------------------------
// Rainmeter/Wallpaper-Engine-style widgets are immune to Show Desktop for
// a structural reason, not a message-filtering one: they aren't part of
// the population Explorer walks when it minimizes everything, because
// they're genuinely a *child* window of the desktop's WorkerW, not a
// top-level window that merely happens to render underneath everything
// else. `attach()` below does the reparenting, but two things in the
// previous version of this file meant that reparenting wasn't actually
// producing that structural result on every machine:
//
//   1. `SetParent()` moves a window in the parent/child hierarchy, but it
//      deliberately does NOT flip the WS_CHILD/WS_POPUP style bits --
//      this is called out explicitly in Microsoft's own docs. Without
//      also setting WS_CHILD (and clearing WS_POPUP) ourselves, the
//      widget was reparented in name only: still styled like an
//      independent popup window, which is very likely why Explorer's
//      shell hooks kept tracking it as something to manage. `attach()`
//      now corrects the style bits immediately after SetParent, then
//      calls SetWindowPos(..., SWP_FRAMECHANGED) -- MSDN requires that
//      call for a style change made via SetWindowLongPtr to actually take
//      effect.
//   2. The widget window was created *visible* and only reparented a
//      moment later (see widget_window.rs). In that gap it briefly
//      existed as a completely normal, independent top-level window, and
//      Explorer's shell hooks (which register new top-level windows for
//      Alt+Tab / Task View / Show-Desktop tracking as soon as they become
//      visible) can pick it up right then -- a later SetParent doesn't
//      retroactively un-register it. widget_window.rs now builds the
//      window hidden, finishes the WorkerW attach + style fix here, and
//      only shows it after that's done, so Explorer never sees it exist
//      as an independent window in the first place.
//
// `attach()` also now handles a second, unrelated failure mode: on some
// Windows 10/11 builds, Explorer parents the desktop icons
// ("SHELLDLL_DefView") directly under Progman with no separate WorkerW at
// all, which the previous EnumWindows-only search couldn't find -- it
// would silently bail, leaving the widget as a normal, unparented window
// with no protection whatsoever. See the comments in `attach()` for the
// fallback.
//
// ---------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------
// `install_message_logger` subclasses the widget's WndProc purely to log
// -- it forwards every message to the original proc completely unchanged,
// it never blocks or rewrites anything. It logs WM_SHOWWINDOW,
// WM_WINDOWPOSCHANGING/CHANGED (decoded, including the SWP_ flags -- this
// is the one most likely to show exactly what Explorer is doing),
// WM_SIZE, WM_SYSCOMMAND, WM_COMMAND, WM_DISPLAYCHANGE, WM_ACTIVATE, and a
// few close neighbors (WM_NCACTIVATE, WM_ACTIVATEAPP, WM_MOVE) that are
// relevant to diagnosing visibility/z-order changes. This is meant to be
// temporary: once the real mechanism is confirmed from the logs, this can
// be trimmed back down or removed. Logging goes to both stderr (visible
// under `cargo tauri dev`) and OutputDebugStringW (visible via
// Sysinternals DebugView or an attached debugger even in a release build,
// which has no console since main.rs sets windows_subsystem = "windows").
//
// This is inherently fragile in the way any WorkerW-based trick is:
// 0x052C and the WorkerW arrangement are undocumented Explorer
// implementation details that could change between Windows versions, and
// an Explorer restart destroys the WorkerW our widget was parented into,
// silently stranding it. Every step below fails soft and logs clearly
// rather than panicking; `attach()` returns whether it actually stuck so
// the caller can at least log a loud warning when it didn't.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use tauri::WebviewWindow;
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, EnumWindows, FindWindowExW, FindWindowW, GetWindowLongPtrW, GWLP_WNDPROC,
    GWL_STYLE, HWND_BOTTOM, SendMessageTimeoutW, SetParent, SetWindowLongPtrW, SetWindowPos,
    SMTO_NORMAL, SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, WINDOWPOS, WM_ACTIVATE, WM_ACTIVATEAPP, WM_COMMAND, WM_DISPLAYCHANGE,
    WM_MOVE, WM_NCACTIVATE, WM_SHOWWINDOW, WM_SIZE, WM_SYSCOMMAND, WM_WINDOWPOSCHANGED,
    WM_WINDOWPOSCHANGING, WNDPROC, WS_CHILD, WS_POPUP,
};

/// Undocumented message that tells Progman to spawn the icon-layer WorkerW.
/// Ignored (no-op) if that WorkerW already exists.
const SPAWN_WORKERW: u32 = 0x052C;

// ---------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------

static START: OnceLock<Instant> = OnceLock::new();

/// Logs a line to both stderr and OutputDebugStringW, with a millisecond
/// timestamp relative to the first log call, so ordering and timing
/// around a Show Desktop trigger are easy to read back. See the module
/// doc comment for how to view this in a release build (no console).
fn log(msg: &str) {
    let start = START.get_or_init(Instant::now);
    let elapsed = start.elapsed().as_millis();
    let line = format!("[Chronon/DesktopLayer][{elapsed:>7}ms] {msg}");

    eprintln!("{line}");

    let mut wide: Vec<u16> = line.encode_utf16().collect();
    wide.push(0);
    unsafe {
        OutputDebugStringW(PCWSTR(wide.as_ptr()));
    }
}

/// Decodes the handful of SWP_ flags most relevant to a Show-Desktop
/// investigation (is the window being hidden/shown, is its z-order being
/// forced, is it being denied activation). Not exhaustive on purpose --
/// widen this if a future investigation needs the rest.
fn decode_swp_flags(flags: u32) -> String {
    let mut parts = Vec::new();
    if flags & SWP_HIDEWINDOW.0 != 0 {
        parts.push("HIDEWINDOW");
    }
    if flags & SWP_SHOWWINDOW.0 != 0 {
        parts.push("SHOWWINDOW");
    }
    if flags & SWP_NOACTIVATE.0 != 0 {
        parts.push("NOACTIVATE");
    }
    if flags & SWP_NOMOVE.0 != 0 {
        parts.push("NOMOVE");
    }
    if flags & SWP_NOSIZE.0 != 0 {
        parts.push("NOSIZE");
    }
    if flags & SWP_FRAMECHANGED.0 != 0 {
        parts.push("FRAMECHANGED");
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join("|"))
    }
}

// ---------------------------------------------------------------------
// Part 1: WorkerW reparenting
// ---------------------------------------------------------------------
//
// Z-order this produces, front (top) to back (bottom):
//   normal application windows  →  desktop icons  →  our widget  →  wallpaper

/// `EnumWindows` callback used as the fallback search: for each top-level
/// window, check whether it owns the desktop icons (a "SHELLDLL_DefView"
/// child). The one that does is not itself the layer we want -- its
/// *next sibling* "WorkerW" is the empty layer sitting just behind the
/// icons, which is what gets written into `lparam` (a pointer to an
/// `HWND` living on the caller's stack).
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

/// Reparents `window` into the desktop's WorkerW layer, corrects its
/// style bits so it's a *real* child window (not just structurally
/// reparented while still styled as a popup), and forces it to the back
/// of that parent's z-order so it stays behind the desktop icons. Returns
/// whether it actually stuck, so the caller knows whether the widget is
/// truly desktop-attached or just a normal (best-effort) window.
pub fn attach(window: &WebviewWindow) -> bool {
    let hwnd = match window.hwnd() {
        Ok(h) => h,
        Err(e) => {
            log(&format!("couldn't get the widget's HWND: {e}"));
            return false;
        }
    };

    unsafe {
        let progman = match FindWindowW(w!("Progman"), PCWSTR::null()) {
            Ok(h) => h,
            Err(e) => {
                log(&format!("Progman window not found: {e}"));
                return false;
            }
        };
        log(&format!("Progman = {progman:?}"));

        // Ask Explorer to spawn the icon-layer WorkerW. A one-second
        // timeout keeps a wedged/slow shell from hanging widget creation;
        // failure here isn't fatal on its own since the WorkerW may
        // already exist from a previous run.
        let _ = SendMessageTimeoutW(
            progman,
            SPAWN_WORKERW,
            WPARAM(0),
            LPARAM(0),
            SMTO_NORMAL,
            1000,
            None,
        );
        log("sent 0x052C to Progman (spawn icon-layer WorkerW if missing)");

        let mut target = HWND::default();

        // Case 1: on a growing share of Windows 10/11 builds, Explorer
        // parents SHELLDLL_DefView directly under Progman with no
        // separate WorkerW wrapping it at all. The old EnumWindows-only
        // search couldn't find a target in that case and would silently
        // bail, leaving the widget completely unprotected. Check for it
        // explicitly first.
        let progman_owns_icons =
            FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok();
        log(&format!("Progman owns SHELLDLL_DefView directly: {progman_owns_icons}"));

        if progman_owns_icons {
            if let Ok(worker) = FindWindowExW(None, Some(progman), w!("WorkerW"), PCWSTR::null()) {
                target = worker;
                log(&format!("found WorkerW as a sibling of Progman: {worker:?}"));
            }
        }

        // Case 2: the "classic" layout -- some other top-level window
        // (itself usually an earlier WorkerW) owns SHELLDLL_DefView, and
        // *its* sibling WorkerW is the empty layer we want.
        if target.0.is_null() {
            if EnumWindows(
                Some(find_icon_layer_sibling),
                LPARAM(&mut target as *mut HWND as isize),
            )
            .is_err()
            {
                log("EnumWindows failed while searching for the WorkerW layer");
                return false;
            }
            if !target.0.is_null() {
                log(&format!("found WorkerW via EnumWindows: {target:?}"));
            }
        }

        // Last resort: no distinct WorkerW exists at all. Parenting
        // directly into Progman still gets the structural result that
        // actually matters for Show Desktop (see the module doc comment)
        // -- it's no longer part of the top-level window population.
        // SetWindowPos(HWND_BOTTOM) below keeps it visually behind the
        // icons regardless of which of the three targets we ended up
        // using.
        if target.0.is_null() {
            log("no distinct WorkerW found anywhere; falling back to Progman itself as the parent");
            target = progman;
        }

        let previous_style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        log(&format!("style before reparent: 0x{previous_style:08X}"));

        if let Err(e) = SetParent(hwnd, Some(target)) {
            log(&format!("SetParent into {target:?} failed: {e}"));
            return false;
        }
        log(&format!("SetParent succeeded, new parent = {target:?}"));

        // SetParent alone does NOT flip the WS_CHILD/WS_POPUP style bits
        // -- Microsoft's own docs call this out explicitly: "SetParent
        // does not modify the WS_CHILD or WS_POPUP window styles of the
        // window whose parent is being changed." Without this, the
        // widget was reparented in name only while still being styled
        // like an independent popup window -- this correction is the
        // actual fix, not a Show-Desktop-specific workaround: it makes
        // the widget genuinely, structurally a child window, the same as
        // a Rainmeter skin is.
        let corrected_style = (previous_style & !WS_POPUP.0) | WS_CHILD.0;
        SetWindowLongPtrW(hwnd, GWL_STYLE, corrected_style as isize);
        log(&format!("style after correction: 0x{corrected_style:08X}"));

        // Per MSDN: after changing style bits with SetWindowLong(Ptr) you
        // must call SetWindowPos with SWP_FRAMECHANGED for the change to
        // take effect. Piggyback HWND_BOTTOM on the same call so the
        // widget also lands at the back of the new parent's child
        // z-order -- behind the icons -- regardless of which of the three
        // targets above we ended up using.
        if let Err(e) = SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        ) {
            log(&format!("SetWindowPos (frame refresh + z-order) failed: {e}"));
        } else {
            log("SetWindowPos (frame refresh + push to back of z-order) succeeded");
        }

        true
    }
}

// ---------------------------------------------------------------------
// Part 2: diagnostics (was: blocking SC_MINIMIZE — removed, see module
// doc comment for why that message was never the right thing to swallow)
// ---------------------------------------------------------------------

/// The widget's original window procedure, saved by
/// `install_message_logger` so the subclass below can forward every
/// message unchanged. A single slot is enough: there is only ever one
/// widget window alive at a time (see `widget_window::WIDGET_LABEL`), and
/// it's re-subclassed fresh each time a new widget window is created.
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

/// Replacement window procedure installed over the widget's own. Purely
/// diagnostic: it logs the messages listed in the module doc comment with
/// their decoded parameters, then forwards *every* message, unmodified,
/// to the original proc. It never intercepts, swallows, or reacts to
/// anything -- if the widget still gets minimized/hidden, this log is
/// what tells us the real mechanism instead of guessing again.
unsafe extern "system" fn logging_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SHOWWINDOW => {
            let reason = match lparam.0 {
                0 => "ShowWindow/CreateWindow call by the app itself".to_string(),
                1 => "SW_PARENTCLOSING".to_string(),
                2 => "SW_OTHERZOOM".to_string(),
                3 => "SW_PARENTOPENING".to_string(),
                4 => "SW_OTHERUNZOOM".to_string(),
                other => format!("unknown ({other})"),
            };
            log(&format!(
                "WM_SHOWWINDOW      being_shown={} reason={reason}",
                wparam.0 != 0
            ));
        }
        WM_WINDOWPOSCHANGING | WM_WINDOWPOSCHANGED => {
            let name = if msg == WM_WINDOWPOSCHANGING {
                "WM_WINDOWPOSCHANGING"
            } else {
                "WM_WINDOWPOSCHANGED "
            };
            if lparam.0 != 0 {
                let pos = &*(lparam.0 as *const WINDOWPOS);
                log(&format!(
                    "{name}  x={} y={} cx={} cy={} insertAfter={:?} flags=0x{:08X}{}",
                    pos.x,
                    pos.y,
                    pos.cx,
                    pos.cy,
                    pos.hwndInsertAfter,
                    pos.flags.0,
                    decode_swp_flags(pos.flags.0)
                ));
            }
        }
        WM_SIZE => {
            let kind = match wparam.0 as u32 {
                0 => "SIZE_RESTORED",
                1 => "SIZE_MINIMIZED",
                2 => "SIZE_MAXIMIZED",
                3 => "SIZE_MAXSHOW",
                4 => "SIZE_MAXHIDE",
                _ => "unknown",
            };
            let raw = lparam.0 as u32;
            let width = raw & 0xFFFF;
            let height = (raw >> 16) & 0xFFFF;
            log(&format!("WM_SIZE            {kind} ({width}x{height})"));
        }
        WM_SYSCOMMAND => {
            let cmd = (wparam.0 as u32) & 0xFFF0;
            let name = match cmd {
                0xF020 => "SC_MINIMIZE",
                0xF120 => "SC_RESTORE",
                0xF030 => "SC_MAXIMIZE",
                0xF060 => "SC_CLOSE",
                0xF010 => "SC_MOVE",
                0xF000 => "SC_SIZE",
                _ => "other",
            };
            log(&format!("WM_SYSCOMMAND      {name} (0x{cmd:04X})"));
        }
        WM_COMMAND => {
            let notify = (wparam.0 as u32) >> 16;
            let id = (wparam.0 as u32) & 0xFFFF;
            log(&format!(
                "WM_COMMAND         notify=0x{notify:04X} id={id} lparam=0x{:08X}",
                lparam.0
            ));
        }
        WM_DISPLAYCHANGE => {
            let raw = lparam.0 as u32;
            let width = raw & 0xFFFF;
            let height = (raw >> 16) & 0xFFFF;
            log(&format!(
                "WM_DISPLAYCHANGE   {width}x{height} @ {}bpp",
                wparam.0
            ));
        }
        WM_ACTIVATE => {
            let state = match (wparam.0 as u32) & 0xFFFF {
                0 => "WA_INACTIVE",
                1 => "WA_ACTIVE",
                2 => "WA_CLICKACTIVE",
                _ => "unknown",
            };
            let minimized = ((wparam.0 as u32) >> 16) != 0;
            log(&format!(
                "WM_ACTIVATE        {state} other_window_minimized={minimized}"
            ));
        }
        WM_NCACTIVATE => {
            log(&format!("WM_NCACTIVATE      active={}", wparam.0 != 0));
        }
        WM_ACTIVATEAPP => {
            log(&format!(
                "WM_ACTIVATEAPP     activated={} other_thread_id={}",
                wparam.0 != 0,
                lparam.0
            ));
        }
        WM_MOVE => {
            let raw = lparam.0 as u32;
            let x = (raw & 0xFFFF) as i16;
            let y = ((raw >> 16) & 0xFFFF) as i16;
            log(&format!("WM_MOVE            x={x} y={y}"));
        }
        _ => {}
    }

    let original: WNDPROC = std::mem::transmute(ORIGINAL_WNDPROC.load(Ordering::SeqCst));
    CallWindowProcW(original, hwnd, msg, wparam, lparam)
}

/// Subclasses `window` purely for diagnostic logging -- see the module
/// doc comment. Unlike the SC_MINIMIZE guard this replaces, it never
/// changes the widget's behavior; every message is forwarded to the
/// original proc unmodified. Independent of `attach`: install this
/// whether or not the WorkerW reparenting succeeded, since a big part of
/// the point is to see what happens to the widget in *both* cases.
pub fn install_message_logger(window: &WebviewWindow) {
    let hwnd = match window.hwnd() {
        Ok(h) => h,
        Err(e) => {
            log(&format!(
                "couldn't get the widget's HWND for the message logger: {e}"
            ));
            return;
        }
    };

    unsafe {
        let previous = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, logging_wndproc as usize as isize);
        if previous == 0 {
            log("failed to install the message logger (SetWindowLongPtrW returned 0)");
            return;
        }
        ORIGINAL_WNDPROC.store(previous, Ordering::SeqCst);
    }

    log("message logger installed -- trigger Show Desktop now (corner click / Win+D / three-finger swipe) and watch this log");
}
