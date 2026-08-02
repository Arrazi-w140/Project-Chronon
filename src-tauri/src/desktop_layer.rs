// src-tauri/src/desktop_layer.rs
//
// Windows-only. Makes the widget behave like Rainmeter/Wallpaper-Engine
// style "desktop layer" content: always visible, immune to Show Desktop
// (Win+D / taskbar corner / three-finger touchpad swipe), absent from
// Alt+Tab and Task View, and stacked above the wallpaper but below the
// desktop icons and every normal application window.
//
// =====================================================================
// WHY THE PREVIOUS (WorkerW SetParent) APPROACH WAS REPLACED
// =====================================================================
// The previous version of this file reparented the widget into the
// desktop's WorkerW using SetParent(), then corrected the WS_CHILD/
// WS_POPUP style bits SetParent() deliberately leaves untouched. That
// was a reasonable-sounding theory -- "make it a real child window,
// like Rainmeter" -- but it does not match how Rainmeter (or any other
// long-lived Windows desktop-widget tool) actually works, and it comes
// with failure modes the previous version's own comments already flagged
// as risks without being able to name why they mattered:
//
//   - Rainmeter's own maintainer has stated directly that there is no
//     structural trick that survives Show Desktop:
//       "There is no magical flag or winapi function that achieves this.
//        The solution we came up with is to hook the EVENT_SYSTEM_FOREGROUND
//        event, then manually set all the skins['] z-pos to above the
//        desktop window. We also utilize a timer to periodically check
//        the desktop state."
//       -- rainmeter/rainmeter#339 (github.com/rainmeter/rainmeter/issues/339),
//          answered by Rainmeter core developer brianferguson.
//     Rainmeter's actual source confirms this: Skin::Initialize() in
//     Library/Skin.cpp creates every skin as an ordinary, UNPARENTED,
//     WS_POPUP top-level window:
//         m_Window = CreateWindowEx(
//             WS_EX_LAYERED | WS_EX_TOOLWINDOW,
//             METERWINDOW_CLASS_NAME, nullptr, WS_POPUP,
//             CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
//             nullptr /* no parent */, nullptr, ..., this);
//     There is no SetParent() call on a skin window anywhere in
//     Rainmeter's source. Immunity to Show Desktop comes entirely from
//     System::Initialize() in Library/System.cpp installing
//     SetWinEventHook(EVENT_SYSTEM_FOREGROUND, ...) plus a 100-250ms
//     polling SetTimer, both of which drive System::ChangeZPosInOrder()
//     / Skin::ChangeZPos() to re-assert each skin's z-order via
//     SetWindowPos every time the desktop state changes or the timer
//     fires. It is a continuously self-healing z-order defense, not a
//     one-time structural placement.
//
//   - Reparenting into WorkerW is the technique used by *wallpaper*
//     replacement tools (content that lives in the same plane as the
//     wallpaper itself and never needs its own z-order or input focus),
//     not by interactive widget/skin tools. Using it for an interactive
//     widget inherits wallpaper-tool fragility for no benefit:
//       - A WS_CHILD window's on-screen visibility is gated by
//         IsWindowVisible()'s ancestor-chain rule: "If the window has
//         the WS_VISIBLE style, but its parent [...] is not visible, the
//         window is not visible either" (IsWindowVisible, learn.microsoft.com).
//         If Explorer hides, destroys, or recreates the WorkerW the
//         widget was parented into -- which is exactly the kind of thing
//         that can happen around Show Desktop and is *undocumented*
//         Explorer-internal behavior that has changed across Windows
//         versions -- the widget can disappear or be silently destroyed
//         without its own WndProc ever receiving WM_SHOWWINDOW,
//         WM_WINDOWPOSCHANGING, or any other per-window notification
//         about itself. That is consistent with the reported symptom
//         (widget vanishes on Show Desktop, reappears when the desktop
//         is restored, with no corresponding message the widget's own
//         subclass could see) -- see the investigation notes
//         accompanying this rewrite for the full reasoning. This is
//         presented as the best-supported explanation consistent with
//         documented Win32 behavior, not as something empirically
//         confirmed against a captured log from the reporter's machine;
//         the instrumented logger below is what makes that distinction
//         checkable.
//       - Rainmeter's own source shows the desktop shell's internal
//         window hierarchy changed in Windows 11 24H2 (Progman now owns
//         SHELLDLL_DefView directly, with no separate WorkerW wrapping
//         it -- see the comment above GetDesktopIconsHostWindow() in
//         Library/System.cpp). A hierarchy an app has SetParent()-ed
//         into is exactly the kind of thing that silently breaks when
//         Explorer changes this internal, undocumented structure.
//
// =====================================================================
// THE REPLACEMENT: match Rainmeter's actual architecture
// =====================================================================
// The widget stays a genuine, unparented, top-level window (tao/Tauri
// already create it that way; this file no longer calls SetParent() at
// all). Three independent, documented mechanisms are layered on top,
// mirroring Rainmeter's Skin::Initialize() / System::Initialize() /
// System::ChangeZPosInOrder() exactly:
//
//   1. WS_EX_TOOLWINDOW (+ clearing WS_EX_APPWINDOW) on the widget's own
//      HWND. Documented (learn.microsoft.com, Extended Window Styles):
//      "A tool window does not appear in the taskbar or in the dialog
//      that appears when the user presses ALT+TAB." This is the same
//      extended style Rainmeter passes to CreateWindowEx for every skin.
//      Handles Alt+Tab and Task View exclusion. NOTE: this file's
//      previous version relied on Tauri's `.skip_taskbar(true)` builder
//      option for taskbar/Alt-Tab exclusion. Inspecting tao 0.35.3 (the
//      windowing crate under tauri 2.11.5) shows that option calls
//      `ITaskbarList::DeleteTab` (see set_skip_taskbar in
//      tao-0.35.3/src/platform_impl/windows/window.rs) -- a COM call
//      that removes the taskbar *button* -- and never sets
//      WS_EX_TOOLWINDOW anywhere (confirmed by reading
//      WindowFlags::to_window_styles() in
//      tao-0.35.3/src/platform_impl/windows/window_state.rs end to end).
//      So the widget's window style was, and until this change remained,
//      a completely ordinary top-level window's style. This file now
//      sets WS_EX_TOOLWINDOW directly with SetWindowLongPtrW, since
//      neither tao's cross-platform builder nor its
//      WindowBuilderExtWindows extension trait exposes it.
//   2. DWMWA_EXCLUDED_FROM_PEEK via DwmSetWindowAttribute. This is the
//      same DWM call Rainmeter makes from Skin::IgnoreAeroPeek() in
//      Library/Skin.cpp, right after CreateWindowEx. It excludes the
//      window from the Aero Peek preview effect that previews the
//      desktop when the user hovers the Show Desktop button.
//   3. An active, continuously self-healing z-order defense, matching
//      Rainmeter's System::Initialize()/ChangeZPosInOrder():
//        - SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
//          ...) fires whenever the foreground window changes -- which
//          includes the moment Show Desktop hands the foreground to the
//          desktop -- and immediately re-applies the widget's z-order.
//        - A 250ms polling SetTimer (same interval Rainmeter uses for
//          its TIMER_SHOWDESKTOP in the non-desktop-shown state; see
//          Library/System.cpp) re-applies it on a fixed cadence too, as
//          a self-healing fallback that does not depend on catching
//          every relevant event. Unlike Rainmeter, this file does not
//          replicate Rainmeter's separate FindWindowEx-based show-desktop
//          *state detector* (System::CheckDesktopState(), which looks
//          for Rainmeter's own marker window positioned immediately
//          behind the desktop-icons host in z-order). Instead it
//          unconditionally re-pins on every hook fire and every timer
//          tick. This is a deliberate simplification, not an oversight:
//          the re-pin (SetWindowPos with SWP_NOMOVE | SWP_NOSIZE |
//          SWP_NOACTIVATE | SWP_NOOWNERZORDER) is a no-op in terms of
//          visible effect when the widget is already correctly
//          positioned, so doing it unconditionally costs one extra
//          syscall per tick/event instead of adding a second detection
//          system that would itself need to be kept correct across
//          Windows versions.
//      Both the hook and the timer run a fresh lookup of the current
//      desktop-icons-host window each time (see find_desktop_icon_host),
//      rather than caching the HWND once, specifically because that
//      window can be destroyed and recreated by Explorer (resolution
//      changes, monitor topology changes, Explorer restarts) -- caching
//      it would reproduce the exact staleness problem the WorkerW
//      SetParent approach had.
//
// The z-order target itself (find_desktop_icon_host) mirrors Rainmeter's
// System::GetDesktopIconsHostWindow(): find the WorkerW/Progman window
// that owns the SHELLDLL_DefView desktop-icons view, and use
// SetWindowPos(widget, insertAfter = that window, ...) to keep the widget
// directly behind the icons in z-order -- i.e. above the wallpaper,
// below the icons, and (because it is never given WS_EX_TOPMOST) below
// every normal application window. If no such window is found, it falls
// back to HWND_BOTTOM, which is a strictly weaker guarantee (bottom of
// the *entire* top-level z-order, not specifically "just behind the
// icons") but still keeps the widget out from in front of application
// windows.
//
// =====================================================================
// Diagnostics
// =====================================================================
// `install_message_logger` subclasses the widget's WndProc purely to
// log -- it forwards every message to the original proc completely
// unchanged. It decodes WM_SHOWWINDOW, WM_WINDOWPOSCHANGING/CHANGED
// (including SWP_ flags), WM_PARENTNOTIFY, WM_STYLECHANGING/CHANGED
// (including which GWL_ index and the before/after style bits),
// WM_SIZE, WM_MOVE, WM_SYSCOMMAND, WM_COMMAND, WM_ACTIVATE,
// WM_ACTIVATEAPP, WM_DISPLAYCHANGE, WM_DESTROY, and WM_NCACTIVATE.
//
// One honest limitation: WM_CREATE cannot be observed this way. The
// subclass is installed with SetWindowLongPtrW *after* the window
// already exists (see install(), called from widget_window.rs once the
// window is built), and WM_CREATE is sent synchronously during
// CreateWindowEx, before that point -- there is no message queue
// involved for it to arrive late. Observing it would require a
// class-level default-procedure replacement or a CBT hook installed
// before the window is created, which is disproportionate to what this
// investigation needs; window-creation success/failure is already
// covered by widget_window.rs's own logging around `.build()`. This
// file logs a line at install() time instead so the gap is visible in
// the log rather than silently absent.
//
// Logging goes to both stderr (visible under `cargo tauri dev`) and
// OutputDebugStringW (visible via Sysinternals DebugView or an attached
// debugger even in a release build, which has no console since main.rs
// sets windows_subsystem = "windows").
//
// This is inherently fragile in the way any undocumented-Explorer-
// internals code is: 0x052C, WorkerW, and the exact SHELLDLL_DefView
// hierarchy are undocumented implementation details that have already
// changed once (Windows 11 24H2, per Rainmeter's own source comments)
// and could change again. Every step below fails soft and logs clearly
// rather than panicking.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use std::ffi::c_void;
use tauri::WebviewWindow;
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_EXCLUDED_FROM_PEEK};
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, EVENT_SYSTEM_FOREGROUND, FindWindowExW,
    FindWindowW, GWLP_WNDPROC, GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, HWND_BOTTOM,
    HWND_MESSAGE, KillTimer, PostQuitMessage, RegisterClassW, SWP_FRAMECHANGED, SWP_HIDEWINDOW,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_SHOWWINDOW,
    SetTimer, SetWindowLongPtrW, SetWindowPos, WINDOWPOS, WINDOW_EX_STYLE, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_ACTIVATE, WM_ACTIVATEAPP, WM_COMMAND, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_MOVE, WM_NCACTIVATE, WM_PARENTNOTIFY, WM_SHOWWINDOW, WM_SIZE,
    WM_STYLECHANGED, WM_STYLECHANGING, WM_SYSCOMMAND, WM_TIMER, WM_WINDOWPOSCHANGED,
    WM_WINDOWPOSCHANGING, WNDCLASSW, WNDPROC, WS_DISABLED, WS_EX_APPWINDOW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

/// Undocumented message that tells Progman to spawn the icon-layer WorkerW.
/// Ignored (no-op) if that WorkerW already exists. Kept from the previous
/// version of this file -- still needed so a widget pushed to the desktop
/// before the icon layer has ever been created (e.g. immediately after
/// Explorer starts) has something to find on the first lookup.
const SPAWN_WORKERW: u32 = 0x052C;

/// Matches Rainmeter's INTERVAL_SHOWDESKTOP (Library/System.cpp): the
/// polling interval for the self-healing z-order timer.
const GUARD_TIMER_INTERVAL_MS: u32 = 250;
const GUARD_TIMER_ID: usize = 1;

// ---------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------

static START: OnceLock<Instant> = OnceLock::new();

/// Logs a line to both stderr and OutputDebugStringW, with a millisecond
/// timestamp relative to the first log call, so ordering and timing
/// around a Show Desktop trigger are easy to read back.
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

/// Decodes the SWP_ flags relevant to a Show-Desktop investigation (is
/// the window being hidden/shown, is its z-order being forced, is it
/// being denied activation).
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

/// Decodes the GWL_STYLE bits most relevant to this investigation
/// (visibility, child/popup-ness) -- not exhaustive by design, see the
/// module doc comment's "decode every flag" note: only bits that matter
/// for diagnosing this specific class of bug are decoded, to keep the
/// log readable.
fn decode_style_bits(style: u32) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{WS_CHILD, WS_DISABLED as WSD, WS_POPUP as WSP, WS_VISIBLE};
    let mut parts = Vec::new();
    if style & WS_VISIBLE.0 != 0 {
        parts.push("VISIBLE");
    }
    if style & WS_CHILD.0 != 0 {
        parts.push("CHILD");
    }
    if style & WSP.0 != 0 {
        parts.push("POPUP");
    }
    if style & WSD.0 != 0 {
        parts.push("DISABLED");
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join("|"))
    }
}

/// Decodes the GWL_EXSTYLE bits this file cares about.
fn decode_exstyle_bits(exstyle: u32) -> String {
    let mut parts = Vec::new();
    if exstyle & WS_EX_TOOLWINDOW.0 != 0 {
        parts.push("TOOLWINDOW");
    }
    if exstyle & WS_EX_APPWINDOW.0 != 0 {
        parts.push("APPWINDOW");
    }
    if exstyle & WS_EX_TOPMOST.0 != 0 {
        parts.push("TOPMOST");
    }
    if exstyle & WS_EX_NOACTIVATE.0 != 0 {
        parts.push("NOACTIVATE");
    }
    if exstyle & WS_EX_LAYERED.0 != 0 {
        parts.push("LAYERED");
    }
    if exstyle & WS_EX_TRANSPARENT.0 != 0 {
        parts.push("TRANSPARENT");
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join("|"))
    }
}

// ---------------------------------------------------------------------
// Pointer-sized handle <-> AtomicIsize helpers
// ---------------------------------------------------------------------
// HWND / HWINEVENTHOOK are pointer-sized newtype handles. This mirrors
// the storage pattern the previous version of this file already used
// for ORIGINAL_WNDPROC (`SetWindowLongPtrW(...) as isize`, restored via
// `std::mem::transmute`) rather than assuming a specific internal
// representation for the handle type, which has changed between
// windows-rs releases.

fn store_handle<T: Copy>(cell: &AtomicIsize, handle: T) {
    debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<isize>());
    let bits: isize = unsafe { std::mem::transmute_copy(&handle) };
    cell.store(bits, Ordering::SeqCst);
}

fn load_handle<T: Copy>(cell: &AtomicIsize) -> T {
    debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<isize>());
    let bits = cell.load(Ordering::SeqCst);
    unsafe { std::mem::transmute_copy(&bits) }
}

/// The widget's HWND, cached so the WinEventHook callback and the
/// helper window's WM_TIMER handler (both of which run without a
/// reference to the WebviewWindow) can re-pin it. 0 means "no widget
/// currently installed".
static WIDGET_HWND: AtomicIsize = AtomicIsize::new(0);

/// The helper (message-only-parented) window that owns the WinEventHook
/// and the polling timer -- mirrors Rainmeter's System::c_Window.
static HELPER_HWND: AtomicIsize = AtomicIsize::new(0);

/// The active WinEventHook, so it can be unhooked on teardown.
static WIN_EVENT_HOOK: AtomicIsize = AtomicIsize::new(0);

// ---------------------------------------------------------------------
// Desktop-icons-host lookup (z-order reference point only -- never a
// SetParent target; see the module doc comment)
// ---------------------------------------------------------------------

/// `EnumWindows` callback: fallback search for the classic (pre-24H2)
/// layout, where some other top-level window (itself usually an
/// earlier WorkerW) owns SHELLDLL_DefView, and *its* sibling WorkerW is
/// the one we want as a z-order reference.
unsafe extern "system" fn find_icon_layer_sibling(
    hwnd: HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    let owns_icons =
        FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok();

    if owns_icons {
        if let Ok(worker) = FindWindowExW(None, Some(hwnd), w!("WorkerW"), PCWSTR::null()) {
            let out = &mut *(lparam.0 as *mut HWND);
            *out = worker;
            return BOOL(0); // found it -- stop enumerating
        }
    }

    BOOL(1) // keep looking
}

/// Finds the window that currently hosts the desktop icons
/// (SHELLDLL_DefView), to use purely as a z-order reference point for
/// SetWindowPos -- this file never calls SetParent. Mirrors Rainmeter's
/// System::GetDesktopIconsHostWindow() (Library/System.cpp), including
/// its Windows 11 24H2 special case, where Explorer stopped wrapping
/// SHELLDLL_DefView in a separate WorkerW and parents it directly under
/// Progman instead.
///
/// Re-resolved from scratch on every call (by the WinEventHook callback
/// and by the polling timer) rather than cached, because this window can
/// be destroyed and recreated by Explorer -- caching it would reproduce
/// the staleness problem the previous WorkerW SetParent approach had.
fn find_desktop_icon_host() -> Option<HWND> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), PCWSTR::null()).ok()?;

        // Windows 11 24H2+: Progman owns SHELLDLL_DefView directly.
        if FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok() {
            if let Ok(worker) = FindWindowExW(None, Some(progman), w!("WorkerW"), PCWSTR::null())
            {
                return Some(worker);
            }
            return Some(progman);
        }

        // Classic layout: ask Progman to spawn the icon WorkerW if it
        // doesn't exist yet, then search for the sibling-of-the-icon-
        // owner pattern.
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(
            progman,
            SPAWN_WORKERW,
            WPARAM(0),
            LPARAM(0),
            windows::Win32::UI::WindowsAndMessaging::SMTO_NORMAL,
            1000,
            None,
        );

        let mut target = HWND::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(find_icon_layer_sibling),
            LPARAM(&mut target as *mut HWND as isize),
        );
        if !target.0.is_null() {
            Some(target)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------
// Part 1: static shell exclusions (Alt+Tab / Task View / taskbar / peek)
// ---------------------------------------------------------------------

/// Sets WS_EX_TOOLWINDOW (clearing WS_EX_APPWINDOW if present) and
/// DWMWA_EXCLUDED_FROM_PEEK on the widget's own HWND. Both are the exact
/// mechanisms Rainmeter uses for every skin (CreateWindowEx's
/// WS_EX_TOOLWINDOW flag and Skin::IgnoreAeroPeek()'s DwmSetWindowAttribute
/// call, both in Library/Skin.cpp) -- see the module doc comment for the
/// documentation citations. Neither of these makes the widget immune to
/// Show Desktop by itself (see install()); they only cover taskbar /
/// Alt+Tab / Task View / Aero-Peek-preview exclusion.
///
/// Must run before the window is first shown so it is never, even
/// momentarily, visible without these styles -- widget_window.rs builds
/// the window with `.visible(false)` and calls this before `.show()`.
fn apply_shell_exclusions(hwnd: HWND) -> bool {
    unsafe {
        let previous = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        log(&format!(
            "GWL_EXSTYLE before exclusions: 0x{previous:08X}{}",
            decode_exstyle_bits(previous)
        ));

        let corrected = (previous | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, corrected as isize);

        // Per MSDN (SetWindowLongPtr remarks): some window data is
        // cached and changes are not reflected until SetWindowPos is
        // called with SWP_FRAMECHANGED. This applies to GWL_EXSTYLE the
        // same way the previous version of this file already correctly
        // applied it to GWL_STYLE.
        let pos_ok = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                | SWP_NOACTIVATE
                | SWP_FRAMECHANGED,
        )
        .is_ok();

        let after = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        log(&format!(
            "GWL_EXSTYLE after exclusions:  0x{after:08X}{} (SetWindowPos/FRAMECHANGED {})",
            decode_exstyle_bits(after),
            if pos_ok { "ok" } else { "FAILED" }
        ));

        let enabled: BOOL = BOOL(1);
        let dwm_result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_EXCLUDED_FROM_PEEK,
            &enabled as *const _ as *const c_void,
            std::mem::size_of::<BOOL>() as u32,
        );
        match &dwm_result {
            Ok(()) => log("DwmSetWindowAttribute(DWMWA_EXCLUDED_FROM_PEEK, TRUE) succeeded"),
            Err(e) => log(&format!(
                "DwmSetWindowAttribute(DWMWA_EXCLUDED_FROM_PEEK) failed: {e} \
                 (non-fatal: only affects the Aero Peek hover-preview effect, \
                 not Show Desktop itself)"
            )),
        }

        pos_ok && dwm_result.is_ok()
    }
}

// ---------------------------------------------------------------------
// Part 2: active z-order defense (the actual Show-Desktop immunity)
// ---------------------------------------------------------------------

/// Re-asserts the widget's z-order: directly behind the desktop-icons
/// host if one can be found (above the wallpaper, below the icons),
/// falling back to HWND_BOTTOM (bottom of the whole top-level z-order --
/// a weaker guarantee, but still never in front of an app window) if
/// not. Idempotent and cheap when the widget is already correctly
/// positioned, so it is safe to call unconditionally from both the
/// WinEventHook callback and the polling timer. Mirrors
/// Skin::ChangeZPos's `SetWindowPos(m_Window, insertAfter, ...)` calls
/// in Rainmeter's Library/Skin.cpp.
fn pin_to_desktop_layer(hwnd: HWND) -> bool {
    let icon_host = find_desktop_icon_host();
    let result = unsafe {
        match icon_host {
            Some(host) => SetWindowPos(
                hwnd,
                Some(host),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            ),
            None => SetWindowPos(
                hwnd,
                Some(HWND_BOTTOM),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            ),
        }
    };

    match &result {
        Ok(()) => log(&format!(
            "pin_to_desktop_layer: insertAfter={} ok",
            match icon_host {
                Some(h) => format!("{h:?} (desktop-icons host)"),
                None => "HWND_BOTTOM (icons host not found)".to_string(),
            }
        )),
        Err(e) => log(&format!("pin_to_desktop_layer: SetWindowPos failed: {e}")),
    }

    result.is_ok()
}

/// The helper window's WndProc. This window's only jobs are to own the
/// WM_TIMER that drives the polling half of the z-order defense and to
/// give the WinEventHook callback (below) a place to log from; it is
/// never shown and never receives input. Parented to HWND_MESSAGE so it
/// never appears in any window enumeration a shell component might do,
/// and never needs its own Show-Desktop defense.
unsafe extern "system" fn helper_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER if wparam.0 == GUARD_TIMER_ID => {
            let widget: HWND = load_handle(&WIDGET_HWND);
            if !widget.0.is_null() {
                pin_to_desktop_layer(widget);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// WinEventHook callback for EVENT_SYSTEM_FOREGROUND. Fires whenever the
/// foreground window changes system-wide, which includes the moment
/// Show Desktop hands the foreground to the desktop -- this is the
/// "immediate reaction" half of the defense; the polling timer above is
/// the fallback for anything this doesn't catch. Per MSDN (SetWinEventHook
/// remarks): "The client thread that calls SetWinEventHook must have a
/// message loop in order to receive events" -- this hook is installed
/// from Tauri's main thread in install() below, which already runs
/// tao's event loop (a real Win32 message loop), so no extra thread or
/// message pump is needed.
unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if event != EVENT_SYSTEM_FOREGROUND {
        return;
    }
    let widget: HWND = load_handle(&WIDGET_HWND);
    if widget.0.is_null() {
        return;
    }
    log("EVENT_SYSTEM_FOREGROUND observed -> re-pinning widget");
    pin_to_desktop_layer(widget);
}

/// Creates the hidden helper window and installs the WinEventHook and
/// polling timer. Returns false (logging why) if any part fails; the
/// widget still functions as a normal window in that case, just without
/// the active defense.
fn install_show_desktop_guard(widget_hwnd: HWND) -> bool {
    unsafe {
        let instance = match GetModuleHandleW(PCWSTR::null()) {
            Ok(h) => h,
            Err(e) => {
                log(&format!("GetModuleHandleW failed: {e}"));
                return false;
            }
        };

        let class_name = w!("ChrononShowDesktopGuard");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(helper_wndproc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        // Ignore the "class already registered" case (ERROR_CLASS_ALREADY_EXISTS):
        // that's expected if a widget was previously installed and torn down
        // in the same process lifetime.
        RegisterClassW(&wc);

        let helper = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("ChrononShowDesktopGuard"),
            WS_POPUP | WS_DISABLED,
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                log(&format!("CreateWindowExW (helper window) failed: {e}"));
                return false;
            }
        };
        log(&format!("helper window created: {helper:?} (parent=HWND_MESSAGE)"));
        store_handle(&HELPER_HWND, helper);

        if SetTimer(Some(helper), GUARD_TIMER_ID, GUARD_TIMER_INTERVAL_MS, None) == 0 {
            log("SetTimer failed for the show-desktop guard's polling timer");
        } else {
            log(&format!(
                "polling timer armed at {GUARD_TIMER_INTERVAL_MS}ms (self-healing fallback)"
            ));
        }

        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.0.is_null() {
            log("SetWinEventHook(EVENT_SYSTEM_FOREGROUND) failed (returned NULL) -- \
                 relying on the polling timer alone");
        } else {
            log("SetWinEventHook(EVENT_SYSTEM_FOREGROUND) installed");
            store_handle(&WIN_EVENT_HOOK, hook);
        }

        store_handle(&WIDGET_HWND, widget_hwnd);
        true
    }
}

/// Tears down the helper window, timer, and WinEventHook. Call this from
/// the widget window's Destroyed handler (see widget_window.rs) so a
/// second `push_widget` in the same process doesn't accumulate hooks or
/// timers, and so nothing keeps trying to re-pin a widget HWND that no
/// longer exists.
pub fn uninstall() {
    unsafe {
        let hook: HWINEVENTHOOK = load_handle(&WIN_EVENT_HOOK);
        if !hook.0.is_null() {
            let _ = UnhookWinEvent(hook);
            log("WinEventHook unhooked");
        }
        store_handle(&WIN_EVENT_HOOK, HWINEVENTHOOK::default());

        let helper: HWND = load_handle(&HELPER_HWND);
        if !helper.0.is_null() {
            let _ = KillTimer(Some(helper), GUARD_TIMER_ID);
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(helper);
            log("helper window destroyed, polling timer killed");
        }
        store_handle(&HELPER_HWND, HWND::default());
        store_handle(&WIDGET_HWND, HWND::default());
    }
}

/// Entry point called once, from widget_window.rs, right after the
/// widget window is built (while it is still hidden -- see the
/// module doc comment on apply_shell_exclusions). Applies the static
/// shell exclusions, does an initial z-order pin, and installs the
/// active defense. Returns whether every step succeeded; the caller
/// logs a warning if not, since the widget still works as a normal
/// window in that case, just without Rainmeter-equivalent guarantees.
pub fn install(window: &WebviewWindow) -> bool {
    let hwnd = match window.hwnd() {
        Ok(h) => h,
        Err(e) => {
            log(&format!("couldn't get the widget's HWND: {e}"));
            return false;
        }
    };

    log("install() starting -- note: WM_CREATE cannot be logged from here, \
         see the module doc comment for why");

    let exclusions_ok = apply_shell_exclusions(hwnd);
    let pin_ok = pin_to_desktop_layer(hwnd);
    let guard_ok = install_show_desktop_guard(hwnd);

    install_message_logger(window);

    exclusions_ok && pin_ok && guard_ok
}

// ---------------------------------------------------------------------
// Part 3: diagnostics -- passthrough message logger
// ---------------------------------------------------------------------

/// The widget's original window procedure, saved by
/// `install_message_logger` so the subclass below can forward every
/// message unchanged.
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

/// Replacement window procedure installed over the widget's own. Purely
/// diagnostic: it logs the messages listed in the module doc comment
/// with their decoded parameters, then forwards *every* message,
/// unmodified, to the original proc. As an explicit, narrow exception to
/// "purely diagnostic": on WM_DISPLAYCHANGE it also calls
/// pin_to_desktop_layer() as a corrective action (not an interception --
/// the message itself is still forwarded unchanged) since a display/
/// monitor-topology change is exactly the kind of event that can cause
/// Explorer to recreate the desktop-icons host window this file
/// positions against.
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
        WM_PARENTNOTIFY => {
            let event = wparam.0 as u32 & 0xFFFF;
            log(&format!(
                "WM_PARENTNOTIFY    event=0x{event:04X} (fires for a child HWND being \
                 created/destroyed/clicked -- expected from WebView2's own child window, \
                 not evidence of the widget itself being reparented)"
            ));
        }
        WM_STYLECHANGING | WM_STYLECHANGED => {
            let name = if msg == WM_STYLECHANGING {
                "WM_STYLECHANGING"
            } else {
                "WM_STYLECHANGED "
            };
            let which = if wparam.0 as i32 == GWL_STYLE.0 {
                "GWL_STYLE"
            } else if wparam.0 as i32 == GWL_EXSTYLE.0 {
                "GWL_EXSTYLE"
            } else {
                "unknown-index"
            };
            if lparam.0 != 0 {
                let styles = &*(lparam.0
                    as *const windows::Win32::UI::WindowsAndMessaging::STYLESTRUCT);
                let decode = if wparam.0 as i32 == GWL_EXSTYLE.0 {
                    decode_exstyle_bits as fn(u32) -> String
                } else {
                    decode_style_bits as fn(u32) -> String
                };
                log(&format!(
                    "{name}   index={which} old=0x{:08X}{} new=0x{:08X}{}",
                    styles.styleOld,
                    decode(styles.styleOld),
                    styles.styleNew,
                    decode(styles.styleNew)
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
                "WM_DISPLAYCHANGE   {width}x{height} @ {}bpp -> forcing a re-pin, since this \
                 can mean Explorer recreated the desktop-icons host",
                wparam.0
            ));
            pin_to_desktop_layer(hwnd);
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
        WM_DESTROY => {
            log("WM_DESTROY         widget window being destroyed");
        }
        _ => {}
    }

    let original: WNDPROC = std::mem::transmute(ORIGINAL_WNDPROC.load(Ordering::SeqCst));
    CallWindowProcW(original, hwnd, msg, wparam, lparam)
}

/// Subclasses `window` purely for diagnostic logging -- see the module
/// doc comment. Never changes the widget's behavior; every message is
/// forwarded to the original proc unmodified except for the one
/// documented, narrow exception on WM_DISPLAYCHANGE noted above
/// logging_wndproc.
fn install_message_logger(window: &WebviewWindow) {
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

    log("message logger installed -- trigger Show Desktop now (corner click / Win+D / \
         three-finger swipe) and watch this log");
}
