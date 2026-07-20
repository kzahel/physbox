#!/usr/bin/env bash

physbox5_host_platform_from_uname() {
    case "$1" in
        Darwin) printf 'macos\n' ;;
        Linux) printf 'linux\n' ;;
        *) printf 'unsupported\n' ;;
    esac
}

physbox5_host_platform() {
    physbox5_host_platform_from_uname "$(uname -s)"
}

physbox5_host_apply_environment() {
    local contents="$1" mode="${2:-missing}" line key value current
    while IFS= read -r line; do
        case "$line" in
            DISPLAY=*) key=DISPLAY; value="${line#DISPLAY=}" ;;
            WAYLAND_DISPLAY=*) key=WAYLAND_DISPLAY; value="${line#WAYLAND_DISPLAY=}" ;;
            XAUTHORITY=*) key=XAUTHORITY; value="${line#XAUTHORITY=}" ;;
            XDG_RUNTIME_DIR=*) key=XDG_RUNTIME_DIR; value="${line#XDG_RUNTIME_DIR=}" ;;
            DBUS_SESSION_BUS_ADDRESS=*) key=DBUS_SESSION_BUS_ADDRESS; value="${line#DBUS_SESSION_BUS_ADDRESS=}" ;;
            *) continue ;;
        esac
        current="${!key:-}"
        if [[ -n "$value" && ( "$mode" = force || -z "$current" ) ]]; then
            printf -v "$key" '%s' "$value"
            export "$key"
            PHYSBOX5_HOST_IMPORTED_GRAPHICS=1
        fi
    done <<< "$contents"
}

physbox5_host_import_graphical_session() {
    local mode="${1:-missing}" manager_environment runtime_dir
    PHYSBOX5_HOST_IMPORTED_GRAPHICS=0
    [[ "$(physbox5_host_platform)" = linux ]] || return 0

    if [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
        runtime_dir="/run/user/$(id -u)"
        if [[ -d "$runtime_dir" ]]; then
            export XDG_RUNTIME_DIR="$runtime_dir"
        fi
    fi
    if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" && -S "${XDG_RUNTIME_DIR:-}/bus" ]]; then
        export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
    fi
    command -v systemctl >/dev/null || return 0
    manager_environment="$(systemctl --user show-environment 2>/dev/null)" || return 0
    physbox5_host_apply_environment "$manager_environment" "$mode"
}

physbox5_host_x11_available() {
    [[ -n "${DISPLAY:-}" ]] || return 1
    if command -v xdpyinfo >/dev/null; then
        xdpyinfo >/dev/null 2>&1
    else
        [[ -z "${XAUTHORITY:-}" || -r "$XAUTHORITY" ]]
    fi
}
