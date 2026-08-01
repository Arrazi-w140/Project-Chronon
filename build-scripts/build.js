// ================================================================
// build-scripts/build.js
// ----------------------------------------------------------------
// `npm run build` (and `npm run release`) run this instead of calling
// `tauri build` directly:
//   1. Runs the real Tauri CLI to compile the Rust binary and produce
//      the Windows NSIS installer (`tauri build --bundles nsis`).
//      Tauri reads the app version straight from package.json (see
//      "version": "../package.json" in src-tauri/tauri.conf.json), so
//      there is exactly one place that number is ever set.
//   2. Copies the generated installer into `release/` at the repo
//      root, renamed to the friendly `Chronon-Setup-<version>.exe`
//      pattern, so every build leaves one predictable artifact behind
//      regardless of Tauri's internal bundle-folder naming.
//   3. If `--publish` was passed (see the "release" npm script),
//      hands off to publish-release.js to create/update a draft
//      GitHub release and upload the artifact(s) — mirroring Project
//      Playnck's "npm run build" vs "npm run release" split.
//
// This script only orchestrates the build/copy/publish steps; it
// never touches application source. Nothing here changes what
// Chronon does or looks like — it only decides how the finished
// binary gets packaged and handed off.
// ================================================================

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const ROOT = path.join(__dirname, "..");
const pkg = require(path.join(ROOT, "package.json"));

const NSIS_BUNDLE_DIR = path.join(ROOT, "src-tauri", "target", "release", "bundle", "nsis");
const RELEASE_DIR = path.join(ROOT, "release");
const FRIENDLY_NAME = `Chronon-Setup-${pkg.version}.exe`;

console.log(`Building Chronon v${pkg.version} (Windows NSIS installer)...`);
execSync("npx tauri build --bundles nsis", { stdio: "inherit", cwd: ROOT });

if (!fs.existsSync(NSIS_BUNDLE_DIR)) {
  throw new Error(
    `Expected NSIS output at ${NSIS_BUNDLE_DIR}, but it wasn't created. ` +
      "Check the tauri build output above for bundler errors."
  );
}

const installer = fs.readdirSync(NSIS_BUNDLE_DIR).find((f) => f.endsWith("-setup.exe"));
if (!installer) {
  throw new Error(`No installer .exe found in ${NSIS_BUNDLE_DIR}`);
}

fs.mkdirSync(RELEASE_DIR, { recursive: true });
const destPath = path.join(RELEASE_DIR, FRIENDLY_NAME);
fs.copyFileSync(path.join(NSIS_BUNDLE_DIR, installer), destPath);

console.log(`\nInstaller ready: release/${FRIENDLY_NAME}`);
console.log(
  `(Tauri's original artifact is untouched at src-tauri/target/release/bundle/nsis/${installer})`
);

// Any updater artifacts (a .sig signature file plus latest.json) that a
// future auto-update setup produces alongside the installer get copied
// into release/ too, so publish-release.js only ever has to look in one
// folder. Today this loop simply finds nothing and does nothing.
for (const file of fs.readdirSync(NSIS_BUNDLE_DIR)) {
  if (file.endsWith(".sig") || file === "latest.json") {
    fs.copyFileSync(path.join(NSIS_BUNDLE_DIR, file), path.join(RELEASE_DIR, file));
    console.log(`Copied updater artifact: release/${file}`);
  }
}

if (process.argv.includes("--publish")) {
  execSync(`node ${JSON.stringify(path.join(__dirname, "publish-release.js"))}`, {
    stdio: "inherit",
    cwd: ROOT
  });
}
