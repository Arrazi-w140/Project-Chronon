# Releasing Chronon

Once this is set up once, releasing is:

```powershell
npm run release
```

That builds the installer, signs it, generates the updater manifest, and publishes a GitHub release with everything attached — no other manual steps, no PowerShell env vars to set each time.

## One-time setup

### 1. Signing key (already done if you ran `tauri signer generate` before)

`npm run release` looks for your private key at:

```
%USERPROFILE%\.tauri\chronon.key
```

If it's there, it's loaded automatically — nothing else to do. If you haven't generated one yet:

```powershell
npx @tauri-apps/cli signer generate -w %USERPROFILE%\.tauri\chronon.key
```

Paste the **public** key it prints into `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`. That's a one-time step too — you never touch it again.

### 2. Signing key password + GitHub token

Both are loaded automatically if either:
- an environment variable is already set (`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `GH_TOKEN`), **or**
- they're in a local file, `.release-secrets.json`, at the project root.

Pick whichever of these two you're more comfortable with:

**Option A — `.release-secrets.json` (simplest)**

Copy `.release-secrets.example.json` to `.release-secrets.json` and fill it in:

```json
{
  "signingKeyPassword": "the password you set when generating the key",
  "githubToken": "ghp_xxxxxxxxxxxxxxxxxxxx"
}
```

This file is already in `.gitignore` — it will never be committed. It's plain text on disk, protected only by your normal Windows account file permissions (the same protection your `chronon.key` file already relies on).

**Option B — persistent environment variables (more secure, one-time setup)**

A file sitting next to your encrypted key that also holds the password to decrypt it somewhat defeats the point of encrypting it in the first place — if someone gets one file, they likely get both. A persistent **user-level** environment variable doesn't have that problem, and it's still "set once, forget it":

```powershell
setx TAURI_SIGNING_PRIVATE_KEY_PASSWORD "the password you set when generating the key"
setx GH_TOKEN "ghp_xxxxxxxxxxxxxxxxxxxx"
```

`setx` writes to your Windows user profile, not the current shell — close and reopen your terminal once afterward for it to take effect. It'll then be there in every future session automatically, exactly like a value you'd type in System Properties → Environment Variables.

Either option satisfies "no manual steps before `npm run release`" — pick A if you want the least ceremony, B if you'd rather the password not live in any file at all.

### GitHub token scope

Classic personal access token, scope: **`repo`** (Settings → Developer settings → Personal access tokens → Tokens (classic) → Generate new token). Needs to be able to create releases and upload assets on `Project-Chronon`.

## What happens on `npm run release`

1. Loads the signing key from disk (or `TAURI_SIGNING_PRIVATE_KEY` if already set) and the password from wherever it's configured.
2. Runs `tauri build --bundles nsis` — produces the signed installer + `.sig`.
3. Builds `release/latest.json` from that `.sig`.
4. Creates (or reuses) a GitHub release for the current `package.json` version, uploads the installer + `.sig` + `latest.json`, then **publishes it immediately** — not a draft.
5. Chronon installs already in the wild see the update within their next background check (or whenever someone clicks "Check for Updates Now").

Re-running `npm run release` for the same version is safe — it replaces assets on the existing release rather than duplicating them, and if it's already published, the publish step just confirms that and does nothing further.

## Plain `npm run build` (no publish)

Still works exactly as before, with or without secrets configured — it only builds and copies the installer into `release/`. It signs the build too (same automatic key loading), so `release/` always has a real `.sig`, but it never touches GitHub.
