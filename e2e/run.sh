#!/usr/bin/env bash
# Full end-to-end test of Mini MQTT Client on a real X server.
#
# Starts: a private mosquitto broker (TCP / WebSocket / TLS), Xvfb, a private
# D-Bus session with the AT-SPI accessibility bus, then runs e2e/test_app.py
# against the real application binary.
#
# Environment:
#   E2E_BIN       application binary (default: target/debug/mini-mqtt-client, built if missing)
#   E2E_WORK_DIR  working directory for data, logs, screenshots, report (default: target/e2e)
#   E2E_DISPLAY   X display to use (default: :97)
#   E2E_ONLY      comma separated test ids to run (e.g. T01,T05)
#   E2E_PYTHON    python interpreter with pyatspi + PyYAML
#   E2E_SCRIPT    run another python script in the prepared environment (debugging)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export E2E_ROOT="$ROOT"
export E2E_WORK="${E2E_WORK_DIR:-$ROOT/target/e2e}"
export E2E_BIN="${E2E_BIN:-$ROOT/target/debug/mini-mqtt-client}"
export E2E_DISPLAY="${E2E_DISPLAY:-:97}"
export E2E_TCP_PORT="${E2E_TCP_PORT:-18830}"
export E2E_WS_PORT="${E2E_WS_PORT:-18083}"
export E2E_TLS_PORT="${E2E_TLS_PORT:-18883}"
# The scenarios find controls by their (Chinese) accessible names: pin the
# interface language instead of following the machine's locale (often C or
# en_US on CI). No update check either: the tests must not depend on the
# network or on what GitHub currently lists as the latest release.
export MINI_MQTT_LANG=zh-CN
export MINI_MQTT_NO_UPDATE_CHECK=1

missing=()
for tool in Xvfb mosquitto mosquitto_pub mosquitto_sub xdotool xclip xwd convert openssl dbus-run-session; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done
if ((${#missing[@]})); then
  echo "missing tools: ${missing[*]}" >&2
  echo "Debian/Ubuntu: apt-get install xvfb mosquitto mosquitto-clients xdotool xclip x11-apps imagemagick openssl dbus at-spi2-core python3-pyatspi python3-yaml mesa-vulkan-drivers fonts-noto-cjk" >&2
  exit 2
fi

PY="${E2E_PYTHON:-}"
if [[ -z "$PY" ]]; then
  for candidate in python3 python3.12 python3.13 python3.11; do
    if command -v "$candidate" >/dev/null && "$candidate" -c "import pyatspi, yaml" 2>/dev/null; then
      PY="$candidate"
      break
    fi
  done
fi
[[ -n "$PY" ]] || { echo "no python with pyatspi + PyYAML found (set E2E_PYTHON)" >&2; exit 2; }
export E2E_PYTHON="$PY"

if [[ ! -x "$E2E_BIN" ]]; then
  (cd "$ROOT" && cargo build)
fi

rm -rf "$E2E_WORK"
mkdir -p "$E2E_WORK"/{tls,shots,data}

# --- TLS material: a throwaway CA and a server certificate for localhost ----
(
  cd "$E2E_WORK/tls"
  openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.crt -days 2 -subj "/CN=E2E Test CA" 2>/dev/null
  openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr -subj "/CN=localhost" 2>/dev/null
  printf "subjectAltName=DNS:localhost,IP:127.0.0.1\n" >ext.cnf
  openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 2 -extfile ext.cnf 2>/dev/null
  chmod 644 ./*
)
export E2E_CA="$E2E_WORK/tls/ca.crt"

# --- Broker ------------------------------------------------------------------
{
  [[ "$(id -u)" == 0 ]] && echo "user root"
  cat <<EOF
per_listener_settings false
allow_anonymous true
listener $E2E_TCP_PORT 127.0.0.1
protocol mqtt
listener $E2E_WS_PORT 127.0.0.1
protocol websockets
listener $E2E_TLS_PORT 127.0.0.1
protocol mqtt
cafile $E2E_WORK/tls/ca.crt
certfile $E2E_WORK/tls/server.crt
keyfile $E2E_WORK/tls/server.key
EOF
} >"$E2E_WORK/mosquitto.conf"

pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
}
trap cleanup EXIT

mosquitto -c "$E2E_WORK/mosquitto.conf" -v >"$E2E_WORK/broker.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do
  mosquitto_pub -h 127.0.0.1 -p "$E2E_TCP_PORT" -t e2e/ready -n 2>/dev/null && break
  sleep 0.1
done

# --- Display -----------------------------------------------------------------
Xvfb "$E2E_DISPLAY" -screen 0 1400x900x24 -nolisten tcp >"$E2E_WORK/xvfb.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do
  DISPLAY="$E2E_DISPLAY" xdotool getmouselocation >/dev/null 2>&1 && break
  sleep 0.1
done

# --- Accessibility session + tests --------------------------------------------
set +e
dbus-run-session -- bash -c '
  set -e
  for launcher in /usr/libexec/at-spi-bus-launcher /usr/lib/at-spi2-core/at-spi-bus-launcher /usr/lib/at-spi2/at-spi-bus-launcher; do
    [[ -x "$launcher" ]] && { "$launcher" --launch-immediately >/dev/null 2>&1 & break; }
  done
  for _ in $(seq 50); do
    dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
      org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:IsEnabled variant:boolean:true \
      >/dev/null 2>&1 && break
    sleep 0.1
  done
  exec "$E2E_PYTHON" "${E2E_SCRIPT:-$E2E_ROOT/e2e/test_app.py}"
' 2> >(grep -v -e "Failed to set fd limit" -e "dbind-WARNING" -e '^$' >&2)
status=$?
set -e

echo "report: $E2E_WORK/report.md"
exit $status
