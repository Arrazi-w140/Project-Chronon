# ⏱️ Chronon

> A lightweight, customizable desktop time & date widget, built with Tauri.

Chronon sits on your Windows desktop and shows the time, date, or both, in a layout you control — size, position, background, transparency, row spacing, colors, and custom-loaded fonts.

---

# Project Structure

```
Project-Chronon/
│
├── build-scripts/      # Packaging/release scripts (see "Building" below)
├── src/                # Frontend: index.html, styles.css, main.js, assets
├── src-tauri/           # Rust backend + Tauri config, icons, bundler settings
│   ├── icons/
│   ├── src/
│   └── tauri.conf.json
├── package.json
└── ...
```

---

# Installation (for development)

Clone the repository

```bash
git clone https://github.com/Arrazi-w140/Project-Chronon.git
```

Move into the project

```bash
cd Project-Chronon
```

Install dependencies

```bash
npm install
```

Run Chronon in development mode

```bash
npm run dev
```

---

# Building

To produce a Windows installer

```bash
npm run build
```

This compiles the Rust backend, bundles the frontend, and generates a ready-to-share NSIS installer at:

```
release/Chronon-Setup-<version>.exe
```

(Tauri's own unrenamed artifact is also left in place at `src-tauri/target/release/bundle/nsis/`.)

The installer is a normal Windows setup wizard — Next → Next → Install → Finish — and installs Chronon with a Start Menu shortcut, a Desktop shortcut, and a standard entry in "Apps & features" for uninstalling.

To build **and** publish a draft GitHub release in one step

```bash
npm run release
```

`npm run release` requires a `GH_TOKEN` (or `GITHUB_TOKEN`) environment variable — a GitHub personal access token with `repo` scope — to upload the installer to a draft release. Without it, `npm run release` still builds the installer, it just skips the publish step. Review the draft on GitHub and publish it manually when you're ready.

---

# Requirements

**To build Chronon:**

- Node.js (LTS recommended) + npm
- Rust (stable toolchain) + Cargo
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for Windows (MSVC build tools + WebView2, both of which `tauri build` will point you to if missing)

**To run the installed app:** nothing extra. The installer produces a self-contained native `.exe` — end users don't need Node.js, npm, Rust, or any other development tool. Windows 10/11 already ships the WebView2 runtime Chronon relies on; on the rare machine that's missing it, the installer downloads it automatically during setup.

---

# Technologies Used

- Tauri (Rust)
- HTML5 / CSS3 / JavaScript

---

# Releases

The latest stable releases can always be found on the **Releases** page of this repository.

## Auto-updates (planned)

Chronon's packaging is already organized for this: `build-scripts/build.js` will pick up and publish a `.sig` file and a `latest.json` manifest alongside the installer the moment they exist, without any changes to the release scripts. To turn that on later:

1. Generate a signing key: `npx tauri signer generate -w ~/.tauri/chronon.key`
2. Add `tauri-plugin-updater` and `tauri-plugin-process` to `src-tauri/Cargo.toml`, register them in `src-tauri/src/lib.rs`, and grant their permissions in `src-tauri/capabilities/default.json`
3. Add a `plugins.updater` block to `tauri.conf.json` with the generated public key and an `endpoints` URL pointing at this repo's `latest.json` release asset
4. Set `bundle.createUpdaterArtifacts: true` in `tauri.conf.json`
5. Keep the private key and its password out of source control — store them as CI secrets (e.g. `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) for release builds

---

# License

This project is released under the included LICENSE (if present) or is otherwise all rights reserved.

---

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
