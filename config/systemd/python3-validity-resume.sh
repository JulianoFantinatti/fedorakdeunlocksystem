#!/bin/bash
# Automatically restart fingerprint services on resume from sleep/suspend
# to prevent the "Unexpected TLS version 4 0" hardware state reset error on Validity sensors.
case "$1/$2" in
    post/*)
        systemctl restart python3-validity.service open-fprintd.service 2>/dev/null || true
        ;;
esac
