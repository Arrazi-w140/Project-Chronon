// =========================================================================
// time-formats.js — SHARED between the editor (index.html/main.js) and the
// standalone desktop widget (widget.html/widget.js).
//
// Pure logic only: given a "type" key + locale, compute the string a row
// should display right now. No DOM access happens in this file, so it's
// safe to load into either window unchanged. Loaded as a plain classic
// script (not a module) — its top-level consts/functions are visible to
// whichever script tag follows it in the same page.
// =========================================================================

function pad(n) {
  return String(n).padStart(2, "0");
}

function getISOWeek(date) {
  const d = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()));
  const dayNum = d.getUTCDay() || 7;
  d.setUTCDate(d.getUTCDate() + 4 - dayNum);
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
  return Math.ceil((((d - yearStart) / 86400000) + 1) / 7);
}

function getDayOfYear(date) {
  const start = new Date(date.getFullYear(), 0, 0);
  return Math.floor((date - start) / 86400000);
}

function getUtcOffsetString(date) {
  const offsetMin = -date.getTimezoneOffset();
  const sign = offsetMin >= 0 ? "+" : "-";
  const abs = Math.abs(offsetMin);
  return `UTC${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
}

function getTimeZoneAbbrev(date, locale) {
  try {
    const parts = new Intl.DateTimeFormat(locale, { timeZoneName: "short" }).formatToParts(date);
    const tz = parts.find((p) => p.type === "timeZoneName");
    return tz ? tz.value : "";
  } catch (e) {
    return "";
  }
}

function getDayPeriod(date, locale) {
  try {
    const parts = new Intl.DateTimeFormat(locale, { hour: "numeric", hour12: true }).formatToParts(date);
    const p = parts.find((part) => part.type === "dayPeriod");
    return p ? p.value : "";
  } catch (e) {
    return date.getHours() < 12 ? "AM" : "PM";
  }
}

const LANGUAGE_OPTIONS = [
  { value: "en", label: "English", locale: "en-US" },
  { value: "fr", label: "French", locale: "fr-FR" },
  { value: "ar", label: "Arabic", locale: "ar" },
  { value: "de", label: "German", locale: "de-DE" },
  { value: "es", label: "Spanish", locale: "es-ES" },
  { value: "ja", label: "Japanese", locale: "ja-JP" },
  { value: "zh", label: "Chinese", locale: "zh-CN" },
];

const LANGUAGE_LOCALES = Object.fromEntries(LANGUAGE_OPTIONS.map((l) => [l.value, l.locale]));

// =========================================================================
// Format registry — every selectable "Type" option lives here as one entry.
//
// To add a new format later: add one key to FORMAT_REGISTRY (label + a
// compute(date, locale) function) and list that key inside FORMAT_GROUPS
// wherever it should appear in the dropdown. Nothing else needs to change —
// the <select> markup, the "Row order" labels, and both the editor preview
// and the real widget read from this registry automatically.
// =========================================================================

const FORMAT_REGISTRY = {
  // ---------- Time ----------
  time24: { label: "Hours : Minutes (24-hour)", compute: (d, l) => d.toLocaleTimeString(l, { hour: "2-digit", minute: "2-digit", hour12: false }) },
  time12: { label: "Hours : Minutes (12-hour)", compute: (d, l) => d.toLocaleTimeString(l, { hour: "2-digit", minute: "2-digit", hour12: true }) },
  time24Sec: { label: "Hours : Minutes : Seconds (24-hour)", compute: (d, l) => d.toLocaleTimeString(l, { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false }) },
  time12Sec: { label: "Hours : Minutes : Seconds (12-hour)", compute: (d, l) => d.toLocaleTimeString(l, { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: true }) },
  hoursOnly: { label: "Hours only", compute: (d) => pad(d.getHours()) },
  minutesOnly: { label: "Minutes only", compute: (d) => pad(d.getMinutes()) },
  secondsOnly: { label: "Seconds only", compute: (d) => pad(d.getSeconds()) },
  ampm: { label: "AM / PM only", compute: (d, l) => getDayPeriod(d, l) },
  unixTimestamp: { label: "Unix Timestamp", compute: (d) => String(Math.floor(d.getTime() / 1000)) },

  // ---------- Date ----------
  dayOfMonth: { label: "Day of Month (29)", compute: (d) => String(d.getDate()) },
  dayName: { label: "Day Name (Wednesday)", compute: (d, l) => d.toLocaleDateString(l, { weekday: "long" }) },
  dayNameShort: { label: "Short Day Name (Wed)", compute: (d, l) => d.toLocaleDateString(l, { weekday: "short" }) },
  dayNumberName: { label: "Day Number + Day Name (29 Wednesday)", compute: (d, l) => `${d.getDate()} ${d.toLocaleDateString(l, { weekday: "long" })}` },
  monthName: { label: "Month Name (July)", compute: (d, l) => d.toLocaleDateString(l, { month: "long" }) },
  monthNameShort: { label: "Short Month Name (Jul)", compute: (d, l) => d.toLocaleDateString(l, { month: "short" }) },
  monthNumber: { label: "Month Number (07)", compute: (d) => pad(d.getMonth() + 1) },
  monthYear: { label: "Month & Year (July 2026)", compute: (d, l) => d.toLocaleDateString(l, { month: "long", year: "numeric" }) },
  monthNumberYear: { label: "Month Number & Year (07 / 2026)", compute: (d) => `${pad(d.getMonth() + 1)} / ${d.getFullYear()}` },
  year: { label: "Year (2026)", compute: (d) => String(d.getFullYear()) },
  yearShort: { label: "Short Year (26)", compute: (d) => String(d.getFullYear()).slice(-2) },
  fullDate: { label: "Full Date (Wednesday, July 29, 2026)", compute: (d, l) => d.toLocaleDateString(l, { weekday: "long", month: "long", day: "numeric", year: "numeric" }) },
  shortDate: { label: "Short Date (29/07/2026)", compute: (d) => `${pad(d.getDate())}/${pad(d.getMonth() + 1)}/${d.getFullYear()}` },
  isoDate: { label: "ISO Date (2026-07-29)", compute: (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}` },

  // ---------- Combined Date + Time ----------
  dateTime: { label: "Date + Time", compute: (d, l) => `${d.toLocaleDateString(l, { month: "long", day: "numeric" })}, ${FORMAT_REGISTRY.time24.compute(d, l)}` },
  dateTimeSeconds: { label: "Date + Time + Seconds", compute: (d, l) => `${d.toLocaleDateString(l, { month: "long", day: "numeric" })}, ${FORMAT_REGISTRY.time24Sec.compute(d, l)}` },
  dayTime: { label: "Day + Time", compute: (d, l) => `${d.getDate()} ${FORMAT_REGISTRY.time24.compute(d, l)}` },
  monthYearTime: { label: "Month + Year + Time", compute: (d, l) => `${d.toLocaleDateString(l, { month: "long", year: "numeric" })}, ${FORMAT_REGISTRY.time24.compute(d, l)}` },
  fullDateTime: { label: "Full Date + Time", compute: (d, l) => `${FORMAT_REGISTRY.fullDate.compute(d, l)}, ${FORMAT_REGISTRY.time24.compute(d, l)}` },
  isoDateTime: { label: "ISO Date + Time", compute: (d, l) => `${FORMAT_REGISTRY.isoDate.compute(d, l)} ${FORMAT_REGISTRY.time24.compute(d, l)}` },
  shortDateTime: { label: "Short Date + Time", compute: (d, l) => `${FORMAT_REGISTRY.shortDate.compute(d, l)} ${FORMAT_REGISTRY.time24.compute(d, l)}` },
  dayNameTime: { label: "Day Name + Time", compute: (d, l) => `${FORMAT_REGISTRY.dayName.compute(d, l)} ${FORMAT_REGISTRY.time24.compute(d, l)}` },

  // ---------- Miscellaneous ----------
  weekNumber: { label: "Week Number", compute: (d) => `Week ${getISOWeek(d)}` },
  dayOfYear: { label: "Day of Year", compute: (d) => `Day ${getDayOfYear(d)}` },
  quarter: { label: "Quarter (Q1–Q4)", compute: (d) => `Q${Math.floor(d.getMonth() / 3) + 1}` },
  timeZone: { label: "Time Zone", compute: (d, l) => getTimeZoneAbbrev(d, l) },
  utcOffset: { label: "UTC Offset", compute: (d) => getUtcOffsetString(d) },
  custom: { label: "Custom Format (coming soon)", disabled: true, compute: () => "Custom format" },
  none: { label: "None", compute: () => null },
};

// Dropdown structure: which keys appear under which heading, and in what order.
const FORMAT_GROUPS = [
  { heading: "Time", keys: ["time24", "time12", "time24Sec", "time12Sec", "hoursOnly", "minutesOnly", "secondsOnly", "ampm", "unixTimestamp"] },
  { heading: "Date", keys: ["dayOfMonth", "dayName", "dayNameShort", "dayNumberName", "monthName", "monthNameShort", "monthNumber", "monthYear", "monthNumberYear", "year", "yearShort", "fullDate", "shortDate", "isoDate"] },
  { heading: "Combined Date + Time", keys: ["dateTime", "dateTimeSeconds", "dayTime", "monthYearTime", "fullDateTime", "isoDateTime", "shortDateTime", "dayNameTime"] },
  { heading: "Miscellaneous", keys: ["weekNumber", "dayOfYear", "quarter", "timeZone", "utcOffset", "custom", "none"] },
];

const CONTENT_LABELS = Object.fromEntries(Object.entries(FORMAT_REGISTRY).map(([key, entry]) => [key, entry.label]));

function computeContent(type, locale) {
  const entry = FORMAT_REGISTRY[type];
  if (!entry) return "";
  return entry.compute(new Date(), locale);
}

// Most formats only need a refresh every 30s, but seconds-level formats
// (HH:MM:SS, Seconds only, Unix Timestamp...) need to tick every second to
// look alive. Both the editor preview and the real widget use this to
// decide their own tick interval.
const SECOND_LEVEL_TYPES = new Set(["time24Sec", "time12Sec", "secondsOnly", "unixTimestamp", "dateTimeSeconds"]);

function needsSecondTicks(settings) {
  if (!settings || !Array.isArray(settings.rows)) return false;
  return settings.rows.some((r) => SECOND_LEVEL_TYPES.has(r.type));
}
