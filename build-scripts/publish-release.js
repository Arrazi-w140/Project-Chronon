// ================================================================
// build-scripts/publish-release.js
// ----------------------------------------------------------------
// Publishes the contents of `release/` (produced by build.js) to a
// GitHub release for the current version, then publishes it
// immediately — no manual "hit publish on GitHub" step.
//
// It's still created as a draft internally for a moment first: asking
// GitHub to create a release as already-published on a brand new tag
// intermittently fails with a 422 ("Published releases must have a
// valid tag") before the tag has had time to settle server-side.
// Creating as a draft, uploading every asset, then flipping it to
// published as the very last step avoids that.
//
// Needs GH_TOKEN or GITHUB_TOKEN — loaded automatically from
// .release-secrets.json if neither is already set in the environment;
// see load-release-secrets.js and RELEASE.md. Without a token this is
// a no-op — `npm run build` never needs one, only `npm run release`
// does.
//
// Safe to re-run: if a release for this version already exists, its
// assets are replaced (delete-then-upload per file) rather than
// duplicated, and if it's already published, the final publish step
// just confirms that and does nothing further.
// ================================================================

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { loadGithubToken } from "./load-release-secrets.js";

loadGithubToken();

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

// Flips a draft release to published, with a couple of retries — the
// "Published releases must have a valid tag" 422 mentioned up top has
// been intermittent even at this stage, moments after the tag itself
// was created. Mirrors Project Playnck's reconcile-github-release.js.
async function publishRelease(release) {
  if (release.draft === false) {
    console.log(`Release ${release.html_url} is already published.`);
    return release;
  }

  const attempts = 3;
  for (let i = 1; i <= attempts; i++) {
    try {
      const published = await gh("PATCH", `/repos/${OWNER}/${REPO}/releases/${release.id}`, { draft: false });
      console.log(`Published: ${published.html_url}`);
      return published;
    } catch (err) {
      if (i === attempts) {
        console.warn(`Could not auto-publish after ${attempts} attempts (${err.message}).`);
        console.warn(`The release is uploaded and ready — publish it manually from: ${release.html_url}`);
        return release;
      }
      console.log(`Publish attempt ${i} failed, retrying in 5s... (${err.message})`);
      await new Promise((r) => setTimeout(r, 5000));
    }
  }
}

async function main() {
  if (!TOKEN) {
    console.log("No GH_TOKEN/GITHUB_TOKEN set, and none found in .release-secrets.json — skipping GitHub publish.");
    console.log("See RELEASE.md for how to set one up (a GitHub personal access token with 'repo' scope).");
    return;
  }
  if (!fs.existsSync(RELEASE_DIR)) {
    throw new Error(`Nothing to publish — ${RELEASE_DIR} doesn't exist. Run "npm run build" first.`);
  }

  let release = await findRelease();
  if (!release) {
    release = await createRelease();
  } else {
    console.log(`Reusing existing release ${TAG} (${release.draft ? "draft" : "published"})...`);
  }

  // build.js wipes release/ before every build, so in the normal
  // `npm run release` flow this folder only ever has this version's
  // files in it. This filter is a second line of defense for the case
  // where this script gets run on its own against a release/ left
  // over from something else (an old build, an interrupted run) —
  // every filename build.js produces embeds the version (e.g.
  // Chronon-Setup-0.1.2.exe, Chronon_0.1.2_x64-setup.exe.sig) except
  // latest.json, whose name is fixed but whose content is
  // version-specific, so it's always kept too.
  const escapedVersion = pkg.version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const versionPattern = new RegExp(escapedVersion);
  const allFiles = fs.readdirSync(RELEASE_DIR).filter((f) => !f.startsWith("."));
  const files = allFiles.filter((f) => f === "latest.json" || versionPattern.test(f));
  const skipped = allFiles.filter((f) => !files.includes(f));

  if (skipped.length) {
    console.warn(
      `Skipping ${skipped.length} file(s) in ${RELEASE_DIR} that don't match version ${pkg.version}: ` +
        `${skipped.join(", ")}`
    );
    console.warn(`These look like leftovers from another build — run "npm run build" again before publishing.`);
  }
  if (!files.length) {
    throw new Error(`${RELEASE_DIR} has no files matching version ${pkg.version} — nothing to upload.`);
  }

  for (const file of files) {
    await uploadAsset(release, path.join(RELEASE_DIR, file));
  }

  release = await publishRelease(release);
  console.log(`\nRelease ready: ${release.html_url}`);
}

main().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
