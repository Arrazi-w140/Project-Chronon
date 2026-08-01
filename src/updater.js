// ================================================================
// src/updater.js
// ----------------------------------------------------------------
// Settings > Updates. Talks to the commands registered in
// src-tauri/src/updater.rs (check_for_updates, install_update_now,
// get_app_version, set_auto_check_updates) through
// window.__TAURI__.core.invoke — the same bridge main.js already uses
// for everything else, since this project loads plain <script> files
// rather than going through a bundler (see the comment above main.js's
// own tauriBridge setup near the top of that file).
//
// The actual check runs on a background thread in Rust (see
// spawn_background_checks in updater.rs), so it keeps happening even
// while this window is closed and only the desktop widget remains open.
// This file just reflects whatever "update-status" event Rust last
// emitted, and lets the user trigger an on-demand check or install.
// ================================================================

(function () {
  const tauriBridge = window.__TAURI__;
  const tauriInvoke = tauriBridge && tauriBridge.core && tauriBridge.core.invoke;
  const tauriListen = tauriBridge && tauriBridge.event && tauriBridge.event.listen;

  const AUTO_CHECK_PREF_KEY = "chrononAutoCheckUpdates";

  const state = {
    status: { state: "idle" },
    version: null,
    lastChecked: null,
    installing: false,
  };

  function readAutoCheckPref() {
    const raw = localStorage.getItem(AUTO_CHECK_PREF_KEY);
    return raw === null ? true : raw === "1";
  }

  function writeAutoCheckPref(enabled) {
    localStorage.setItem(AUTO_CHECK_PREF_KEY, enabled ? "1" : "0");
  }

  function formatLastChecked(date) {
    if (!date) return "";
    const time = date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    return `Last checked ${time}`;
  }

  // Maps the current status + a couple of local flags to exactly what
  // the card should show. Kept as one pure function so render() never
  // has to duplicate this branching.
  function view() {
    const runningVersion = state.version ? `v${state.version}` : "";
    switch (state.status.state) {
      case "checking":
        return { dot: "checking", text: "Checking for updates…", btnText: "Checking…", btnDisabled: true };

      case "available":
        return {
          dot: "available",
          text: `Update available — v${state.status.version}`,
          btnText: state.installing ? "Installing…" : "Download & Install",
          btnDisabled: state.installing,
        };

      case "downloading":
        return {
          dot: "downloading",
          text: `Downloading update…${state.status.percent != null ? " " + state.status.percent + "%" : ""}`,
          btnText: "Downloading…",
          btnDisabled: true,
        };

      case "up-to-date":
        return {
          dot: "up-to-date",
          text: runningVersion ? `You're up to date (${runningVersion})` : "You're up to date",
          btnText: "Check for Updates Now",
          btnDisabled: false,
        };

      case "error":
        return {
          dot: "error",
          text: state.status.message || "Couldn't check for updates.",
          btnText: "Try Again",
          btnDisabled: false,
        };

      default:
        return {
          dot: "idle",
          text: runningVersion ? `Running ${runningVersion}` : "",
          btnText: "Check for Updates Now",
          btnDisabled: false,
        };
    }
  }

  function render() {
    const dot = document.getElementById("updateDot");
    const text = document.getElementById("updateStatusText");
    const btn = document.getElementById("updateActionBtn");
    const versionEl = document.getElementById("updateCurrentVersion");
    const lastCheckedEl = document.getElementById("updateLastChecked");
    if (!dot || !text || !btn) return;

    const v = view();
    dot.dataset.state = v.dot;
    text.textContent = v.text;
    btn.textContent = v.btnText;
    btn.disabled = v.btnDisabled;
    if (versionEl) versionEl.textContent = state.version ? `v${state.version}` : "—";
    if (lastCheckedEl) lastCheckedEl.textContent = formatLastChecked(state.lastChecked);
  }

  async function loadVersion() {
    if (!tauriInvoke) return;
    try {
      state.version = await tauriInvoke("get_app_version");
    } catch (err) {
      // Leave state.version null — render() just shows "—" for it.
      console.warn("[Updater] Couldn't read app version:", err);
    }
    render();
  }

  async function onActionClick() {
    if (!tauriInvoke) return;
    const v = view();
    if (v.btnDisabled) return;

    if (state.status.state === "available") {
      state.installing = true;
      render();
      try {
        await tauriInvoke("install_update_now");
        // On Windows the app exits partway through this call to hand
        // off to the installer, so nothing after this line runs in
        // practice on that platform — see the comment above
        // install_update_now in updater.rs.
      } catch (err) {
        state.status = {
          state: "error",
          message: "Couldn't download or install the update. Check your internet connection and try again.",
        };
        state.installing = false;
        render();
      }
      return;
    }

    state.status = { state: "checking" };
    render();
    const result = await tauriInvoke("check_for_updates");
    state.lastChecked = new Date();
    if (result && result.started === false) {
      state.status = { state: "error", message: result.reason || "Couldn't check for updates." };
    }
    // On success, the real outcome (available / up-to-date / error)
    // arrives shortly after via the "update-status" event listener
    // below — run_check() in updater.rs emits it either way.
    render();
  }

  function initToggle() {
    const toggle = document.getElementById("autoCheckUpdates");
    if (!toggle) return;

    const enabled = readAutoCheckPref();
    toggle.checked = enabled;
    if (tauriInvoke) tauriInvoke("set_auto_check_updates", { enabled }).catch(() => {});

    toggle.addEventListener("change", () => {
      writeAutoCheckPref(toggle.checked);
      if (tauriInvoke) tauriInvoke("set_auto_check_updates", { enabled: toggle.checked }).catch(() => {});
    });
  }

  function init() {
    if (!tauriInvoke) return; // e.g. index.html opened directly in a browser while tweaking CSS

    initToggle();
    loadVersion();

    const btn = document.getElementById("updateActionBtn");
    if (btn) btn.addEventListener("click", onActionClick);

    if (tauriListen) {
      tauriListen("update-status", (event) => {
        state.status = event.payload;
        if (state.status.state === "checking") state.lastChecked = new Date();
        if (state.status.state !== "available") state.installing = false;
        render();
      });
    }

    render();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
