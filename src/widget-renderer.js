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
// widgetBgSize controls only the padding around the rows (i.e. the size of
// the background box) — it never touches row font sizes. Falls back to the
// old fixed 14px for settings saved before this control existed.
function renderWidgetChrome(rootEl, settings) {
  const opacity = Number(settings.widgetBgOpacity) / 100;
  const boxSize = settings.widgetBgSize != null ? Number(settings.widgetBgSize) : 14;
  rootEl.style.background = hexToRgba(settings.widgetBg, opacity);
  rootEl.style.padding = opacity > 0 ? `${boxSize}px` : "0px";
  rootEl.style.borderRadius = opacity > 0 ? "6px" : "0px";
}

// Renders the row stack in the configured order, skipping "None" rows
// entirely. Reuses existing row elements in place where possible (matched
// by position + row id) rather than tearing everything down every call —
// this runs on every clock tick, so it stays cheap and doesn't interrupt
// any future per-row transitions.
function renderWidgetRows(rootEl, settings) {
  const rowsByNumber = {};
  settings.rows.forEach((r) => (rowsByNumber[r.row] = r));

  const order = Array.isArray(settings.rowOrder) && settings.rowOrder.length
    ? settings.rowOrder
    : settings.rows.map((r) => r.row);

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
  });

  while (rootEl.children.length > visible.length) {
    rootEl.removeChild(rootEl.lastChild);
  }
}

// Entry point: renders a full widget (chrome + rows) into rootEl for the
// given settings object. This is the only function callers need.
function renderWidget(rootEl, settings) {
  if (!rootEl || !settings) return;
  renderWidgetChrome(rootEl, settings);
  renderWidgetRows(rootEl, settings);
}
