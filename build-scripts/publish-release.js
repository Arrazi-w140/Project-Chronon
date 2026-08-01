// ================================================================
// build-scripts/publish-release.js
// ----------------------------------------------------------------
// Publishes the contents of `release/` (produced by build.js) to a
// draft GitHub release — the same "draft first, hit publish when
// you're happy with it" workflow Project Playnck uses.
//
// Requires GH_TOKEN or GITHUB_TOKEN in the environment. Without a
// token this is a no-op — `npm run build` never needs one, only
// `npm run release` does.
//
// Safe to re-run: if a draft for this version already exists, its
// assets are replaced (delete-then-upload per file) rather than
// duplicated, so fixing something and re-running never leaves stale
// or doubled-up assets behind.
//
// Forward-compatible with auto-updates: once tauri-plugin-updater is
// wired up (signing key generated, plugin added, bundle.
// createUpdaterArtifacts enabled), `tauri build` will also emit a
// `.sig` file and a `latest.json` manifest next to the installer.
// build.js already copies those into release/ when present, and this
// script uploads everything it finds there — no changes needed here
// when that day comes.
// ================================================================

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ROOT = path.join(__dirname, "..");
const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, "package.json"), "utf-8"));

const OWNER = "Arrazi-w140";
const REPO = "Project-Chronon";
const TAG = `v${pkg.version}`;
const RELEASE_DIR = path.join(ROOT, "release");

const TOKEN = process.env.GH_TOKEN || process.env.GITHUB_TOKEN;
const API = "https://api.github.com";

function headers(extra) {
  return {
    "User-Agent": "chronon-release-script",
    Authorization: `Bearer ${TOKEN}`,
    Accept: "application/vnd.github+json",
    ...extra
  };
}

async function gh(method, urlPath, body) {
  const res = await fetch(`${API}${urlPath}`, {
    method,
    headers: headers(body ? { "Content-Type": "application/json" } : {}),
    body: body ? JSON.stringify(body) : undefined
  });
  if (!res.ok) {
    throw new Error(`${method} ${urlPath} -> ${res.status}: ${await res.text()}`);
  }
  return res.status === 204 ? null : res.json();
}

async function findRelease() {
  const releases = await gh("GET", `/repos/${OWNER}/${REPO}/releases`);
  return releases.find((r) => r.tag_name === TAG) || null;
}

async function createRelease() {
  console.log(`Creating draft release ${TAG}...`);
  return gh("POST", `/repos/${OWNER}/${REPO}/releases`, {
    tag_name: TAG,
    name: `Chronon ${TAG}`,
    draft: true,
    prerelease: false
  });
}

async function deleteAssetIfPresent(release, name) {
  const existing = release.assets.find((a) => a.name === name);
  if (existing) {
    await gh("DELETE", `/repos/${OWNER}/${REPO}/releases/assets/${existing.id}`);
  }
}

async function uploadAsset(release, filePath) {
  const name = path.basename(filePath);
  await deleteAssetIfPresent(release, name);

  const uploadUrl = release.upload_url.replace("{?name,label}", `?name=${encodeURIComponent(name)}`);
  const buffer = fs.readFileSync(filePath);
  const res = await fetch(uploadUrl, {
    method: "POST",
    headers: headers({ "Content-Type": "application/octet-stream" }),
    body: buffer
  });
  if (!res.ok) {
    throw new Error(`upload ${name} -> ${res.status}: ${await res.text()}`);
  }
  console.log(`Uploaded ${name}`);
}

async function main() {
  if (!TOKEN) {
    console.log("No GH_TOKEN/GITHUB_TOKEN set — skipping GitHub publish.");
    console.log("Set GH_TOKEN (a GitHub personal access token with 'repo' scope) to enable this step.");
    return;
  }
  if (!fs.existsSync(RELEASE_DIR)) {
    throw new Error(`Nothing to publish — ${RELEASE_DIR} doesn't exist. Run "npm run build" first.`);
  }

  let release = await findRelease();
  if (!release) {
    release = await createRelease();
  } else {
    console.log(`Reusing existing draft release ${TAG}...`);
  }

  const files = fs.readdirSync(RELEASE_DIR).filter((f) => !f.startsWith("."));
  if (!files.length) {
    throw new Error(`${RELEASE_DIR} is empty — nothing to upload.`);
  }

  for (const file of files) {
    await uploadAsset(release, path.join(RELEASE_DIR, file));
  }

  console.log(`\nDraft release ready: ${release.html_url}`);
  console.log("Review it on GitHub and publish when you're ready.");
}

main().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
