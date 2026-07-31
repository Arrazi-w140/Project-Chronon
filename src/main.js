// =========================================================================
// Row configuration schema
//
// Each row is just a plain data object. To support a 4th, 5th, ... row in
// the future, add another entry to ROW_DEFAULTS — createRowCardEl() and the
// event delegation below already handle any number of rows without changes.
// =========================================================================

const DEFAULT_FONT = "system-ui, -apple-system, sans-serif";

// Out-of-the-box defaults: Michroma for words, JetBrains Mono for digits.
// Both are plain CSS font stacks, so unsupported glyphs (e.g. non-Latin
// scripts Michroma doesn't cover) fall back to the system UI font
// automatically, per-character, with no extra logic needed.
const DEFAULT_TEXT_FONT = "'Michroma', system-ui, sans-serif";
const DEFAULT_NUMBER_FONT = "'JetBrains Mono', 'Courier New', monospace";

const ROW_DEFAULTS = [
  { row: "1", type: "dayName", size: 48, color: "#FFFFFF", numberFont: DEFAULT_NUMBER_FONT, textFont: DEFAULT_TEXT_FONT, language: "en", align: "center" },
  { row: "2", type: "time24Sec", size: 32, color: "#D9A441", numberFont: DEFAULT_NUMBER_FONT, textFont: DEFAULT_TEXT_FONT, language: "en", align: "center" },
  { row: "3", type: "monthYear", size: 20, color: "#C7CCD1", numberFont: DEFAULT_NUMBER_FONT, textFont: DEFAULT_TEXT_FONT, language: "en", align: "center" },
];

const FONT_OPTIONS = [
  { value: "system-ui, -apple-system, sans-serif", label: "Default (System UI)" },
  { value: DEFAULT_TEXT_FONT, label: "Michroma" },
  { value: "'Inter', system-ui, sans-serif", label: "Inter" },
  { value: "'Roboto', system-ui, sans-serif", label: "Roboto" },
  { value: "'Segoe UI', sans-serif", label: "Segoe UI" },
  { value: "-apple-system, 'SF Pro Display', system-ui, sans-serif", label: "SF Pro Display" },
  { value: "'Poppins', system-ui, sans-serif", label: "Poppins" },
  { value: DEFAULT_NUMBER_FONT, label: "JetBrains Mono" },
  { value: "Arial, Helvetica, sans-serif", label: "Arial" },
  { value: "Georgia, serif", label: "Georgia" },
  { value: "'Courier New', monospace", label: "Courier New" },
];

// Quick-pick color presets shown under every color picker (General background
// + each row's text color). One shared list so every picker in the app stays
// visually consistent — add/remove a color here and it updates everywhere.
const PRESET_COLORS = [
  // Neutral
  { hex: "#FFFFFF", label: "White" },
  { hex: "#C7CCD1", label: "Light Gray" },
  { hex: "#4B5160", label: "Dark Gray" },
  { hex: "#0A0B0D", label: "Black" },
  // Warm
  { hex: "#D9A441", label: "Gold" },
  { hex: "#E8813A", label: "Orange" },
  { hex: "#F2B705", label: "Amber" },
  { hex: "#E5484D", label: "Red" },
  { hex: "#FF6F61", label: "Coral" },
  // Cool
  { hex: "#4FC3F7", label: "Sky Blue" },
  { hex: "#3B5BDB", label: "Royal Blue" },
  { hex: "#22D3EE", label: "Cyan" },
  { hex: "#14B8A6", label: "Teal" },
  { hex: "#6EE7B7", label: "Mint" },
  // Nature
  { hex: "#4CAF50", label: "Green" },
  { hex: "#10B981", label: "Emerald" },
  { hex: "#A3E635", label: "Lime" },
  { hex: "#A6A15C", label: "Olive" },
  // Purple
  { hex: "#8B5CF6", label: "Violet" },
  { hex: "#A855F7", label: "Purple" },
  { hex: "#C4B5FD", label: "Lavender" },
  { hex: "#D946EF", label: "Magenta" },
  { hex: "#F472B6", label: "Pink" },
];

function colorSwatchesHtml(colors = PRESET_COLORS) {
  return colors.map(
    (c) => `<button type="button" class="color-swatch" data-color="${c.hex}" style="background:${c.hex}" title="${c.label}" aria-label="${c.label}"></button>`
  ).join("");
}

// Curated subset of PRESET_COLORS used for the Row cards' Quick Colors only
// (General's background swatches keep the full list above). Kept short so
// it always renders as a single row: one neutral pair, the app's own accent
// gold, then one warm/cool/green/purple pick for broad, balanced coverage.
const ROW_QUICK_COLORS = ["#FFFFFF", "#0A0B0D", "#D9A441", "#E5484D", "#4FC3F7", "#10B981", "#A855F7"]
  .map((hex) => PRESET_COLORS.find((c) => c.hex === hex));

// Same idea as ROW_QUICK_COLORS, but for the General card's background
// color field — kept to 6 so it always renders as a single, non-wrapping
// row just like the Row cards' Quick Colors.
const GENERAL_QUICK_COLORS = ["#FFFFFF", "#0A0B0D", "#D9A441", "#E5484D", "#4FC3F7", "#A855F7"]
  .map((hex) => PRESET_COLORS.find((c) => c.hex === hex));

// NOTE: the date/time format registry (FORMAT_REGISTRY, FORMAT_GROUPS,
// CONTENT_LABELS, computeContent, LANGUAGE_OPTIONS, LANGUAGE_LOCALES, and
// their helper functions) now lives in time-formats.js, shared with
// widget.js so the real desktop widget computes content identically to
// this editor's preview. It's loaded before this file in index.html.

// ---------- reusable row card template ----------

function optionsHtml(options, selected) {
  return options
    .map((o) => `<option value="${o.value}"${o.value === selected ? " selected" : ""}>${o.label}</option>`)
    .join("");
}

function typeOptionsHtml(selected) {
  return FORMAT_GROUPS.map((group) => {
    const opts = group.keys
      .map((key) => {
        const entry = FORMAT_REGISTRY[key];
        const selectedAttr = key === selected ? " selected" : "";
        const disabledAttr = entry.disabled ? " disabled" : "";
        return `<option value="${key}"${selectedAttr}${disabledAttr}>${entry.label}</option>`;
      })
      .join("");
    return `<optgroup label="${group.heading}">${opts}</optgroup>`;
  }).join("");
}

// The Size slider still stores/controls the actual font-size in pixels —
// only its readout is shown as a percentage, mapped linearly across the
// slider's full range so the minimum reads 1% and the maximum reads 100%.
const ROW_SIZE_MIN_PX = 12;
const ROW_SIZE_MAX_PX = 60;

function sizePxToPercent(px) {
  const pct = 1 + ((Number(px) - ROW_SIZE_MIN_PX) * 99) / (ROW_SIZE_MAX_PX - ROW_SIZE_MIN_PX);
  return Math.round(pct);
}

function createRowCardEl(cfg) {
  const section = document.createElement("section");
  section.className = "card row-card collapsed";
  section.dataset.row = cfg.row;

  section.innerHTML = `
    <button class="card-title row-toggle" type="button" aria-expanded="false">
      <span class="tick"></span>
      <span class="row-color-dot"></span>
      <span class="row-title-text">Row ${cfg.row}</span>
    </button>
    <div class="accordion">
      <div class="accordion-inner" aria-hidden="true">
        <div class="card-body">
          <div class="field">
            <label>Type</label>
            <select class="row-type">${typeOptionsHtml(cfg.type)}</select>
          </div>

          <div class="row-divider" aria-hidden="true"></div>

          <div class="field">
            <label>Size <span class="size-readout">${sizePxToPercent(cfg.size)}%</span></label>
            <input type="range" class="row-size" min="12" max="60" value="${cfg.size}" />
          </div>

          <div class="row-divider" aria-hidden="true"></div>

          <div class="field">
            <label>Language</label>
            <select class="row-language">${optionsHtml(LANGUAGE_OPTIONS, cfg.language)}</select>
          </div>

          <div class="row-divider" aria-hidden="true"></div>

          <div class="field-row">
            <div class="field">
              <label>Text Font</label>
              <select class="row-text-font">${optionsHtml(FONT_OPTIONS, cfg.textFont)}</select>
            </div>
            <div class="field">
              <label>Number Font</label>
              <select class="row-number-font">${optionsHtml(FONT_OPTIONS, cfg.numberFont)}</select>
            </div>
          </div>

          <div class="row-divider" aria-hidden="true"></div>

          <div class="field">
            <label>Color</label>
            <div class="color-row">
              <div class="color-field">
                <input type="color" class="row-color" value="${cfg.color}" />
                <span class="hex-readout">${cfg.color.toUpperCase()}</span>
              </div>
              <div class="color-row-divider" aria-hidden="true"></div>
              <div class="color-swatches" role="group" aria-label="Quick colors">${colorSwatchesHtml(ROW_QUICK_COLORS)}</div>
            </div>
          </div>

          <div class="row-divider" aria-hidden="true"></div>

          <div class="field">
            <label>Alignment</label>
            <div class="align-group">
              <button type="button" class="align-btn" data-align="left" aria-label="Align left">⇤</button>
              <button type="button" class="align-btn" data-align="center" aria-label="Align center">⇔</button>
              <button type="button" class="align-btn" data-align="right" aria-label="Align right">⇥</button>
            </div>
          </div>

          <div class="row-divider" aria-hidden="true"></div>

          <!-- Reserved space for future controls (letter spacing, shadow, opacity, etc.) -->
          <div class="future-settings">
            <span class="future-settings-dot" aria-hidden="true"></span>
            More settings coming soon
          </div>
        </div>
      </div>
    </div>
  `;

  const alignBtn = section.querySelector(`.align-btn[data-align="${cfg.align}"]`);
  if (alignBtn) alignBtn.classList.add("active");

  return section;
}

function renderRows() {
  const container = document.getElementById("rowsContainer");
  container.innerHTML = "";
  ROW_DEFAULTS.forEach((cfg) => container.appendChild(createRowCardEl(cfg)));
}

// ---------- gather current settings from the DOM ----------

function readRowCard(cardEl) {
  const row = cardEl.dataset.row;
  const type = cardEl.querySelector(".row-type").value;
  const numberFont = cardEl.querySelector(".row-number-font").value;
  const textFont = cardEl.querySelector(".row-text-font").value;
  const size = cardEl.querySelector(".row-size").value;
  const color = cardEl.querySelector(".row-color").value;
  const language = cardEl.querySelector(".row-language").value;
  const alignBtn = cardEl.querySelector(".align-btn.active");
  const align = alignBtn ? alignBtn.dataset.align : "center";
  return { row, type, numberFont, textFont, size, color, language, align };
}

function getRowOrder() {
  return Array.from(document.querySelectorAll("#orderList li")).map((li) => li.dataset.row);
}

function readAllSettings() {
  const rowCards = Array.from(document.querySelectorAll("#rowsContainer .row-card"));
  const rows = rowCards.map(readRowCard);
  const widgetBg = document.getElementById("widgetBg").value;
  const widgetBgOpacity = document.getElementById("widgetBgOpacity").value;
  const widgetBgSizeV = document.getElementById("widgetBgSizeV").value;
  const widgetBgSizeH = document.getElementById("widgetBgSizeH").value;
  const rowOrder = getRowOrder();
  const posX = Number(document.getElementById("posX").value) || 0;
  const posY = Number(document.getElementById("posY").value) || 0;
  return { widgetBg, widgetBgOpacity, widgetBgSizeV, widgetBgSizeH, rows, rowOrder, posX, posY };
}

// ---------- live preview ----------

// Renders the editor's own preview via the SAME renderWidget() the real
// desktop widget uses (widget-renderer.js) — this is the one place the
// visual widget gets built from settings, so the two can never drift.
// Also pushes the change to the real widget window, if one is currently
// on the desktop (see "live sync" below).
function updatePreview() {
  const settings = readAllSettings();
  const rowsByNumber = {};
  settings.rows.forEach((r) => (rowsByNumber[r.row] = r));

  renderWidget(document.getElementById("widgetMock"), settings);

  updateOrderMeta(rowsByNumber);
  updateRowColorDots(settings.rows);
}

// Every user-driven settings change should update the editor preview AND,
// if a desktop widget is currently active, push the new config to it live.
// The clock tick (below) intentionally calls updatePreview() directly
// instead of this, since a tick isn't a settings change and the widget
// already ticks its own clock independently.
function handleSettingsChanged() {
  updatePreview();
  syncWidgetIfActive();
}

// Show each row's current type next to it in the "Row order" list.
function updateOrderMeta(rowsByNumber) {
  document.querySelectorAll("#orderList li").forEach((li) => {
    const r = rowsByNumber[li.dataset.row];
    const meta = li.querySelector(".order-meta");
    if (meta && r) meta.textContent = CONTENT_LABELS[r.type] || "";
  });
}

// Reflect each row's chosen color as a small swatch on its header,
// visible even when the accordion is collapsed.
function updateRowColorDots(rows) {
  rows.forEach((r) => {
    const card = document.querySelector(`.row-card[data-row="${r.row}"]`);
    const dot = card && card.querySelector(".row-color-dot");
    if (dot) dot.style.background = r.color;
  });
}

// ---------- accordion toggle (shared by General + every Row section) ----------

function setupAccordionToggles() {
  document.addEventListener("click", (e) => {
    const toggle = e.target.closest(".row-toggle");
    if (!toggle) return;
    const card = toggle.closest(".card");
    if (!card) return;
    const collapsed = card.classList.toggle("collapsed");
    toggle.setAttribute("aria-expanded", String(!collapsed));
    const inner = card.querySelector(".accordion-inner");
    if (inner) inner.setAttribute("aria-hidden", String(collapsed));

    // Bring the header being expanded to the top of the scroll container.
    // Combined with the sticky header itself, this is what stops a long
    // expanded row from "trapping" the user far down the page — the header
    // (and the ability to collapse it again) is always one glance away.
    if (!collapsed) {
      requestAnimationFrame(() => {
        toggle.scrollIntoView({ behavior: "smooth", block: "start" });
      });
    }
  });
}

// ---------- color presets (shared by General background + every row color) ----------

function populateGeneralSwatches() {
  const el = document.getElementById("widgetBgSwatches");
  if (el) el.innerHTML = colorSwatchesHtml(GENERAL_QUICK_COLORS);
}

// Highlights whichever swatch (if any) matches a color input's current
// value, and clears the rest — so picking a fully custom color naturally
// leaves no preset marked as selected.
function syncSwatchSelection(colorInput) {
  const field = colorInput.closest(".field");
  if (!field) return;
  const value = colorInput.value.toLowerCase();
  field.querySelectorAll(".color-swatch").forEach((sw) => {
    sw.classList.toggle("selected", sw.dataset.color.toLowerCase() === value);
  });
}

function syncAllSwatchSelections() {
  document.querySelectorAll('input[type="color"]').forEach(syncSwatchSelection);
}

function setupColorSwatches() {
  document.addEventListener("click", (e) => {
    const swatch = e.target.closest(".color-swatch");
    if (!swatch) return;
    const field = swatch.closest(".field");
    const colorInput = field && field.querySelector('input[type="color"]');
    if (!colorInput) return;
    colorInput.value = swatch.dataset.color;
    // Dispatching "input" lets the existing per-field listeners (hex
    // readout, live preview, row color dot) react exactly as they would
    // to a manual pick — no duplicate logic needed here.
    colorInput.dispatchEvent(new Event("input", { bubbles: true }));
  });

  // Keep swatch selection state in sync no matter how a color input's
  // value changes — swatch click, native picker, or restored settings.
  document.addEventListener("input", (e) => {
    if (e.target.matches('input[type="color"]')) syncSwatchSelection(e.target);
  });
}

// ---------- row interactions (event delegation — scales to any number of rows) ----------

function setupRowsDelegation() {
  const container = document.getElementById("rowsContainer");
  if (!container) return;

  container.addEventListener("click", (e) => {
    const alignBtn = e.target.closest(".align-btn");
    if (alignBtn) {
      const group = alignBtn.closest(".align-group");
      group.querySelectorAll(".align-btn").forEach((b) => b.classList.remove("active"));
      alignBtn.classList.add("active");
      handleSettingsChanged();
    }
  });

  container.addEventListener("input", (e) => {
    if (e.target.matches(".row-size")) {
      const readout = e.target.closest(".field").querySelector(".size-readout");
      if (readout) readout.textContent = `${sizePxToPercent(e.target.value)}%`;
    }
    if (e.target.matches(".row-color")) {
      const hex = e.target.closest(".color-field").querySelector(".hex-readout");
      if (hex) hex.textContent = e.target.value.toUpperCase();
    }
    handleSettingsChanged();
  });

  container.addEventListener("change", (e) => {
    if (e.target.matches(".row-type, .row-number-font, .row-text-font, .row-language")) {
      handleSettingsChanged();
    }
  });
}

// ---------- General section: widget background ----------

function setupGeneralLiveInputs() {
  document.getElementById("widgetBg").addEventListener("input", (e) => {
    const hex = e.target.closest(".color-field").querySelector(".hex-readout");
    if (hex) hex.textContent = e.target.value.toUpperCase();
    handleSettingsChanged();
  });
}

// ---------- General section: Size (widget background box) ----------
//
// Vertical Size and Horizontal Size are independent sliders that each
// control one dimension of the background box's padding (height/width
// respectively) — same slider component, same behavior, just wired to
// their own readout so they can be adjusted separately.

function setupGeneralSizeReadout() {
  [
    { sliderId: "widgetBgSizeV", readoutId: "widgetBgSizeVValue" },
    { sliderId: "widgetBgSizeH", readoutId: "widgetBgSizeHValue" },
  ].forEach(({ sliderId, readoutId }) => {
    const slider = document.getElementById(sliderId);
    const readout = document.getElementById(readoutId);
    slider.addEventListener("input", () => {
      readout.textContent = `${slider.value}px`;
      handleSettingsChanged();
    });
  });
}

// ---------- General section: Position ----------
//
// Unlike the rest of General, X/Y don't affect the editor's own preview
// (widgetMock is just a DOM element on this page, not something the OS
// positions) — they only matter to a real desktop widget window, if one is
// currently active. So this intentionally calls syncWidgetIfActive()'s
// position-only counterpart instead of handleSettingsChanged().
function setupPositionLiveInputs() {
  ["posX", "posY"].forEach((id) => {
    document.getElementById(id).addEventListener("input", handlePositionChanged);
  });
}

function handlePositionChanged() {
  if (!widgetActive || !tauriInvoke) return;
  const x = Number(document.getElementById("posX").value) || 0;
  const y = Number(document.getElementById("posY").value) || 0;
  tauriInvoke("set_widget_position", { x, y }).catch((err) => {
    console.error("[PushToDesktop] Failed to move desktop widget:", err);
  });
}

function setupOpacityReadout() {
  const slider = document.getElementById("widgetBgOpacity");
  const readout = document.getElementById("widgetBgOpacityValue");
  slider.addEventListener("input", () => {
    readout.textContent = `${slider.value}%`;
    handleSettingsChanged();
  });
}

// ---------- drag-to-reorder rows ----------

function setupOrderDragDrop() {
  const list = document.getElementById("orderList");
  if (!list) return;

  list.querySelectorAll("li").forEach((li) => {
    li.addEventListener("dragstart", () => {
      li.classList.add("dragging");
    });
    li.addEventListener("dragend", () => {
      li.classList.remove("dragging");
      renumberOrderPositions();
      handleSettingsChanged();
    });
  });

  list.addEventListener("dragover", (e) => {
    e.preventDefault();
    const dragging = list.querySelector(".dragging");
    if (!dragging) return;
    const afterElement = getDragAfterElement(list, e.clientY);
    if (afterElement == null) {
      list.appendChild(dragging);
    } else {
      list.insertBefore(dragging, afterElement);
    }
    renumberOrderPositions();
  });
}

function getDragAfterElement(container, y) {
  const items = [...container.querySelectorAll("li:not(.dragging)")];
  return items.reduce(
    (closest, child) => {
      const box = child.getBoundingClientRect();
      const offset = y - box.top - box.height / 2;
      if (offset < 0 && offset > closest.offset) {
        return { offset, element: child };
      }
      return closest;
    },
    { offset: Number.NEGATIVE_INFINITY, element: null }
  ).element;
}

function renumberOrderPositions() {
  document.querySelectorAll("#orderList li").forEach((li, i) => {
    const pos = li.querySelector(".order-pos");
    if (pos) pos.textContent = i + 1;
  });
}

// ---------- desktop widget: Tauri bridge ----------
//
// The editor talks to the real desktop widget (a separate, borderless
// Tauri window — see src-tauri/src/widget_window.rs) through six commands:
//   push_widget(config, x, y)    create the widget, or update it (config
//                                 AND position) if it's already up
//   update_widget_config(config) live-sync config while the widget is active
//   set_widget_position(x, y)    live-sync position while the widget is
//                                 active (Settings' Position fields)
//   delete_widget()              close the widget and free its resources
//   get_widget_config()          widget.html pulls this on load
//   is_widget_active()           used to restore the Delete button's state
// plus one event the widget's window fires back if it's ever torn down by
// something other than the Delete button: "widget-closed".
//
// `window.__TAURI__` requires `app.withGlobalTauri: true` in
// tauri.conf.json. Everything here degrades gracefully (buttons stay
// informative instead of throwing) if this file is ever opened outside
// the Tauri shell, e.g. directly in a browser while tweaking CSS.

const tauriBridge = window.__TAURI__;
const tauriInvoke = tauriBridge && tauriBridge.core && tauriBridge.core.invoke;
const tauriListen = tauriBridge && tauriBridge.event && tauriBridge.event.listen;

let widgetActive = false;

function updateWidgetControlsUI() {
  const deleteBtn = document.getElementById("deleteBtn");
  if (!deleteBtn) return;
  deleteBtn.disabled = !widgetActive;
  deleteBtn.title = widgetActive ? "" : "No widget is currently on the desktop";
}

async function initWidgetState() {
  if (!tauriInvoke) return;
  try {
    widgetActive = await tauriInvoke("is_widget_active");
  } catch (e) {
    widgetActive = false;
  }
  updateWidgetControlsUI();
}

function setupWidgetClosedListener() {
  if (!tauriListen) return;
  tauriListen("widget-closed", () => {
    widgetActive = false;
    updateWidgetControlsUI();
  });
}

// Fires on every settings change (see handleSettingsChanged above) — a
// no-op unless a widget is actually on the desktop right now.
function syncWidgetIfActive() {
  if (!widgetActive || !tauriInvoke) return;
  tauriInvoke("update_widget_config", { config: readAllSettings() }).catch((err) => {
    console.error("Failed to sync desktop widget", err);
  });
}

function flashStatus(text) {
  const status = document.getElementById("saveStatus");
  status.textContent = text;
  setTimeout(() => (status.textContent = ""), 2500);
}

// ---------- push to desktop ----------

// Hard ceiling on how long we'll wait for the backend before giving up and
// resetting the UI ourselves. `push_widget` should always resolve or reject
// well before this fires — it exists purely as a last-resort safety net so
// a future regression (or a genuinely wedged backend) can never again leave
// the button stuck in "Pushing…" forever with no way out for the user.
const PUSH_TIMEOUT_MS = 10000;

function setupPush() {
  const btn = document.getElementById("pushBtn");
  const label = btn.querySelector(".btn-label");
  const originalLabel = label.textContent;

  btn.addEventListener("click", async () => {
    console.log("[PushToDesktop] Button clicked");

    if (!tauriInvoke) {
      console.error("[PushToDesktop] No Tauri bridge available (running outside the app?)");
      flashStatus("Desktop widgets require the Chronon app");
      return;
    }

    btn.disabled = true;
    label.textContent = "Pushing…";

    console.log("[PushToDesktop] Validating configuration...");
    let config;
    try {
      config = readAllSettings();
    } catch (err) {
      console.error("[PushToDesktop] Failed to read widget configuration", err);
      flashStatus("Failed to load widget configuration.");
      label.textContent = originalLabel;
      btn.disabled = false;
      return;
    }

    console.log("[PushToDesktop] Sending invoke() command...");

    let timedOut = false;
    const timeout = new Promise((_, reject) => {
      setTimeout(() => {
        timedOut = true;
        reject(new Error("Timed out waiting for the backend to respond."));
      }, PUSH_TIMEOUT_MS);
    });

    try {
      await Promise.race([
        tauriInvoke("push_widget", { config, x: config.posX, y: config.posY }),
        timeout,
      ]);
      console.log("[PushToDesktop] Desktop widget is now active");
      widgetActive = true;
      updateWidgetControlsUI();
      flashStatus("Widget successfully pushed to the desktop.");
    } catch (err) {
      const message = timedOut
        ? "Couldn't push to desktop: the backend didn't respond in time."
        : `Couldn't push to desktop: ${errorMessage(err)}`;
      console.error("[PushToDesktop] Failed at push step:", err);
      flashStatus(message);
    } finally {
      // Always runs, on every path above (success, backend error, thrown
      // error, or timeout) — the button can never remain stuck disabled
      // or showing "Pushing…".
      label.textContent = originalLabel;
      btn.disabled = false;
    }
  });
}

// Tauri command errors can arrive as a plain string, an Error, or an
// object with a message field depending on how the Rust side rejected.
// Normalize all of those into a readable string for the status message.
function errorMessage(err) {
  if (typeof err === "string") return err;
  if (err && typeof err.message === "string") return err.message;
  try {
    return JSON.stringify(err);
  } catch {
    return "Unexpected error.";
  }
}

// ---------- delete from desktop ----------

function setupDelete() {
  const btn = document.getElementById("deleteBtn");
  const overlay = document.getElementById("deleteConfirmOverlay");
  const cancelBtn = document.getElementById("deleteConfirmCancel");
  const okBtn = document.getElementById("deleteConfirmOk");
  if (!btn || !overlay) return;

  const openConfirm = () => {
    overlay.hidden = false;
    requestAnimationFrame(() => overlay.classList.add("open"));
  };
  const closeConfirm = () => {
    overlay.classList.remove("open");
    setTimeout(() => {
      overlay.hidden = true;
    }, 180);
  };

  btn.addEventListener("click", () => {
    if (btn.disabled) return;
    openConfirm();
  });

  cancelBtn.addEventListener("click", closeConfirm);
  overlay.addEventListener("click", (e) => {
    if (e.target === overlay) closeConfirm();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !overlay.hidden) closeConfirm();
  });

  okBtn.addEventListener("click", async () => {
    closeConfirm();
    if (!tauriInvoke) return;

    try {
      await tauriInvoke("delete_widget");
      widgetActive = false;
      updateWidgetControlsUI();
      flashStatus("Removed from desktop");
    } catch (err) {
      console.error("Failed to remove desktop widget", err);
      flashStatus("Couldn't remove widget");
    }
  });
}

// ---------- restore previously saved settings on load ----------

function restoreSettings() {
  const raw = localStorage.getItem("widgetTimeSettings");
  if (!raw) return;
  try {
    const settings = JSON.parse(raw);
    document.getElementById("widgetBg").value = settings.widgetBg || "#000000";
    const generalColorField = document.getElementById("widgetBg").closest(".color-field");
    const generalHex = generalColorField && generalColorField.querySelector(".hex-readout");
    if (generalHex) generalHex.textContent = (settings.widgetBg || "#000000").toUpperCase();
    document.getElementById("widgetBgOpacity").value = settings.widgetBgOpacity || 0;
    document.getElementById("widgetBgOpacityValue").textContent = `${settings.widgetBgOpacity || 0}%`;
    // Migrate settings saved before Size was split: fall back to the old
    // single widgetBgSize (then the original 14px default) for either
    // dimension that hasn't been set independently yet.
    const legacySize = settings.widgetBgSize != null ? settings.widgetBgSize : 14;
    const widgetBgSizeV = settings.widgetBgSizeV != null ? settings.widgetBgSizeV : legacySize;
    const widgetBgSizeH = settings.widgetBgSizeH != null ? settings.widgetBgSizeH : legacySize;
    document.getElementById("widgetBgSizeV").value = widgetBgSizeV;
    document.getElementById("widgetBgSizeVValue").textContent = `${widgetBgSizeV}px`;
    document.getElementById("widgetBgSizeH").value = widgetBgSizeH;
    document.getElementById("widgetBgSizeHValue").textContent = `${widgetBgSizeH}px`;
    document.getElementById("posX").value = settings.posX || 0;
    document.getElementById("posY").value = settings.posY || 0;

    (settings.rows || []).forEach((r) => {
      const card = document.querySelector(`.row-card[data-row="${r.row}"]`);
      if (!card) return;
      card.querySelector(".row-type").value = FORMAT_REGISTRY[r.type] ? r.type : "none";
      // Fall back to the old single "font" field for settings saved before
      // Number/Text fonts existed, so previously saved widgets keep working.
      card.querySelector(".row-number-font").value = r.numberFont || r.font || DEFAULT_FONT;
      card.querySelector(".row-text-font").value = r.textFont || r.font || DEFAULT_FONT;
      card.querySelector(".row-size").value = r.size;
      card.querySelector(".row-color").value = r.color;
      card.querySelector(".row-language").value = r.language || "en";
      card.querySelector(".size-readout").textContent = `${sizePxToPercent(r.size)}%`;
      card.querySelector(".hex-readout").textContent = (r.color || "").toUpperCase();
      card.querySelectorAll(".align-btn").forEach((b) => {
        b.classList.toggle("active", b.dataset.align === r.align);
      });
    });

    if (Array.isArray(settings.rowOrder) && settings.rowOrder.length) {
      const list = document.getElementById("orderList");
      settings.rowOrder.forEach((rowNum) => {
        const li = list.querySelector(`li[data-row="${rowNum}"]`);
        if (li) list.appendChild(li);
      });
    }
  } catch (e) {
    console.error("Failed to restore settings", e);
  }
}

// ---------- live clock tick ----------

// SECOND_LEVEL_TYPES + needsSecondTicks() live in time-formats.js, shared
// with widget.js so the real desktop widget ticks on the same schedule.
function anyRowNeedsSecondTicks() {
  return needsSecondTicks(readAllSettings());
}

function startClockTick() {
  let timer = null;
  const tick = () => {
    updatePreview();
    clearTimeout(timer);
    timer = setTimeout(tick, anyRowNeedsSecondTicks() ? 1000 : 1000 * 30);
  };
  tick();
}

// ---------- sidebar navigation ----------

// Collapsed sidebar shows icons only; expanding it reveals labels via CSS
// (see .sidebar.expanded / .nav-label in styles.css). This just toggles state.
function setupSidebarToggle() {
  const sidebar = document.getElementById("sidebar");
  const toggle = document.getElementById("sidebarToggle");
  if (!sidebar || !toggle) return;

  toggle.addEventListener("click", () => {
    const expanded = sidebar.classList.toggle("expanded");
    toggle.setAttribute("aria-expanded", String(expanded));
    toggle.setAttribute("aria-label", expanded ? "Collapse navigation" : "Expand navigation");
  });
}

// Home / Presets / Settings / About each get their own scroll container
// (see .page-panel in styles.css — position:absolute inside .page-viewport),
// so switching pages never carries over another page's scroll offset, and
// only the active page is reachable/visible at rest. The brief transition
// (~240ms) fades + slides the outgoing page down while the incoming page
// fades + slides up into place.
const PAGE_TRANSITION_MS = 240;

function setupSidebarPages() {
  const navItems = Array.from(document.querySelectorAll(".nav-item"));
  const panels = Array.from(document.querySelectorAll(".page-panel"));
  if (!navItems.length || !panels.length) return;

  const goToPage = (page) => {
    const next = panels.find((p) => p.dataset.pagePanel === page);
    const current = panels.find((p) => !p.hidden);
    if (!next || next === current) return;

    navItems.forEach((n) => {
      const isActive = n.dataset.page === page;
      n.classList.toggle("active", isActive);
      if (isActive) n.setAttribute("aria-current", "page");
      else n.removeAttribute("aria-current");
    });

    if (current) {
      current.classList.add("page-leave");
      window.setTimeout(() => {
        current.hidden = true;
        current.classList.remove("page-leave");
      }, PAGE_TRANSITION_MS);
    }

    next.hidden = false;
    next.classList.add("page-enter");
    // Force layout so the "enter" starting state actually paints before we
    // transition away from it, then release it on the next frame.
    void next.offsetWidth;
    requestAnimationFrame(() => {
      next.classList.remove("page-enter");
    });
  };

  navItems.forEach((item) => {
    item.addEventListener("click", () => goToPage(item.dataset.page));
  });
}

// ---------- init ----------

window.addEventListener("DOMContentLoaded", () => {
  renderRows();
  populateGeneralSwatches();
  setupAccordionToggles();
  setupRowsDelegation();
  setupGeneralLiveInputs();
  setupGeneralSizeReadout();
  setupPositionLiveInputs();
  setupOpacityReadout();
  setupOrderDragDrop();
  setupColorSwatches();
  setupPush();
  setupDelete();
  setupWidgetClosedListener();
  setupSidebarToggle();
  setupSidebarPages();
  restoreSettings();
  renumberOrderPositions();
  syncAllSwatchSelections();
  updatePreview();
  startClockTick();
  initWidgetState();
});
