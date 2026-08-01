// ================================================================
// build-scripts/build.js
// ----------------------------------------------------------------
// `npm run build` (and `npm run release`) run this instead of calling
// `tauri build` directly:
//   1. Loads the updater signing key automatically (see
//      load-release-secrets.js) — from TAURI_SIGNING_PRIVATE_KEY if
//      already set, otherwise straight from
//      %USERPROFILE%\.tauri\chronon.key.
//   2. Runs the real Tauri CLI to compile the Rust binary and produce
//      the Windows NSIS installer (`tauri build --bundles nsis`).
//      Tauri reads the app version straight from package.json (see
//      "version": "../package.json" in src-tauri/tauri.conf.json), so
//      there is exactly one place that number is ever set.
//   3. Copies the generated installer into `release/` at the repo
//      root, renamed to the friendly `Chronon-Setup-<version>.exe`
//      pattern, so every build leaves one predictable artifact behind
//      regardless of Tauri's internal bundle-folder naming.
//   4. If `--publish` was passed (see the "release" npm script),
//      hands off to publish-release.js to create/update and publish a
//      GitHub release with the artifact(s) attached.
//
// This script only orchestrates the build/copy/publish steps; it
// never touches application source. Nothing here changes what
// Chronon does or looks like — it only decides how the finished
// binary gets packaged and handed off.
// ================================================================

import fs from "fs";
import path from "path";
import { execSync } from "child_process";
import { fileURLToPath } from "url";
import { loadSigningKey } from "./load-release-secrets.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ROOT = path.join(__dirname, "..");
const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, "package.json"), "utf-8"));

const NSIS_BUNDLE_DIR = path.join(ROOT, "src-tauri", "target", "release", "bundle", "nsis");
const RELEASE_DIR = path.join(ROOT, "release");
const FRIENDLY_NAME = `Chronon-Setup-${pkg.version}.exe`;

// Same repo publish-release.js uploads to — the updater manifest's "url"
// below has to point at where that installer will actually live once
// uploaded, which follows GitHub's predictable
// /releases/download/<tag>/<asset> URL shape.
const OWNER = "Arrazi-w140";
const REPO = "Project-Chronon";

console.log(`Building Chronon v${pkg.version} (Windows NSIS installer)...`);

// `release/` is a staging area for THIS build's output only (see
// .gitignore: "the source of truth is src-tauri/target/release/bundle/,
// this is just the friendly-named copy"), never a cache. Wipe it before
// every build so nothing from a previous version (or a previous
// interrupted run) can survive into this one — publish-release.js later
// uploads every file it finds in here, so anything stale left behind
// would get uploaded as if it belonged to this release.
fs.rmSync(RELEASE_DIR, { recursive: true, force: true });
fs.mkdirSync(RELEASE_DIR, { recursive: true });

loadSigningKey();
execSync("npx tauri build --bundles nsis", { stdio: "inherit", cwd: ROOT });

if (!fs.existsSync(NSIS_BUNDLE_DIR)) {
  throw new Error(
    `Expected NSIS output at ${NSIS_BUNDLE_DIR}, but it wasn't created. ` +
      "Check the tauri build output above for bundler errors."
  );
}

// Tauri/Cargo never clean out src-tauri/target/release/bundle/nsis
// between builds (that's their call, not ours — the folder doubles as
// part of the incremental build cache), so it can easily hold
// installers from several past versions side by side. Matching on
// "-setup.exe" alone — which is what this used to do — just grabs
// whichever file happens to come back first from readdirSync, with no
// guarantee that's the one we just built; that's how a stale prior
// version's installer/.sig got picked up and shipped before. Require
// the exact current package.json version in the filename instead, so a
// leftover from an old build can never be selected here.
const escapedVersion = pkg.version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const installerPattern = new RegExp(`_${escapedVersion}_.*-setup\\.exe$`);
const installerCandidates = fs.readdirSync(NSIS_BUNDLE_DIR).filter((f) => installerPattern.test(f));

if (installerCandidates.length === 0) {
  const found = fs.readdirSync(NSIS_BUNDLE_DIR).join(", ") || "(nothing)";
  throw new Error(
    `No installer for version ${pkg.version} found in ${NSIS_BUNDLE_DIR}. ` +
      `Files present: ${found}. Check the tauri build output above for bundler errors, ` +
      "or confirm package.json's version matches what was just built."
  );
}
if (installerCandidates.length > 1) {
  throw new Error(
    `Found more than one installer matching version ${pkg.version} in ${NSIS_BUNDLE_DIR}: ` +
      `${installerCandidates.join(", ")}. Not sure which one to trust — clear out that folder ` +
      "(or run a clean build) and try again."
  );
}
const installer = installerCandidates[0];

const destPath = path.join(RELEASE_DIR, FRIENDLY_NAME);
fs.copyFileSync(path.join(NSIS_BUNDLE_DIR, installer), destPath);

console.log(`\nInstaller ready: release/${FRIENDLY_NAME}`);
console.log(
  `(Tauri's original artifact is untouched at src-tauri/target/release/bundle/nsis/${installer})`
);

// Tauri's updater plugin only produces a per-installer .sig file
// alongside the .exe (see "createUpdaterArtifacts" in tauri.conf.json) —
// it does NOT assemble the latest.json manifest that the running app's
// updater endpoint (also in tauri.conf.json) actually reads. That
// manifest gets built here instead, straight from that .sig file's
// content, then written into release/ so publish-release.js uploads it
// as a release asset alongside the installer.
//
// The .sig only exists if a signing key was actually loaded above — a
// plain `npm run build` with no key at %USERPROFILE%\.tauri\chronon.key
// and no TAURI_SIGNING_PRIVATE_KEY set still produces a normal, working
// installer, it just skips this block and nothing update-related gets
// published. See RELEASE.md for how the key gets loaded.
const sigFile = `${installer}.sig`;
const sigPath = path.join(NSIS_BUNDLE_DIR, sigFile);

if (fs.existsSync(sigPath)) {
  fs.copyFileSync(sigPath, path.join(RELEASE_DIR, sigFile));
  console.log(`Copied updater artifact: release/${sigFile}`);

  const signature = fs.readFileSync(sigPath, "utf8").trim();
  const tag = `v${pkg.version}`;
  const manifest = {
    version: pkg.version,
    notes: process.env.RELEASE_NOTES || `Chronon ${tag}`,
    pub_date: new Date().toISOString(),
    platforms: {
      // Windows-only for now, matching bundle.targets ["nsis"] above.
      // Add more platform keys here if Chronon ever ships for
      // macOS/Linux too — see Tauri's updater docs for the OS-ARCH
      // key format.
      "windows-x86_64": {
        signature,
        url: `https://github.com/${OWNER}/${REPO}/releases/download/${tag}/${encodeURIComponent(FRIENDLY_NAME)}`
      }
    }
  };
  fs.writeFileSync(path.join(RELEASE_DIR, "latest.json"), JSON.stringify(manifest, null, 2));
  console.log("Wrote release/latest.json (auto-update manifest)");
} else {
  console.log(
    "No .sig file found — skipping the update manifest. " +
      "Set TAURI_SIGNING_PRIVATE_KEY before building to publish an update users' apps can see."
  );
}

if (process.argv.includes("--publish")) {
  execSync(`node ${JSON.stringify(path.join(__dirname, "publish-release.js"))}`, {
    stdio: "inherit",
    cwd: ROOT
  });
}
