#!/bin/bash
# Fedora KDE Biometric Unlock System Uninstaller
# Restores default KDE Plasma PolicyKit agent and system PAM configuration.

set -e

echo "[*] Removing /etc/pam.d/polkit-1 (restoring vendor PAM config)..."
sudo rm -f /etc/pam.d/polkit-1

echo "[*] Removing systemd user override..."
rm -f "$HOME/.config/systemd/user/plasma-polkit-agent.service.d/override.conf"
systemctl --user daemon-reload
systemctl --user restart plasma-polkit-agent.service

echo "[*] Removing installed binaries from /usr/local/bin..."
sudo rm -f /usr/local/bin/biometric-polkit-agent
sudo rm -f /usr/local/bin/check-biometric-mode
sudo rm -f /usr/local/bin/w
sudo rm -f /usr/local/bin/sudo
sudo rm -f /usr/lib/systemd/system-sleep/python3-validity.sh
rm -f "$HOME/.local/bin/biometric-polkit-agent"

echo "[+] Default KDE PolicyKit authentication agent and PAM configuration restored!"
