// ================================================================
// build-scripts/load-release-secrets.js
// ----------------------------------------------------------------
// Lets `npm run release` run with nothing set up in the shell at all
// (not even a persistent env var, if you use the secrets file below).
// Imported by both build.js (signing key + password, before it calls
// `tauri build`) and publish-release.js (GitHub token).
//
// Resolution order for every secret is: an already-set environment
// variable always wins first — so nothing changes for anyone who
// prefers exporting things themselves (or a future CI setup) — and
// only falls back to the sources below if that env var is unset.
//
//   TAURI_SIGNING_PRIVATE_KEY
//     -> read straight from %USERPROFILE%\.tauri\chronon.key, the
//        file `tauri signer generate` wrote. Never copied anywhere
//        else and never logged.
//
//   TAURI_SIGNING_PRIVATE_KEY_PASSWORD, GH_TOKEN
//     -> read from .release-secrets.json at the project root, if it
//        exists. That file is git-ignored (see .gitignore) and is
//        never read for anything except these two values.
//
// See RELEASE.md for how to create .release-secrets.json, and for a
// note on why the signing password specifically has a more secure
// alternative (a persistent env var) that's worth considering instead
// of putting it in that file.
// ================================================================

import fs from "fs";
import os from "os";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT = path.join(__dirname, "..");

const SECRETS_FILE = path.join(ROOT, ".release-secrets.json");
const DEFAULT_KEY_PATH = path.join(os.homedir(), ".tauri", "chronon.key");

let cachedSecrets = null;

function readSecretsFile() {
  if (cachedSecrets) return cachedSecrets;
  if (!fs.existsSync(SECRETS_FILE)) {
    cachedSecrets = {};
    return cachedSecrets;
  }
  try {
    cachedSecrets = JSON.parse(fs.readFileSync(SECRETS_FILE, "utf8"));
  } catch (err) {
    console.warn(`Couldn't parse ${SECRETS_FILE} (${err.message}) — ignoring it.`);
    cachedSecrets = {};
  }
  return cachedSecrets;
}

// Populates TAURI_SIGNING_PRIVATE_KEY (and, where possible,
// TAURI_SIGNING_PRIVATE_KEY_PASSWORD) in process.env before build.js
// calls `tauri build`. Returns true if a key was loaded (from the
// environment or from disk), false if the build is going ahead
// unsigned.
export function loadSigningKey() {
  if (process.env.TAURI_SIGNING_PRIVATE_KEY) {
    return true;
  }

  const secrets = readSecretsFile();
  const keyPath = secrets.signingKeyPath || DEFAULT_KEY_PATH;

  if (!fs.existsSync(keyPath)) {
    console.log(
      `No signing key found at ${keyPath} — building unsigned (no .sig/latest.json will be produced this run).`
    );
    return false;
  }

  process.env.TAURI_SIGNING_PRIVATE_KEY = fs.readFileSync(keyPath, "utf8");
  console.log(`Loaded signing key from ${keyPath}`);

  if (!process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    if (secrets.signingKeyPassword) {
      process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = secrets.signingKeyPassword;
      console.log(`Loaded signing key password from ${path.basename(SECRETS_FILE)}`);
    } else {
      console.log(
        `No signing key password found (checked TAURI_SIGNING_PRIVATE_KEY_PASSWORD and ` +
          `"signingKeyPassword" in ${path.basename(SECRETS_FILE)}). If ${path.basename(keyPath)} needs one, ` +
          "the build will fail below with a clear error — see RELEASE.md to set it up."
      );
    }
  }

  return true;
}

// Populates GH_TOKEN in process.env before publish-release.js talks to
// the GitHub API, if it isn't already set some other way.
export function loadGithubToken() {
  if (process.env.GH_TOKEN || process.env.GITHUB_TOKEN) return;

  const secrets = readSecretsFile();
  if (secrets.githubToken) {
    process.env.GH_TOKEN = secrets.githubToken;
    console.log(`Loaded GitHub token from ${path.basename(SECRETS_FILE)}`);
  }
}
