#!/usr/bin/env bash
# High-performance launcher for 3D Paintcraft on Linux / Bazzite OS
set -e
cd "$(dirname "$0")"

# Dedicated NVIDIA GPU and Vulkan backend flags
export WGPU_BACKEND=vulkan
export __NV_PRIME_RENDER_OFFLOAD=1
export __GLX_VENDOR_LIBRARY_NAME=nvidia

ARGS=()
for arg in "$@"; do
    case "$arg" in
        --x11)
            echo "[3D Paintcraft] Forcing X11 / XWayland backend..."
            export WINIT_UNIX_BACKEND=x11
            export GDK_BACKEND=x11
            ;;
        --wayland)
            echo "[3D Paintcraft] Forcing native Wayland backend..."
            export WINIT_UNIX_BACKEND=wayland
            ;;
        --no-vsync)
            echo "[3D Paintcraft] Disabling VSync presentation..."
            export PAINTCRAFT_PRESENT_MODE=no_vsync
            ;;
        *)
            ARGS+=("$arg")
            ;;
    esac
done

exec cargo run --release -- "${ARGS[@]}"

