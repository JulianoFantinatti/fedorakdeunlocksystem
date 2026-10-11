# Hardware Setup Guide: Biometrics on Fedora KDE

This guide walks through configuring **Validity/Synaptics Fingerprint Readers** and **Infrared (IR) Facial Recognition Cameras** on Fedora Linux across supported laptop platforms.

---

## Part 1: Validity / Synaptics Fingerprint Sensors (e.g. `138a:XXXX`, `06cb:XXXX`)

Many business laptops (HP, Lenovo ThinkPad, Dell Latitude, etc.) use Validity/Synaptics sensors that are not supported out of the box by upstream `libfprint`.

### 1. Identify Your Sensor
Run `lsusb`:
```bash
lsusb | grep -iE "validity|synaptics"
# Example output: Bus 001 Device 004: ID 138a:XXXX Validity Sensors, Inc.
```

### 2. Install open-fprintd & python-validity
Fedora provides or supports `open-fprintd` which uses `python3-validity` to interface with the sensor over TLS:
```bash
sudo dnf copr enable -y uunicorn/open-fprintd
sudo dnf install -y open-fprintd python3-validity fprintd-pam
```

### 3. Deploy Firmware
The sensor requires proprietary firmware to boot its micro-controller.
1. Obtain the appropriate firmware file for your sensor model (`<firmware_name>.xpfwext`) from the vendor driver package or `python3-validity` repository.
2. Deploy the file to:
   ```bash
   sudo mkdir -p /var/lib/python-validity/firmware
   sudo cp <firmware_name>.xpfwext /var/lib/python-validity/firmware/
   sudo chmod 644 /var/lib/python-validity/firmware/<firmware_name>.xpfwext
   ```

### 4. Enable Services
```bash
sudo systemctl enable --now open-fprintd.service python3-validity.service
```

### 5. Enroll Fingerprints
```bash
# Enroll right index finger (default):
fprintd-enroll

# Enroll specific fingers:
fprintd-enroll -f right-thumb
fprintd-enroll -f right-middle-finger
```
Touch the sensor square repeatedly until `Enroll result: enroll-completed` is shown.

### 6. Verify Sensor
```bash
fprintd-verify
```

---

## Part 2: IR Camera & Face Recognition (Visage)

Fedora 40–44 ships with Python 3.14+, rendering unmaintained Howdy packages obsolete. This project uses **Visage**, a modern, pure-Rust daemon with ONNX neural models (SCRFD detector + ArcFace recognition).

### 1. Identify Your IR Camera
```bash
ls -l /sys/class/video4linux/video*/name
# Look for your vendor's IR device (e.g. "IR Camera", often mapped to /dev/video2)
```

### 2. Visage Architecture
* Daemon: `/usr/bin/visaged` running as `visaged.service`.
* D-Bus Interface: `org.freedesktop.Visage1` on the system bus.
* PAM Module: `/usr/lib64/security/pam_visage.so`.
* Neural Models: Located in `/usr/share/visage/models/` or `/var/lib/visage/models/`.

### 3. Enroll Face
```bash
visage enroll $(whoami)
```
Look directly into the IR camera lens during enrollment.

### 4. Verify Face Recognition
```bash
visage verify $(whoami)
```
The IR emitters will illuminate and output `Face matched!`.
