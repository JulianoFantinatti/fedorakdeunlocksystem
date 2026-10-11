#!/bin/bash
# Fedora KDE Biometric Unlock System Installer
# Sets up native PolicyKit biometric agent, PAM mode gating, wrappers, and sleep recovery.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=========================================================="
echo "    Fedora KDE Biometric Unlock System - Installer        "
echo "=========================================================="
echo ""

# 1. Check prerequisites
echo "[*] Checking build dependencies..."
for pkg in cargo ffmpeg git; do
    if ! command -v "$pkg" >/dev/null 2>&1; then
        echo "[!] Missing '$pkg'. Installing via dnf..."
        sudo dnf install -y "$pkg"
    fi
done

# 2. Compile Rust Polkit Agent
echo "[*] Compiling biometric-polkit-agent (release profile)..."
cd "$SCRIPT_DIR/agent"
cargo build --release

# 3. Install Binaries & Helpers
echo "[*] Installing binaries to /usr/local/bin..."
sudo install -m 755 "$SCRIPT_DIR/agent/target/release/biometric-polkit-agent" /usr/local/bin/biometric-polkit-agent
sudo install -m 755 "$SCRIPT_DIR/config/wrappers/check-biometric-mode" /usr/local/bin/check-biometric-mode
sudo install -m 755 "$SCRIPT_DIR/config/wrappers/w" /usr/local/bin/w
sudo install -m 755 "$SCRIPT_DIR/config/wrappers/sudo" /usr/local/bin/sudo

# Also install to ~/.local/bin as a fallback
mkdir -p "$HOME/.local/bin"
cp "$SCRIPT_DIR/agent/target/release/biometric-polkit-agent" "$HOME/.local/bin/biometric-polkit-agent"
chmod 755 "$HOME/.local/bin/biometric-polkit-agent"

# 4. Configure PAM Mode Gating
echo "[*] Configuring /etc/pam.d/polkit-1..."
sudo install -m 644 "$SCRIPT_DIR/config/pam/polkit-1" /etc/pam.d/polkit-1

# 5. Configure Systemd User Service Override
echo "[*] Configuring systemd user override for KDE Plasma Polkit Agent..."
OVERRIDE_DIR="$HOME/.config/systemd/user/plasma-polkit-agent.service.d"
mkdir -p "$OVERRIDE_DIR"
cp "$SCRIPT_DIR/config/systemd/override.conf" "$OVERRIDE_DIR/override.conf"

# 6. Install Suspend/Resume Recovery Hook
if systemctl list-unit-files | grep -q "python3-validity"; then
    echo "[*] Installing suspend/resume sleep hook for python3-validity..."
    sudo mkdir -p /usr/lib/systemd/system-sleep
    sudo install -m 755 "$SCRIPT_DIR/config/systemd/python3-validity-resume.sh" /usr/lib/systemd/system-sleep/python3-validity.sh
    sudo systemctl restart python3-validity.service open-fprintd.service 2>/dev/null || true
fi

# 7. Reload & Restart User Agent
echo "[*] Restarting plasma-polkit-agent.service..."
systemctl --user daemon-reload
systemctl --user restart plasma-polkit-agent.service

echo ""
echo "=========================================================="
echo "    Installation Complete!                                "
echo "=========================================================="
echo ""
echo "Try running:"
echo "  sudo ls"
echo "or:"
echo "  w ls"
echo "or test the UI preview:"
echo "  ~/.local/bin/biometric-polkit-agent --test"
echo ""
