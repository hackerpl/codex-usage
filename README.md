# Codex Usage

[![CI](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml)
[![Release](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml)

Codex Usage is a standalone Tauri desktop app for viewing local Codex account usage, switching accounts, and managing auto-switch behavior from a tray-style GUI.

## What It Does

- Reads real local Codex state from `~/.codex`
- Shows current account, 5h usage, and weekly usage
- Switches accounts by rewriting local `auth.json` and registry state
- Starts native `codex login` directly from the GUI to add accounts
- Runs as a tray app with a menu-like popover window
- Installs and manages its own Linux user timer: `codex-usage-autoswitch.timer`
- Refreshes automatically when local Codex files change

## Download

- Latest builds: [GitHub Releases](https://github.com/hackerpl/codex-usage/releases)

## Requirements

- Node.js 20+
- Rust stable toolchain
- A local Codex CLI install with access to `~/.codex`

## Local Development

Install dependencies:

```bash
npm install
```

Run the web preview:

```bash
npm run dev
```

Run the desktop app:

```bash
npm run tauri:dev
```

Create a production build:

```bash
npm run build
npm run tauri:build
```

Run the compiled binary directly:

```bash
./scripts/desktop-env.sh src-tauri/target/release/codex-usage
```

## Ubuntu Setup

For Ubuntu 24.04, bootstrap the machine with:

```bash
./scripts/setup-ubuntu.sh
```

That script installs the Linux Tauri system libraries, Rust via `rustup`, and the project npm dependencies.

## Auto Switch Service

The GUI `Settings` panel can install, start, stop, uninstall, and manually run the background auto-switch check on Linux.

The same actions are available from the binary:

```bash
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --auto-switch-check
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --install-auto-switch-service
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --start-auto-switch-service
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --stop-auto-switch-service
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --uninstall-auto-switch-service
```

## Release Process

Push a version tag to trigger the release workflow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The GitHub Actions release workflow builds desktop bundles for Linux, macOS, and Windows and uploads them to the matching GitHub Release.
