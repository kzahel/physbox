#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/host-env.sh"
physbox5_host_import_graphical_session

if [[ "${1:-}" = --linux-x11 ]]; then
    shift
    if [[ "$(physbox5_host_platform)" = linux && -n "${DISPLAY:-}" ]]; then
        unset WAYLAND_DISPLAY
    fi
fi

(($#)) || { echo "usage: $0 COMMAND [ARG ...]" >&2; exit 2; }
exec "$@"
