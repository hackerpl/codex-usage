#!/usr/bin/env bash

set -euo pipefail

clean_path="$(
  printf '%s' "${PATH:-}" \
    | tr ':' '\n' \
    | grep -v '/snap/' \
    | paste -sd: -
)"

clean_xdg_data_dirs="$(
  printf '%s' "${XDG_DATA_DIRS:-/usr/share/ubuntu:/usr/share/gnome:/usr/local/share:/usr/share:/var/lib/snapd/desktop}" \
    | tr ':' '\n' \
    | grep -v "${HOME}/snap/code" \
    | grep -v '^/snap/code/' \
    | paste -sd: -
)"

if [[ -z "$clean_path" ]]; then
  clean_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
fi

if [[ -z "$clean_xdg_data_dirs" ]]; then
  clean_xdg_data_dirs="/usr/share/ubuntu:/usr/share/gnome:/usr/local/share:/usr/share:/var/lib/snapd/desktop"
fi

exec env -i \
  HOME="${HOME}" \
  USER="${USER:-$(id -un)}" \
  LOGNAME="${LOGNAME:-${USER:-$(id -un)}}" \
  SHELL="${SHELL:-/bin/bash}" \
  LANG="${LANG:-en_US.UTF-8}" \
  TERM="${TERM:-xterm-256color}" \
  PATH="${clean_path}:${HOME}/.cargo/bin" \
  DISPLAY="${DISPLAY:-}" \
  WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-}" \
  XDG_DATA_HOME="${HOME}/.local/share" \
  XDG_CONFIG_HOME="${HOME}/.config" \
  XDG_CACHE_HOME="${HOME}/.cache" \
  XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-}" \
  XDG_SESSION_TYPE="${XDG_SESSION_TYPE:-}" \
  XDG_CURRENT_DESKTOP="${XDG_CURRENT_DESKTOP:-}" \
  DESKTOP_SESSION="${DESKTOP_SESSION:-}" \
  XDG_CONFIG_DIRS="${XDG_CONFIG_DIRS:-/etc/xdg/xdg-ubuntu:/etc/xdg}" \
  XDG_DATA_DIRS="${clean_xdg_data_dirs}" \
  CARGO_HOME="${CARGO_HOME:-${HOME}/.cargo}" \
  RUSTUP_HOME="${RUSTUP_HOME:-${HOME}/.rustup}" \
  "$@"
