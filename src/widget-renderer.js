// =========================================================================
// widget-renderer.js — SHARED between the editor preview (index.html) and
// the real desktop widget (widget.html).
//
// This is the ONE place that turns a `settings` object into the widget's
// actual DOM/visuals. The editor's live preview and the real desktop widget
// both call renderWidget() with their own root element — neither one has
// its own copy of this logic, so any visual change here applies to both
// automatically. Depends on time-formats.js being loaded first.
// =========================================================================

function escapeHtml(str) {
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// Splits text into alternating runs of digits vs. everything else, so numeric
// values (hours, days, years...) can use the Number Font while words
// (weekday/month names, AM/PM, separators...) use the Text Font.
function renderStyledContent(el, text) {
  const tokens = String(text).match(/\d+|\D+/g) || [];
  el.innerHTML = tokens
    .map((t) => {
      const cls = /^\d+$/.test(t) ? "num-part" : "txt-part";
      return `<span class="${cls}">${escapeHtml(t)}</span>`;
    })
    .join("");
}

function hexToRgba(hex, alpha) {
  const clean = hex.replace("#", "");
  const r = parseInt(clean.substring(0, 2), 16);
  const g = parseInt(clean.substring(2, 4), 16);
  const b = parseInt(clean.substring(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

// Background, padding, and corner radius of the widget "card" itself.
// widgetBgSizeV/widgetBgSizeH control only the padding around the rows
// (i.e. the height/width of the background box) — they never touch row
// font sizes. Falls back to the old single widgetBgSize (or a fixed 14px)
// for settings saved before Vertical/Horizontal Size were split apart.
function renderWidgetChrome(rootEl, settings) {
  const opacity = Number(settings.widgetBgOpacity) / 100;
  rootEl.style.background = hexToRgba(settings.widgetBg, opacity);
  rootEl.style.padding = opacity > 0 ? "14px" : "0px";
  rootEl.style.borderRadius = opacity > 0 ? "6px" : "0px";
}

function widgetScaleFactor(settings) {
  const scalePercent = Number(settings.widgetScale);
  if (!Number.isFinite(scalePercent)) return 1;
  return Math.min(5, Math.max(0.1, scalePercent / 100));
}

// The widget previously used a fixed 4px flex gap between every row. Keep
// that value as the default while allowing the two named row pairs to be
// adjusted independently. Values are clamped so malformed saved settings can
// never turn into negative spacing and make rows overlap.
const DEFAULT_ROW_SPACING_PX = 4;
const MAX_ROW_SPACING_PX = 64;

function rowSpacing(value) {
  if (value == null) return DEFAULT_ROW_SPACING_PX;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return DEFAULT_ROW_SPACING_PX;
  return Math.min(MAX_ROW_SPACING_PX, Math.max(0, parsed));
}

function spacingAfter(previousRow, currentRow, settings) {
  if (previousRow === "1" && currentRow === "2") {
    return rowSpacing(settings.row1To2Spacing);
  }
  if (previousRow === "2" && currentRow === "3") {
    return rowSpacing(settings.row2To3Spacing);
  }
  return DEFAULT_ROW_SPACING_PX;
}

// Renders the row stack in the configured order, skipping "None" rows
// entirely. Reuses existing row elements in place where possible (matched
// by position + row id) rather than tearing everything down every call —
// this runs on every clock tick, so it stays cheap and doesn't interrupt
// any future per-row transitions.
function renderWidgetRows(rootEl, settings) {
  // Apply spacing per adjacent row pair instead of a single flex `gap`, so a
  // slider can change only its named pair while every other gap remains 4px.
  rootEl.style.gap = "0px";

  const rowsByNumber = {};
  settings.rows.forEach((r) => (rowsByNumber[r.row] = r));

  // Rows always render in their configured Row 1 / Row 2 / Row 3 sequence.
  const order = settings.rows.map((r) => r.row);

  const visible = [];
  order.forEach((rowNum) => {
    const r = rowsByNumber[rowNum];
    if (!r) return;
    const locale = LANGUAGE_LOCALES[r.language] || "en-US";
    const text = computeContent(r.type, locale);
    if (text === null || text === undefined) return; // "None" — contributes nothing
    visible.push({ r, text });
  });

  const existing = Array.from(rootEl.children);
  visible.forEach(({ r, text }, i) => {
    let el = existing[i];
    if (!el || el.dataset.row !== r.row) {
      el = document.createElement("div");
      el.className = "w-row";
      el.dataset.row = r.row;
      if (existing[i]) rootEl.insertBefore(el, existing[i]);
      else rootEl.appendChild(el);
    }

    renderStyledContent(el, text);
    el.style.setProperty("--number-font", r.numberFont);
    el.style.setProperty("--text-font", r.textFont);
    el.style.fontSize = `${r.size}px`;
    el.style.color = r.color;
    el.style.textAlign = r.align;
    el.style.marginTop = i === 0
      ? "0px"
      : `${spacingAfter(visible[i - 1].r.row, r.row, settings)}px`;
  });

  while (rootEl.children.length > visible.length) {
    rootEl.removeChild(rootEl.lastChild);
  }
}

// Entry point: renders a full widget (chrome + rows) into rootEl for the
// given settings object. This is the only function callers need.
function renderWidget(rootEl, settings) {
  if (!rootEl || !settings) return;
  rootEl.style.transform = `scale(${widgetScaleFactor(settings)})`;
  rootEl.style.transformOrigin = "center";
  renderWidgetChrome(rootEl, settings);
  renderWidgetRows(rootEl, settings);
}
