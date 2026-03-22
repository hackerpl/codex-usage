# Codex Usage

A standalone desktop app for managing local Codex accounts, usage snapshots, and account switching.

## Current scope

- `M1` desktop UI with a reference-style account popover
- Reads and writes `~/.codex/accounts/registry.json`
- Launches native `codex login` from the GUI to add accounts
- Supports local account switching through Tauri commands
- Owns its own Linux auto-switch timer: `codex-usage-autoswitch.timer`
- Can run manual and scheduled auto-switch checks without `codex-auth`
- Falls back to mock data when running outside Tauri

## Run

```bash
npm install
npm run dev
```

For the desktop app:

```bash
npm run tauri:dev
```

For the compiled desktop binary:

```bash
./scripts/desktop-env.sh src-tauri/target/release/codex-usage
```

## Auto switch service

The GUI `Settings` panel can install, start, stop, uninstall, and manually run the background auto-switch check on Linux.

The same actions are also available from the binary:

```bash
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --auto-switch-check
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --install-auto-switch-service
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --start-auto-switch-service
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --stop-auto-switch-service
./scripts/desktop-env.sh src-tauri/target/debug/codex-usage --uninstall-auto-switch-service
```

## Environment note

This repo expects a Rust toolchain for Tauri (`rustc`, `cargo`) and Node.js.

## Ubuntu setup

For Ubuntu 24.04, you can bootstrap the machine with:

```bash
./scripts/setup-ubuntu.sh
```

That script installs:

- Tauri system libraries
- Rust via `rustup`
- project npm dependencies
