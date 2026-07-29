// =========================================================================
// widget.js — controller for the standalone desktop widget window
// (widget.html). This window is created and torn down by Rust
// (src-tauri/src/widget_window.rs) independently of the editor window, so
// this file has no knowledge of the editor's DOM — it only knows how to
// receive a settings object and render it via the shared renderWidget()
// from widget-renderer.js.
//
// Config flow:
//   1. On load, pull whatever config Rust currently has via
//      `get_widget_config` (covers the normal "just got created" case
//      without racing the push event below).
//   2. Afterwards, listen for "widget-config-update" events for live
//      updates pushed from the editor while this window stays open.
// The widget ticks its own clock independently, so it keeps showing the
// correct time even if the editor window is closed.
// =========================================================================

(function () {
  const tauri = window.__TAURI__;
  const invoke = tauri && tauri.core && tauri.core.invoke;
  const listen = tauri && tauri.event && tauri.event.listen;

  const root = document.getElementById("widgetRoot");
  let currentSettings = null;
  let tickTimer = null;

  function applySettings(settings) {
    if (!settings) return;
    currentSettings = settings;
    renderWidget(root, currentSettings);
    restartTick();
  }

  function restartTick() {
    clearTimeout(tickTimer);
    const tick = () => {
      if (currentSettings) renderWidget(root, currentSettings);
      tickTimer = setTimeout(tick, needsSecondTicks(currentSettings) ? 1000 : 1000 * 30);
    };
    tick();
  }

  async function init() {
    if (listen) {
      listen("widget-config-update", (event) => applySettings(event.payload));
    }

    if (!invoke) {
      console.warn("Tauri APIs unavailable — widget.html must run inside the Tauri app.");
      return;
    }

    try {
      const initial = await invoke("get_widget_config");
      applySettings(initial);
    } catch (err) {
      console.error("Failed to load initial widget config", err);
    }
  }

  window.addEventListener("DOMContentLoaded", init);
})();
