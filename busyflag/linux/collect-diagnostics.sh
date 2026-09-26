#!/bin/sh
# Collect what a Busyflag Linux test needs into one text file.
#
#   sh collect-diagnostics.sh          one snapshot
#   sh collect-diagnostics.sh 60       snapshot, then sample mic/camera state
#                                      every 2 s for 60 s while you test
#
# Run it as your normal user (not sudo), with the flag plugged in.
# It reads only: OS and desktop details, USB/HID devices, the udev rule,
# sound-server capture streams, which processes hold a camera open,
# and Busyflag's own log. It changes nothing.
#
# Your user name and host name are replaced with <user> and <host>, and your
# home folder with ~. Read the file before you share it anyway.
set -u

DURATION=${1:-0}
OUT="busyflag-diagnostics-$(date +%Y%m%d-%H%M%S).txt"
USER_NAME=$(id -un)
HOST_NAME=$(uname -n)

redact() {
  sed -e "s|$HOME|~|g" -e "s|$USER_NAME|<user>|g" -e "s|$HOST_NAME|<host>|g"
}

section() { printf '\n===== %s =====\n' "$1"; }

run() {
  printf '$ %s\n' "$*"
  if command -v "$1" >/dev/null 2>&1; then
    "$@" 2>&1 | head -200
  else
    echo "(not installed: $1)"
  fi
}

mic_and_camera() {
  if command -v pactl >/dev/null 2>&1; then
    echo "-- capture streams (pactl list source-outputs, names only)"
    pactl list source-outputs 2>/dev/null |
      grep -E 'Source Output #|application.name|application.process.binary|media.name|Corked' || echo "(none)"
  fi
  echo "-- ALSA capture devices in use"
  found=0
  for f in /proc/asound/card*/pcm*c/sub*/status; do
    [ -r "$f" ] || continue
    if grep -q 'RUNNING' "$f"; then echo "$f RUNNING"; found=1; fi
  done
  [ "$found" = 1 ] || echo "(none running)"
  echo "-- processes holding a camera open (this user only)"
  found=0
  for p in /proc/[0-9]*; do
    for fd in "$p"/fd/*; do
      t=$(readlink "$fd" 2>/dev/null) || continue
      case $t in /dev/video*)
        echo "$(cat "$p/comm" 2>/dev/null) (pid ${p#/proc/}) -> $t"; found=1; break ;;
      esac
    done
  done
  [ "$found" = 1 ] || echo "(none)"
}

{
  echo "Busyflag Linux diagnostics, $(date '+%Y-%m-%d %H:%M:%S %z')"

  section "System"
  cat /etc/os-release 2>/dev/null | grep -E '^(PRETTY_NAME|VERSION_ID)='
  uname -srm
  echo "Desktop: ${XDG_CURRENT_DESKTOP:-unknown}  Session: ${XDG_SESSION_TYPE:-unknown}"

  section "Busyflag package"
  if command -v dpkg >/dev/null 2>&1; then dpkg -l 2>/dev/null | grep -i busyflag || echo "(no busyflag deb installed)"; fi
  if command -v rpm >/dev/null 2>&1; then rpm -qa 2>/dev/null | grep -i busyflag; fi
  pgrep -a -f -i busyflag || echo "(Busyflag is not running)"

  section "Luxafor flag on USB (vendor 04d8, product f372)"
  run lsusb -d 04d8:f372
  echo "-- hidraw devices"
  for d in /sys/class/hidraw/hidraw*; do
    [ -e "$d" ] || continue
    name=$(basename "$d")
    if grep -qi '04D8.*F372' "$d/device/uevent" 2>/dev/null; then
      echo "/dev/$name is the flag"
      ls -l "/dev/$name"
      if command -v getfacl >/dev/null 2>&1; then getfacl -p "/dev/$name" 2>/dev/null | grep -E '^user:'; fi
      if [ -r "/dev/$name" ] && [ -w "/dev/$name" ]; then echo "this user CAN open it"; else echo "this user CANNOT open it (udev rule not applied? unplug and replug)"; fi
    fi
  done

  section "udev rule"
  for r in /usr/lib/udev/rules.d/70-busyflag.rules /etc/udev/rules.d/70-busyflag.rules; do
    [ -f "$r" ] && { echo "$r:"; grep -v '^#' "$r"; }
  done

  section "Sound server"
  run pactl info
  echo "-- sources (monitors are ignored by Busyflag)"
  pactl list short sources 2>/dev/null || echo "(pactl unavailable: Busyflag falls back to /proc/asound)"

  section "Screen lock"
  if command -v loginctl >/dev/null 2>&1; then
    loginctl show-session "${XDG_SESSION_ID:-self}" -p LockedHint -p Type -p Active 2>&1
  fi

  section "Mic and camera right now"
  mic_and_camera

  if [ "$DURATION" -gt 0 ] 2>/dev/null; then
    section "Sampling every 2 s for ${DURATION} s"
    end=$(( $(date +%s) + DURATION ))
    while [ "$(date +%s)" -lt "$end" ]; do
      echo "--- $(date +%H:%M:%S)"
      mic_and_camera
      sleep 2
    done
  fi

  section "Busyflag log (last 200 lines)"
  found=0
  for l in "${XDG_DATA_HOME:-$HOME/.local/share}/com.busyflag.desktop/logs/busyflag.log" \
           "${XDG_CONFIG_HOME:-$HOME/.config}/com.busyflag.desktop/logs/busyflag.log"; do
    if [ -f "$l" ]; then echo "$l"; tail -n 200 "$l"; found=1; fi
  done
  [ "$found" = 1 ] || echo "(no log file found; is Busyflag installed and has it run once?)"
} 2>&1 | redact > "$OUT"

echo "Wrote $OUT. Read it before sharing it."
