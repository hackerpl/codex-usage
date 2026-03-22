# Codex Usage

[![CI](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml)
[![Release](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml)

Codex Usage is a standalone Tauri desktop app for viewing local Codex account usage, switching accounts, and managing auto-switch behavior from a tray-style GUI. Now with full Windows and Linux support.

## Features

- **Multi-Platform Support**: Full feature parity between Windows and Linux.
- **Usage Modes**:
  - **Local Mode**: Reads real-time Codex state from `~/.codex` session rollouts.
  - **API Mode**: Native usage polling from the OpenAI backend via OS primitives (PowerShell/curl), bypassing programmed HTTP libraries to avoid bot detection and minimize binary size.
- **Account Management**:
  - View current account details, 5h usage, and weekly usage.
  - Switch accounts instantly by rewriting `auth.json` and registry state.
  - Launch native `codex login` directly from the GUI to add new accounts.
- **Automated Switching**:
  - Installs and manages background auto-switch services.
  - **Windows**: Integrated with Windows Task Scheduler (silent execution).
  - **Linux**: Integrated with `systemd` user timers.
- **Modern UI/UX**:
  - Multi-language support (**English** & **Chinese**).
  - Custom beautified cross-platform scrollbars.
  - Premium tray icon and responsive glassmorphism-inspired design.
  - Automatic refresh when local Codex files change.

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

Run the desktop app in development mode:

```bash
# Windows
npm run tauri:dev

# Linux
./scripts/desktop-env.sh npm run tauri:dev
```

Create a production build:

```bash
npm run tauri:build
```

## Auto Switch Service

The GUI `Settings` panel can install, start, stop, uninstall, and manually run the background auto-switch check.

### CLI Commands (Windows/Linux)

The compiled binary supports several flags for service management:

```bash
codex-usage --auto-switch-check
codex-usage --install-auto-switch-service
codex-usage --start-auto-switch-service
codex-usage --stop-auto-switch-service
codex-usage --uninstall-auto-switch-service
```

## Release Process

Push a version tag to trigger the release workflow:

```bash
git tag v0.1.1
git push origin v0.1.1
```

The GitHub Actions release workflow builds desktop bundles for Linux, macOS, and Windows and uploads them to the matching GitHub Release.
