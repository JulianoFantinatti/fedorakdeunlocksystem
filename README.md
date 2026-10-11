# Fedora KDE Biometric Unlock System

[![Fedora 40+](https://img.shields.io/badge/Fedora-40%20%7C%2041%20%7C%2042%20%7C%2043%20%7C%2044-blue.svg)](https://fedoraproject.org/)
[![KDE Plasma 6](https://img.shields.io/badge/KDE%20Plasma-6%20(Wayland)-5277D3.svg)](https://kde.org/plasma-desktop/)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust%20%26%20Slint-orange.svg)](https://slint.dev/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

A native, hardware-accelerated **Windows Hello-style** authentication modal for **Fedora Linux (KDE Plasma 6, Wayland)** with dual biometrics: **Infrared Face Recognition**, **Validity/Synaptics Fingerprint**, and **Password** authentication.

Replaces the default KDE PolicyKit prompt with an audited, low-memory Rust agent featuring a live camera stream, iOS-style scanning beam animation, and dynamic PAM mode isolation.

---

## Features

* **3-Option Unified Modal Dialog**:
  * **🖹 Fingerprint (Default)**: iOS-style animated scanning laser across biometric ridges. Sensor waits for finger touch while the IR camera stays off.
  * **📷 Face Recognition**: Full-square live webcam stream with HUD corner brackets and scanning laser. Neural verification via local IR camera daemon.
  * **🔑 Password**: Auto-focused password field with instant authentication.
* **Dual Navigation**:
  * **Keyboard**: Cycle modes with <kbd>←</kbd> / <kbd>→</kbd> arrow keys, press <kbd>Enter</kbd> to submit, <kbd>Esc</kbd> to cancel.
  * **Mouse**: Click mode pill tabs, OK, or Cancel.
* **Zero-Backdoor Audited Security**:
  * Integrates directly with Fedora's official setuid [`/usr/lib/polkit-1/polkit-agent-helper-1`](docs/architecture.md).
  * Uses the Rust `zeroize` crate to scrub password buffers from RAM immediately upon submission.
  * Zero credential caching, zero replay attacks, zero plaintext files.
* **Strict PAM Mode Isolation**:
  * The IR camera **only** powers on when Face mode is selected.
  * The fingerprint sensor **only** activates when Fingerprint mode is selected.
  * Password mode bypasses all biometrics for instant fallback.
* **Working Directory Preservation**:
  * Standard `pkexec` resets `$PWD` to `/root`. Our `w` and `sudo` wrappers capture `$PWD` and restore your working directory so `sudo ls` lists your current folder.
* **Suspend/Resume TLS Auto-Recovery**:
  * Includes a systemd sleep hook that restarts Validity services on resume, eliminating the notorious `Unexpected TLS version 4 0` error.
* **Lightweight & High Performance**:
  * Uses only **~20MB RAM** (down from ~92MB in the default KDE Qt6 agent).

---

## Architecture Overview

```
User Terminal / Desktop Action
              |
     [sudo]   |   [pkexec / w]
        \     |     /
         v    v    v
  PolicyKit Authority (polkitd on System D-Bus)
              |
              v  (BeginAuthentication)
  biometric-polkit-agent (Rust / Slint / Wayland)
     ├── Mode 0: Fingerprint -> open-fprintd / python3-validity
     ├── Mode 1: Face -> Live Webcam Feed + pam_visage (IR Camera)
     └── Mode 2: Password -> Zeroized buffer -> pam_unix
              |
              v
   polkit-agent-helper-1 (Audited PAM Verification)
              |
              v
       Action Granted
```

For in-depth details, see [Architecture & Security Specification](docs/architecture.md).

---

## Hardware Compatibility

Designed and compatible with modern Linux laptops featuring integrated biometrics, including:
* **Lenovo ThinkPad Series**
* **HP EliteBook / ProBook Series**
* **Dell Latitude / XPS Series**

Supported hardware:
* **Fingerprint Readers**: Validity / Synaptics sensors (`138a:XXXX`, `06cb:XXXX`, etc.) via `open-fprintd` and `python3-validity`, as well as standard libfprint devices.
* **Cameras**: Standard UVC IR + RGB webcams via `visaged`.

---

## Installation

### 1. Prerequisites
Ensure you have Rust, Cargo, and ffmpeg installed:
```bash
sudo dnf install -y cargo ffmpeg git
```

### 2. Biometric Drivers Setup
* If you have not enrolled your fingerprint or configured your IR camera yet, follow the [Hardware Setup Guide](docs/hardware-setup.md).

### 3. Clone & Install
```bash
git clone https://github.com/JulianoFantinatti/fedorakdeunlocksystem.git
cd fedorakdeunlocksystem
./install.sh
```

The installer will:
1. Compile the release Rust binary (`biometric-polkit-agent`).
2. Install system wrappers (`/usr/local/bin/w`, `/usr/local/bin/sudo`, `/usr/local/bin/check-biometric-mode`).
3. Deploy the isolated PAM profile to `/etc/pam.d/polkit-1`.
4. Configure the systemd user service override for `plasma-polkit-agent.service`.
5. Install the sleep/resume hook to keep the fingerprint reader active after laptop suspend.

---

## Usage

### Elevated Commands via Sudo
Just use `sudo` normally:
```bash
sudo dnf check-update
sudo ls
sudo -i
```
The biometric modal will appear on your screen on **Fingerprint** mode by default. Touch your sensor, press <kbd>→</kbd> for Face recognition, or type your password.

### Elevation Shortcut (`w`)
You can also use the `w` shortcut:
```bash
w dnf update
w ls
```

### Standalone UI Preview Mode
To test and inspect the graphical UI without needing root elevation:
```bash
~/.local/bin/biometric-polkit-agent --test
```

---

## Enrolling Biometrics

### Fingerprint Enrollment
```bash
# Enroll right index finger (default):
fprintd-enroll

# Enroll specific fingers:
fprintd-enroll -f right-thumb
fprintd-enroll -f right-middle-finger

# Verify:
fprintd-verify
```

### Face Recognition Enrollment
```bash
visage enroll $(whoami)

# Test verification:
visage verify $(whoami)
```

---

## Uninstallation

To restore the default KDE Plasma PolicyKit prompt and system configuration:
```bash
./uninstall.sh
```

---

## License

This project is licensed under the [MIT License](LICENSE).
