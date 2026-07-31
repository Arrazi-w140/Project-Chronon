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
  let resizeFrame = null;
  let lastWindowSize = "";
  const importedFontFaces = new Map();

  function importedFontSource(font) {
    const convertFileSrc = tauri && tauri.core && tauri.core.convertFileSrc;
    if (!convertFileSrc || !font.path) return null;
    return `url("${convertFileSrc(font.path)}") format("${font.format}")`;
  }

  async function registerImportedFont(font) {
    if (importedFontFaces.has(font.family)) return;
    const source = importedFontSource(font);
    if (!source || typeof FontFace === "undefined") return;

    try {
      const fontFace = new FontFace(font.family, source);
      await fontFace.load();
      document.fonts.add(fontFace);
      importedFontFaces.set(font.family, fontFace);
    } catch (err) {
      console.error(`Failed to register imported font ${font.name}`, err);
    }
  }

  function settingsUseUnregisteredImportedFont(settings) {
    return (settings.rows || []).some((row) => {
      const fonts = [row.textFont, row.numberFont, row.font];
      return fonts.some((font) => {
        const match = typeof font === "string" && font.match(/"(ChrononImported-[^"]+)"/);
        return match && !importedFontFaces.has(match[1]);
      });
    });
  }

  async function ensureImportedFonts(settings) {
    if (!invoke || !settingsUseUnregisteredImportedFont(settings)) return;
    try {
      const fonts = await invoke("list_imported_fonts");
      await Promise.all(fonts.map(registerImportedFont));
    } catch (err) {
      console.error("Failed to load imported fonts", err);
    }
  }

  // The root is transformed as one unit, so its visual bounds are the exact
  // dimensions the transparent native window needs. Keeping the window in
  // sync prevents the large end of the range from being clipped.
  function fitWindowToWidget() {
    if (!invoke || resizeFrame !== null) return;
    resizeFrame = requestAnimationFrame(() => {
      resizeFrame = null;
      const bounds = root.getBoundingClientRect();
      const width = Math.max(1, Math.ceil(bounds.width));
      const height = Math.max(1, Math.ceil(bounds.height));
      const sizeKey = `${width}x${height}`;
      if (sizeKey === lastWindowSize) return;

      lastWindowSize = sizeKey;
      invoke("set_widget_size", { width, height }).catch((err) => {
        lastWindowSize = "";
        console.error("Failed to resize desktop widget", err);
      });
    });
  }

  async function applySettings(settings) {
    if (!settings) return;
    await ensureImportedFonts(settings);
    currentSettings = settings;
    renderWidget(root, currentSettings);
    fitWindowToWidget();
    restartTick();
  }

  function restartTick() {
    clearTimeout(tickTimer);
    const tick = () => {
      if (currentSettings) {
        renderWidget(root, currentSettings);
        fitWindowToWidget();
      }
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
