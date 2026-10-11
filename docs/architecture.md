# System Architecture & Security Specification

This document details the architectural design, security model, and component interactions of **Fedora KDE Unlock System**.

---

## 1. High-Level Architecture

```
                      +---------------------------------------+
                      |       User Desktop (Wayland/KDE)      |
                      +---------------------------------------+
                                          |
          [sudo <cmd>]                    |            [PolicyKit Action]
                |                         |                     |
                v                         |                     v
      +--------------------+              |           +-------------------+
      | /usr/local/bin/sudo|              |           | Polkit Authority  |
      +--------------------+              |           | (polkitd on D-Bus)|
                |                         |           +-------------------+
                v                         |                     |
      +--------------------+              |                     |
      |  /usr/local/bin/w  |--------------+                     |
      |   (exec pkexec)    |                                    |
      +--------------------+                                    |
                |                                               |
                +-----------------------------------------------+
                                          |
                                          v
                      +---------------------------------------+
                      |   biometric-polkit-agent (Rust/Slint) |
                      |   - Hardware Accelerated Wayland Modal|
                      |   - Fingerprint / Face / Password Tabs|
                      |   - Full-Square Live Camera Viewport  |
                      |   - Cryptographic Zeroize Buffer      |
                      +---------------------------------------+
                                    |           |
            Mode: Fingerprint       |           |   Mode: Face Recognition
                   v                |           v               v
            +--------------+        |    +-------------+   +---------+
            | open-fprintd |        |    | pam_visage  |   | ffmpeg  |
            |   (TLS D-Bus)|        |    |  (visaged)  |   | (v4l2)  |
            +--------------+        |    +-------------+   +---------+
                                    v
                      +---------------------------+
                      |   polkit-agent-helper-1   |
                      |   (Audited setuid helper) |
                      +---------------------------+
                                    |
                                    v
                      +---------------------------+
                      |     Target Command Execution     |
                      |     (Elevated root process)|
                      +---------------------------+
```

---

## 2. Core Components

### A. The Native Rust Agent (`biometric-polkit-agent`)
* **Framework**: Built with **Rust** and **Slint UI 1.9** for zero-overhead, GPU-accelerated Wayland rendering.
* **D-Bus Registration**: Registers as an official `org.freedesktop.PolicyKit1.AuthenticationAgent` on the **System D-Bus** for the active graphical session (`seat0`).
* **Memory Footprint**: Consumes only **~20MB RAM** (compared to 90MB+ for default Qt6 agents, and ~400MB for web/Electron-based wrappers).

### B. Security & Memory Hygiene
* **Zero Credential Caching**: The agent never stores passwords, hashes, or replay tokens on disk or in daemon state.
* **Immediate Zeroization**: Uses the Rust `zeroize` crate to securely scrub password byte vectors with cryptographic zeros immediately after transmitting them to the setuid helper.
* **Audited Setuid Boundary**: Instead of executing custom root scripts or granting passwordless sudo rights, authentication flows through Fedora's official, setuid-root `/usr/lib/polkit-1/polkit-agent-helper-1`.

### C. Dynamic PAM Mode Gating
By default, chained PAM modules execute sequentially regardless of the user interface state. To ensure that **Fingerprint mode only activates the fingerprint reader** (leaving the IR camera off) and **Face mode only activates the IR camera**, the system implements dynamic PAM gating:
1. When a tab is selected in the UI, the agent atomically writes the active mode (`fingerprint`, `face`, `password`) to a user-isolated tmpfs state file (`$XDG_RUNTIME_DIR/biometric_mode`, typically `/run/user/<UID>/biometric_mode` with `0700` permissions).
2. `/etc/pam.d/polkit-1` uses `pam_exec.so` and `check-biometric-mode`:
   * In **Fingerprint mode**: `check-biometric-mode face` fails -> PAM skips `pam_visage.so`. The IR camera is never powered on. `check-biometric-mode fingerprint` succeeds -> `pam_fprintd.so` waits for your finger.
   * In **Face mode**: `check-biometric-mode face` succeeds -> `pam_visage.so` activates the IR camera. `pam_fprintd.so` is skipped.
   * In **Password mode**: Both biometrics are skipped and PAM directly engages `pam_unix.so`.

### D. Full-Square Live Video Streaming
* In Face Recognition mode, the agent spawns a low-overhead capture stream (`ffmpeg -f v4l2 -video_size 320x240 -framerate 15 -i /dev/video0 -f rawvideo -pix_fmt rgb24 -`).
* Incoming frames are transferred directly into a `slint::SharedPixelBuffer<Rgb8Pixel>` and rendered onto the canvas in real-time.
* Corner target reticles and an animated scanning laser sweep across the viewport to provide clear visual feedback while `pam_visage` performs neural verification.
* When navigating away from the Face tab or closing the dialog, the video stream process is immediately terminated, ensuring zero camera battery drain and complete privacy.

### E. Working Directory Preservation (`w` & `sudo`)
* Standard `pkexec` changes directory to `/root` by default.
* The `/usr/local/bin/w` wrapper captures the caller's working directory (`$PWD`) and executes `cd "$ORIG_PWD"` inside the elevated subshell.
* Commands like `w ls` or `sudo ls` execute in the caller's local directory, while `w ls /root` explicitly inspects `/root`.
