#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

detect_lan_ip() {
  python3 - <<'PY'
import ipaddress
import re
import socket
import subprocess

# Ranges used by tunnels/VPNs that should not be advertised as the LAN address.
EXCLUDED = ["100.64.0.0/10", "198.18.0.0/15"]

def is_lan(ip):
    try:
        addr = ipaddress.ip_address(ip)
    except ValueError:
        return False
    return (
        addr.is_private
        and not addr.is_loopback
        and not addr.is_link_local
        and not addr.is_multicast
        and not any(addr in ipaddress.ip_network(net) for net in EXCLUDED)
    )

def ifconfig_ips():
    try:
        out = subprocess.run(["ifconfig"], capture_output=True, text=True).stdout
    except OSError:
        return []
    return [
        match.group(1)
        for line in out.splitlines()
        if (match := re.search(r"inet (\d+\.\d+\.\d+\.\d+)", line))
    ]

# Source IP of the default route (no packet is sent).
source = ""
try:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.connect(("8.8.8.8", 80))
        source = sock.getsockname()[0] or ""
    finally:
        sock.close()
except OSError:
    pass

candidates = [ip for ip in ifconfig_ips() if is_lan(ip)]
if is_lan(source):
    print(source)
elif candidates:
    print(candidates[0])
elif source and not source.startswith("127.") and not source.startswith("0."):
    print(source)
else:
    print("127.0.0.1")
PY
}

# Primary LAN address so admin + web are reachable from other hosts; falls back
# to loopback when the host has no routable interface.
LAN_IP="$(detect_lan_ip)"
NOPROXY_HOSTS="127.0.0.1,localhost,${LAN_IP}"

ADMIN_BASE_URL="${ADMIN_BASE_URL:-https://${LAN_IP}:3000}"
ADMIN_API_TOKEN="${ADMIN_API_TOKEN:-}"
if [[ -z "${ADMIN_API_TOKEN}" ]]; then
  # 10 alphanumeric chars (~60 bits of entropy): strong enough for the
  # rate-limited admin control plane while still being easy to type. The
  # 16-byte base64 source guarantees plenty of alphanumerics survive tr so head
  # always yields exactly 10 chars; `|| true` swallows tr's SIGPIPE once head
  # closes the pipe (otherwise pipefail turns it into a fatal 141 exit).
  ADMIN_API_TOKEN="$(openssl rand -base64 16 | LC_ALL=C tr -dc 'A-Za-z0-9' | head -c 10 || true)"
fi
ARCH="${ARCH:-x86}"
# Keep the test workspace inside the repo under .run/test/ so it is easy to find;
# fall back to the system temp dir if the repo dir cannot be created.
TEST_RUN_DIR="${REPO_ROOT}/.run/test"
if ! mkdir -p "${TEST_RUN_DIR}" 2>/dev/null; then
  TEST_RUN_DIR="${TMPDIR:-/tmp}"
fi
TMP_ROOT="$(mktemp -d "${TEST_RUN_DIR}/warp-insight-install-enroll.XXXXXX")"
INSTALL_SCRIPT="${TMP_ROOT}/install.sh"
INSTALL_SIGNATURE="${TMP_ROOT}/install.sh.sig"
INSTALL_PUBLIC_KEY="${TMP_ROOT}/install.pub.pem"
RESPONSE_JSON="${TMP_ROOT}/install-code.json"
INSTALL_HOME="${WARP_INSIGHT_HOME:-${TMP_ROOT}/agent}"
TEST_AGENT_INSTANCE="install-test-$(basename "${TMP_ROOT}" | tr -c '[:alnum:]-' '-')"
ADMIN_CONFIG="${TMP_ROOT}/warp-insight-admin.toml"
INSTALL_SIGNING_PRIVATE_KEY="${TMP_ROOT}/install-signing-ed25519.pkcs8.pem"
ADMIN_TLS_CA_CERT="${TMP_ROOT}/admin-tls-ca.crt.pem"
ADMIN_TLS_CA_KEY="${TMP_ROOT}/admin-tls-ca.key.pem"
ADMIN_TLS_CERT="${TMP_ROOT}/admin-tls.crt.pem"
ADMIN_TLS_KEY="${TMP_ROOT}/admin-tls.key.pem"
ADMIN_TLS_CSR="${TMP_ROOT}/admin-tls.csr.pem"
ADMIN_TLS_EXT="${TMP_ROOT}/admin-tls.ext"
ADMIN_LOG="${TMP_ROOT}/warp-insight-admin.log"
ADMIN_PID=""
ADMIN_WEB_BASE_URL="${ADMIN_WEB_BASE_URL:-http://${LAN_IP}:5173}"
ADMIN_WEB_LOG="${TMP_ROOT}/warp-insight-admin-web.log"
ADMIN_WEB_PID=""
SKIP_ADMIN_WEB="${SKIP_ADMIN_WEB:-0}"
STOP_STARTED_SERVICES="${STOP_STARTED_SERVICES:-0}"
WAIT_FOR_STARTED_SERVICES="${WAIT_FOR_STARTED_SERVICES:-1}"
STATUS_REPORT_WAIT_TIMEOUT_SECONDS="${STATUS_REPORT_WAIT_TIMEOUT_SECONDS:-90}"
WAIT_FOR_EXIT="${WAIT_FOR_EXIT:-1}"
DAEMON_PID=""
AGENT_STATUS_LOG="${TMP_ROOT}/agent-status.log"
OVERVIEW_JSON="${TMP_ROOT}/agent-overview.json"

cleanup() {
  if [[ -n "${DAEMON_PID}" ]] && kill -0 "${DAEMON_PID}" 2>/dev/null; then
    kill "${DAEMON_PID}" 2>/dev/null || true
    wait "${DAEMON_PID}" 2>/dev/null || true
  fi
  DAEMON_PID=""
  if [[ "${STOP_STARTED_SERVICES}" == "1" ]]; then
    if [[ -n "${ADMIN_WEB_PID}" ]] && kill -0 "${ADMIN_WEB_PID}" 2>/dev/null; then
      kill "${ADMIN_WEB_PID}" 2>/dev/null || true
      wait "${ADMIN_WEB_PID}" 2>/dev/null || true
    fi
    ADMIN_WEB_PID=""
    if [[ -n "${ADMIN_PID}" ]] && kill -0 "${ADMIN_PID}" 2>/dev/null; then
      kill "${ADMIN_PID}" 2>/dev/null || true
      wait "${ADMIN_PID}" 2>/dev/null || true
    fi
    ADMIN_PID=""
  fi
  if [[ "${KEEP_WARP_INSIGHT_ENROLL_TEST:-0}" != "1" && -z "${ADMIN_PID}${ADMIN_WEB_PID}" ]]; then
    rm -rf "${TMP_ROOT}"
  else
    echo "kept test workspace: ${TMP_ROOT}"
  fi
}
trap cleanup EXIT

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

curl_get() {
  local url="$1"
  curl -fsSL \
    --noproxy "${NOPROXY_HOSTS}" \
    "${url}"
}

install_code_status() {
  curl -s \
    --noproxy "${NOPROXY_HOSTS}" \
    -H "authorization: Bearer ${ADMIN_API_TOKEN}" \
    -o "${RESPONSE_JSON}" \
    -w "%{http_code}" \
    "${ADMIN_BASE_URL%/}/api/v1/agent/install-code" || true
}

admin_listen_addr() {
  python3 - "$ADMIN_BASE_URL" <<'PY'
import sys
from urllib.parse import urlparse

url = urlparse(sys.argv[1])
if url.scheme != "https":
    raise SystemExit(f"ADMIN_BASE_URL must start with https://: {sys.argv[1]}")
if not url.hostname:
    raise SystemExit(f"ADMIN_BASE_URL is missing hostname: {sys.argv[1]}")
port = url.port
if port is None:
    port = 443
host = url.hostname
# Loopback stays loopback; any other advertised host binds all interfaces so
# the admin is reachable from the network at the advertised URL host.
if host in {"127.0.0.1", "localhost"}:
    print(f"{host}:{port}")
else:
    print(f"0.0.0.0:{port}")
PY
}

# subjectAltName line for the admin TLS cert: always cover loopback, the primary
# LAN address, and any non-loopback IP explicitly advertised in ADMIN_BASE_URL.
admin_cert_san() {
  python3 - "$LAN_IP" "$ADMIN_BASE_URL" <<'PY'
import ipaddress
import sys
from urllib.parse import urlparse

lan_ip = sys.argv[1]
url = urlparse(sys.argv[2])
ips = {"127.0.0.1", lan_ip}
host = url.hostname
if host and host not in {"127.0.0.1", "localhost"} and host != lan_ip:
    try:
        ipaddress.ip_address(host)
        ips.add(host)
    except ValueError:
        pass  # hostname, not a literal IP — cannot go into an IP: SAN entry
entries = ",".join(f"IP:{ip}" for ip in sorted(ips))
print(f"subjectAltName=DNS:localhost,{entries}")
PY
}

require_https_admin_base_url() {
  if [[ "${ADMIN_BASE_URL%/}" != https://* ]]; then
    echo "ADMIN_BASE_URL must start with https:// for install/enrollment tests: ${ADMIN_BASE_URL}" >&2
    exit 1
  fi
}

admin_web_listen_options() {
  python3 - "$ADMIN_WEB_BASE_URL" <<'PY'
import sys
from urllib.parse import urlparse

url = urlparse(sys.argv[1])
if url.scheme not in {"http", "https"}:
    raise SystemExit(f"ADMIN_WEB_BASE_URL must start with http:// or https://: {sys.argv[1]}")
if not url.hostname:
    raise SystemExit(f"ADMIN_WEB_BASE_URL is missing hostname: {sys.argv[1]}")
port = url.port
if port is None:
    port = 443 if url.scheme == "https" else 80
host = url.hostname
# Loopback stays loopback; any other advertised host binds all interfaces so
# the web UI is reachable from the network at the advertised URL host.
if host in {"127.0.0.1", "localhost"}:
    print(host)
else:
    print("0.0.0.0")
print(port)
PY
}

start_admin_service() {
  local listen_addr
  listen_addr="$(admin_listen_addr)"

  require_cmd cargo
  require_cmd openssl
  echo "admin service is not running; building and starting warp-insight-admin..."
  cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" -p warp-insightd -p warp-insight-admin
  openssl genpkey -algorithm ED25519 -out "${INSTALL_SIGNING_PRIVATE_KEY}" >/dev/null 2>&1
  openssl req -x509 -newkey rsa:2048 -nodes \
    -keyout "${ADMIN_TLS_CA_KEY}" \
    -out "${ADMIN_TLS_CA_CERT}" \
    -days 1 \
    -subj "/CN=warp-insight-install-test-ca" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign" \
    >/dev/null 2>&1
  openssl req -newkey rsa:2048 -nodes \
    -keyout "${ADMIN_TLS_KEY}" \
    -out "${ADMIN_TLS_CSR}" \
    -subj "/CN=127.0.0.1" \
    >/dev/null 2>&1
  cat >"${ADMIN_TLS_EXT}" <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
$(admin_cert_san)
EOF
  openssl x509 -req \
    -in "${ADMIN_TLS_CSR}" \
    -CA "${ADMIN_TLS_CA_CERT}" \
    -CAkey "${ADMIN_TLS_CA_KEY}" \
    -CAcreateserial \
    -out "${ADMIN_TLS_CERT}" \
    -days 1 \
    -sha256 \
    -extfile "${ADMIN_TLS_EXT}" \
    >/dev/null 2>&1
  chmod 0600 "${ADMIN_TLS_CA_KEY}" "${ADMIN_TLS_KEY}"
  export CURL_CA_BUNDLE="${ADMIN_TLS_CA_CERT}"

  cat >"${ADMIN_CONFIG}" <<EOF
[server]
listen_addr = "${listen_addr}"
public_base_url = "${ADMIN_BASE_URL%/}"
tls_cert_file = "${ADMIN_TLS_CERT}"
tls_key_file = "${ADMIN_TLS_KEY}"
admin_api_token = "${ADMIN_API_TOKEN}"

[agent]
package_file = "${REPO_ROOT}/target/debug/warp-insightd"
bootstrap_token_ttl_seconds = 900
credential_ttl_seconds = 2592000
store_file = "${TMP_ROOT}/admin-state/admin-store.json"
trust_bundle = '''$(cat "${ADMIN_TLS_CA_CERT}")
'''
install_script_signing_private_key_file = "${INSTALL_SIGNING_PRIVATE_KEY}"
tenant_id = "tenant-install-test"
environment_id = "env-install-test"
EOF
  chmod 0600 "${ADMIN_CONFIG}"

  (
    cd "${REPO_ROOT}"
    exec nohup env WARP_INSIGHT_ADMIN_CONFIG="${ADMIN_CONFIG}" \
      "${REPO_ROOT}/target/debug/warp-insight-admin" \
      >"${ADMIN_LOG}" 2>&1
  ) &
  ADMIN_PID=$!

  for _ in {1..100}; do
    if ! kill -0 "${ADMIN_PID}" 2>/dev/null; then
      echo "warp-insight-admin failed to start; log:" >&2
      cat "${ADMIN_LOG}" >&2
      exit 1
    fi
    if [[ "$(install_code_status)" == "200" ]]; then
      echo "started warp-insight-admin pid=${ADMIN_PID}"
      return
    fi
    sleep 0.1
  done

  echo "warp-insight-admin did not become ready; log:" >&2
  cat "${ADMIN_LOG}" >&2
  exit 1
}

ensure_install_code_endpoint() {
  local status

  status="$(install_code_status)"

  if [[ "${status}" == "000" ]]; then
    start_admin_service
    status="$(install_code_status)"
  fi
  if [[ "${status}" != "200" ]]; then
    if [[ "${status}" == "401" ]]; then
      echo "another warp-insight-admin appears to be running at ${ADMIN_BASE_URL%/} with a different admin API token." >&2
      echo "stop it, or set ADMIN_API_TOKEN to match the running admin." >&2
    else
      echo "admin install-code endpoint check failed: ${ADMIN_BASE_URL%/}/api/v1/agent/install-code returned ${status}." >&2
    fi
    exit 1
  fi
}

admin_web_status() {
  curl -s \
    --noproxy "${NOPROXY_HOSTS}" \
    -o /dev/null \
    -w "%{http_code}" \
    "${ADMIN_WEB_BASE_URL%/}/" || true
}

start_admin_web_service() {
  local host
  local port

  host="$(admin_web_listen_options | sed -n '1p')"
  port="$(admin_web_listen_options | sed -n '2p')"

  require_cmd npm
  if [[ ! -d "${REPO_ROOT}/crates/warp-insight-admin-web/node_modules" ]]; then
    echo "admin-web dependencies are missing: crates/warp-insight-admin-web/node_modules" >&2
    echo "run npm install in crates/warp-insight-admin-web before running this script." >&2
    exit 1
  fi

  echo "admin-web is not running; starting warp-insight-admin-web..."
  (
    cd "${REPO_ROOT}/crates/warp-insight-admin-web"
    exec nohup npm run dev -- --host "${host}" --port "${port}" --strictPort \
      >"${ADMIN_WEB_LOG}" 2>&1
  ) &
  ADMIN_WEB_PID=$!

  for _ in {1..100}; do
    if ! kill -0 "${ADMIN_WEB_PID}" 2>/dev/null; then
      echo "warp-insight-admin-web failed to start; log:" >&2
      cat "${ADMIN_WEB_LOG}" >&2
      exit 1
    fi
    if [[ "$(admin_web_status)" == "200" ]]; then
      echo "started warp-insight-admin-web pid=${ADMIN_WEB_PID}"
      echo "admin web url: ${ADMIN_WEB_BASE_URL}"
      return
    fi
    sleep 0.1
  done

  echo "warp-insight-admin-web did not become ready; log:" >&2
  cat "${ADMIN_WEB_LOG}" >&2
  exit 1
}

ensure_admin_web_service() {
  local status
  status="$(admin_web_status)"

  if [[ "${status}" == "000" ]]; then
    start_admin_web_service
    status="$(admin_web_status)"
  fi
  if [[ "${status}" != "200" ]]; then
    echo "admin-web endpoint check failed: ${ADMIN_WEB_BASE_URL%/}/ returned ${status}." >&2
    exit 1
  fi
}

extract_install_field() {
  python3 - "$RESPONSE_JSON" "$ARCH" "$ADMIN_BASE_URL" "$1" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
arch = sys.argv[2]
base_url = sys.argv[3].rstrip("/")
field_name = sys.argv[4]
payload = json.loads(path.read_text(encoding="utf-8"))
install_code = payload["install_code"]
bundle = install_code["bootstrap_bundle"]

if bundle["control_endpoint"].rstrip("/") != base_url:
    raise SystemExit(
        f"control_endpoint mismatch: {bundle['control_endpoint']} != {base_url}"
    )

field = "arm_linux_install_code" if arch.startswith("arm") else "x86_linux_install_code"
command = install_code[field]
install_url = bundle.get("install_script_url", "")
if not install_url:
    raise SystemExit("bootstrap_bundle.install_script_url is required")
signature_url = f"{install_url}.sig"
required_command_fragments = [
    f'curl -fsSLk "{install_url}" -o "$D/s"',
    f'curl -fsSLk "{signature_url}" -o "$D/sig"',
    'openssl pkeyutl -verify -pubin -inkey "$D/key.pem" -rawin -in "$D/s" -sigfile "$D/sig"',
    'sh "$D/s"',
]
missing = [fragment for fragment in required_command_fragments if fragment not in command]
if missing:
    raise SystemExit(f"install command is missing signature verification fragments: {missing}\n{command}")
if "| sh" in command:
    raise SystemExit(f"install command must not stream directly to sh: {command}")
try:
    public_key = command.split("<<'EOF'\n", 1)[1].split("\nEOF", 1)[0] + "\n"
except IndexError as exc:
    raise SystemExit(f"install command must embed a public key PEM: {command}") from exc
if not public_key.startswith("-----BEGIN PUBLIC KEY-----\n"):
    raise SystemExit(f"install command public key is not PEM: {public_key!r}")
token = install_code.get("bootstrap_enrollment_token") or install_code.get("bootstrapEnrollmentToken")
if not isinstance(token, str) or not token:
    raise SystemExit("install_code.bootstrap_enrollment_token is required")
if token in command or "WARP_INSIGHT_ENROLLMENT_TOKEN=" in command:
    raise SystemExit(f"install command must not contain bootstrap token: {command}")
if "?token=" in install_url or "?token=" in bundle.get("install_script_url", ""):
    raise SystemExit(f"install URL must not contain token query: {install_url}")
if "?token=" in bundle.get("agent_package_url", ""):
    raise SystemExit(f"package URL must not contain token query: {bundle.get('agent_package_url')}")
if bundle.get("agent_package_url") != f"{base_url}/api/v1/agent/packages/current":
    raise SystemExit(f"unexpected package URL: {bundle.get('agent_package_url')}")
if field_name == "url":
    print(install_url)
elif field_name == "token":
    print(token)
elif field_name == "public_key":
    print(public_key, end="")
else:
    raise SystemExit(f"unknown field: {field_name}")
PY
}

extract_install_url() {
  extract_install_field url
}

extract_install_token() {
  extract_install_field token
}

extract_install_public_key() {
  extract_install_field public_key
}

validate_initial_config() {
  local config_path="$1"
  python3 - "$config_path" "$ADMIN_BASE_URL" <<'PY'
import pathlib
import sys

config_path = pathlib.Path(sys.argv[1])
base_url = sys.argv[2].rstrip("/")
text = config_path.read_text(encoding="utf-8")

required = [
    'schema_version = "v1"',
    f'endpoint = "{base_url}"',
    'enabled = true',
    'auth_mode = "enrollment_token"',
    'credential_request = "bearer"',
]
missing = [item for item in required if item not in text]
if missing:
    raise SystemExit(f"initial config missing entries: {missing}")
PY
}

set_test_agent_instance_name() {
  local config_path="$1"
  python3 - "$config_path" "$TEST_AGENT_INSTANCE" <<'PY'
import pathlib
import sys

config_path = pathlib.Path(sys.argv[1])
instance_name = sys.argv[2]
lines = config_path.read_text(encoding="utf-8").splitlines()
output = []
inserted = False
inside_agent = False
for line in lines:
    if line.strip() == "[agent]":
        inside_agent = True
        output.append(line)
        continue
    if inside_agent and not inserted and line.startswith("[") and line.strip().endswith("]"):
        output.append(f'instance_name = "{instance_name}"')
        inserted = True
        inside_agent = False
    if inside_agent and line.strip().startswith("instance_name"):
        output.append(f'instance_name = "{instance_name}"')
        inserted = True
        continue
    output.append(line)
if inside_agent and not inserted:
    output.append(f'instance_name = "{instance_name}"')
elif not inserted:
    output.extend(["", "[agent]", f'instance_name = "{instance_name}"'])
config_path.write_text("\n".join(output) + "\n", encoding="utf-8")
PY
}

check_enrollment_route() {
  local response_path="${TMP_ROOT}/enrollment-route-check.json"
  local status_path="${TMP_ROOT}/enrollment-route-check.status"
  local status

  status="$(
    curl -sS \
      --noproxy "${NOPROXY_HOSTS}" \
      -o "${response_path}" \
      -w "%{http_code}" \
      -X POST "${ADMIN_BASE_URL%/}/api/v1/agent/enroll" \
      -H "content-type: application/json" \
      -d '{"api_version":"v1","kind":"submit_enrollment_request","token":"__route_check_invalid_token__","credential_request":"none","host_profile":{"node_id":"route-check","hostname":"route-check","os":"test","arch":"test","machine_id":"route-check","cloud_instance_id":null,"k8s_node_uid":null,"ip_addresses":[]},"capability_summary":"route-check","requested_at":"2026-07-28T00:00:00Z"}'
  )"
  printf '%s\n' "${status}" >"${status_path}"

  if [[ "${status}" == "404" ]]; then
    cat >&2 <<EOF
admin enrollment route check failed: ${ADMIN_BASE_URL%/}/api/v1/agent/enroll returned 404.
EOF
    exit 1
  fi

  python3 - "${status_path}" "${response_path}" <<'PY'
import json
import pathlib
import sys

status = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").strip()
response_path = pathlib.Path(sys.argv[2])

if status != "201":
    body = response_path.read_text(encoding="utf-8", errors="replace")
    raise SystemExit(f"unexpected enrollment route status {status}: {body}")

payload = json.loads(response_path.read_text(encoding="utf-8"))
result = payload.get("result") or {}
if result.get("status") not in {"accepted", "rejected", "pending_review"}:
    raise SystemExit(f"unexpected enrollment route response: {payload}")
PY
}

validate_runtime_state() {
  local state_path="$1"
  python3 - "$state_path" <<'PY'
import json
import os
import pathlib
import sys

state_path = pathlib.Path(sys.argv[1])
runtime = json.loads(state_path.read_text(encoding="utf-8"))

agent_id = runtime.get("agent_id", "")
instance_id = runtime.get("instance_id", "")
if not agent_id or not instance_id:
    raise SystemExit(f"runtime identity is empty: {runtime}")
if agent_id == "agent-unknown" or instance_id == "unknown":
    raise SystemExit(f"runtime identity uses placeholder values: {runtime}")
if runtime.get("mode") != "normal":
    raise SystemExit(f"unexpected runtime mode: {runtime}")
if not runtime.get("credential_id") or not runtime.get("bearer_token"):
    raise SystemExit(f"runtime credential was not persisted: {runtime}")
if not runtime.get("credential_expires_at"):
    raise SystemExit(f"runtime credential expiration was not persisted: {runtime}")
if os.name == "posix":
    file_mode = state_path.stat().st_mode & 0o777
    dir_mode = state_path.parent.stat().st_mode & 0o777
    if file_mode != 0o600:
        raise SystemExit(f"runtime state file permissions must be 0600, got {oct(file_mode)}")
    if dir_mode != 0o700:
        raise SystemExit(f"runtime state directory permissions must be 0700, got {oct(dir_mode)}")

print("runtime state:")
display = dict(runtime)
if display.get("bearer_token"):
    display["bearer_token"] = "***"
print(json.dumps(display, indent=2, sort_keys=True))
PY
}

check_agent_credential_routes() {
  local state_path="$1"
  local agent_id
  local instance_id
  local bearer_token
  local payload_dir="${TMP_ROOT}/agent-api-payloads"
  local status

  mkdir -p "${payload_dir}"
  agent_id="$(python3 - "$state_path" <<'PY'
import json, pathlib, sys
print(json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))["agent_id"])
PY
)"
  instance_id="$(python3 - "$state_path" <<'PY'
import json, pathlib, sys
print(json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))["instance_id"])
PY
)"
  bearer_token="$(python3 - "$state_path" <<'PY'
import json, pathlib, sys
print(json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))["bearer_token"])
PY
)"

  python3 - "${payload_dir}" "${agent_id}" "${instance_id}" <<'PY'
import json
import pathlib
import sys

payload_dir = pathlib.Path(sys.argv[1])
agent_id = sys.argv[2]
instance_id = sys.argv[3]

(payload_dir / "status.json").write_text(json.dumps({
    "agent_id": agent_id,
    "instance_id": instance_id,
    "version": "0.1.0",
}), encoding="utf-8")

(payload_dir / "poll.json").write_text(json.dumps({
    "agent_id": agent_id,
    "instance_id": instance_id,
    "last_seen_sequence": 0,
    "wait_ms": 0,
    "requested_at": "2026-07-29T00:00:00Z",
}), encoding="utf-8")

(payload_dir / "result.json").write_text(json.dumps({
    "api_version": "v1",
    "report_id": "install-enrollment-report",
    "execution_id": "install-enrollment-exec",
    "dispatch_id": "install-enrollment-dispatch",
    "action_id": "install-enrollment-action",
    "agent_id": agent_id,
    "instance_id": instance_id,
    "kind": "command",
    "final_status": "succeeded",
    "result": "{}",
    "plan_digest": "sha256:test-plan",
    "report_attempt": 1,
    "reported_at": "2026-07-29T00:00:00Z",
    "result_attestation": {
        "issued_by": agent_id,
        "attested_at": "2026-07-29T00:00:00Z",
        "result_digest": "sha256:test-result",
        "signature": "test-signature",
    },
}), encoding="utf-8")

(payload_dir / "renew.json").write_text(json.dumps({
    "api_version": "v1",
    "kind": "renew_agent_credential",
    "agent_id": agent_id,
    "instance_id": instance_id,
    "credential_request": "bearer",
    "requested_at": "2026-07-29T00:00:00Z",
}), encoding="utf-8")
PY

  status="$(curl -sS --noproxy "${NOPROXY_HOSTS}" -o /dev/null -w "%{http_code}" \
    -X POST "${ADMIN_BASE_URL%/}/api/v1/agent/status" \
    -H "authorization: Bearer ${bearer_token}" \
    -H "content-type: application/json" \
    --data-binary "@${payload_dir}/status.json")"
  if [[ "${status}" != "202" ]]; then
    echo "agent status route returned ${status}" >&2
    exit 1
  fi

  status="$(curl -sS --noproxy "${NOPROXY_HOSTS}" -o /dev/null -w "%{http_code}" \
    -X POST "${ADMIN_BASE_URL%/}/api/v1/agent/control-commands:poll" \
    -H "authorization: Bearer ${bearer_token}" \
    -H "content-type: application/json" \
    --data-binary "@${payload_dir}/poll.json")"
  if [[ "${status}" != "200" ]]; then
    echo "agent command poll route returned ${status}" >&2
    exit 1
  fi

  status="$(curl -sS --noproxy "${NOPROXY_HOSTS}" -o /dev/null -w "%{http_code}" \
    -X POST "${ADMIN_BASE_URL%/}/api/v1/agent/action-results" \
    -H "authorization: Bearer ${bearer_token}" \
    -H "content-type: application/json" \
    --data-binary "@${payload_dir}/result.json")"
  if [[ "${status}" != "202" ]]; then
    echo "agent action result route returned ${status}" >&2
    exit 1
  fi

  local renew_response="${payload_dir}/renew-response.json"
  status="$(curl -sS --noproxy "${NOPROXY_HOSTS}" -o "${renew_response}" -w "%{http_code}" \
    -X POST "${ADMIN_BASE_URL%/}/api/v1/agent/credentials:renew" \
    -H "authorization: Bearer ${bearer_token}" \
    -H "content-type: application/json" \
    --data-binary "@${payload_dir}/renew.json")"
  if [[ "${status}" != "200" ]]; then
    echo "agent credential renew route returned ${status}" >&2
    cat "${renew_response}" >&2
    exit 1
  fi

  local renewed_bearer_token
  renewed_bearer_token="$(python3 - "${renew_response}" "${bearer_token}" <<'PY'
import json, pathlib, sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
old = sys.argv[2]
token = payload.get("credential_bundle", {}).get("bearer_token", "")
if not token.startswith("wic_"):
    raise SystemExit(f"renewed bearer token is invalid: {payload}")
if token == old:
    raise SystemExit("renewal returned the existing bearer token")
print(token)
PY
)"

  status="$(curl -sS --noproxy "${NOPROXY_HOSTS}" -o /dev/null -w "%{http_code}" \
    -X POST "${ADMIN_BASE_URL%/}/api/v1/agent/status" \
    -H "authorization: Bearer ${bearer_token}" \
    -H "content-type: application/json" \
    --data-binary "@${payload_dir}/status.json")"
  if [[ "${status}" != "401" ]]; then
    echo "old bearer token should be rejected after renewal, got ${status}" >&2
    exit 1
  fi

  status="$(curl -sS --noproxy "${NOPROXY_HOSTS}" -o /dev/null -w "%{http_code}" \
    -X POST "${ADMIN_BASE_URL%/}/api/v1/agent/status" \
    -H "authorization: Bearer ${renewed_bearer_token}" \
    -H "content-type: application/json" \
    --data-binary "@${payload_dir}/status.json")"
  if [[ "${status}" != "202" ]]; then
    echo "renewed bearer token should be accepted, got ${status}" >&2
    exit 1
  fi
}

start_agent_daemon() {
  if [[ -n "${DAEMON_PID}" ]] && kill -0 "${DAEMON_PID}" 2>/dev/null; then
    return 0
  fi
  echo "starting installed daemon for real status reporting..."
  env \
    NO_PROXY="${NOPROXY_HOSTS}" \
    no_proxy="${NOPROXY_HOSTS}" \
    HTTP_PROXY="" \
    HTTPS_PROXY="" \
    ALL_PROXY="" \
    http_proxy="" \
    https_proxy="" \
    all_proxy="" \
    "${BIN_PATH}" --config-dir "${CONFIG_DIR}" \
    >"${AGENT_STATUS_LOG}" 2>&1 &
  DAEMON_PID=$!
  sleep 0.5
  if ! kill -0 "${DAEMON_PID}" 2>/dev/null; then
    echo "agent daemon failed to start; log:" >&2
    cat "${AGENT_STATUS_LOG}" >&2
    exit 1
  fi
  echo "started agent daemon pid=${DAEMON_PID}"
}

overview_has_at_least_status_samples() {
  local overview_json="$1"
  local agent_id="$2"
  local min_count="$3"
  python3 - "${overview_json}" "${agent_id}" "${min_count}" <<'PY'
import json
import pathlib
import sys

try:
    payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
except Exception:
    sys.exit(1)
agent_id = sys.argv[2]
min_count = int(sys.argv[3])
for agent in payload.get("recent_online_agents") or payload.get("recentOnlineAgents") or []:
    if (agent.get("agent_id") or agent.get("agentId")) == agent_id:
        history = agent.get("metrics_history") or agent.get("metricsHistory") or []
        if len(history) >= min_count:
            sys.exit(0)
        break
sys.exit(1)
PY
}

wait_for_status_samples() {
  local agent_id="$1"
  local overview_json="$2"
  local min_count="${3:-2}"
  local timeout_seconds="${STATUS_REPORT_WAIT_TIMEOUT_SECONDS:-90}"
  local deadline=$(( $(date +%s) + timeout_seconds ))

  while (( $(date +%s) < deadline )); do
    if ! kill -0 "${DAEMON_PID}" 2>/dev/null; then
      echo "agent daemon exited during status reporting; log:" >&2
      cat "${AGENT_STATUS_LOG}" >&2
      exit 1
    fi
    curl -sS \
      --noproxy "${NOPROXY_HOSTS}" \
      -H "authorization: Bearer ${ADMIN_API_TOKEN}" \
      -o "${overview_json}" \
      "${ADMIN_BASE_URL%/}/api/v1/admin/agents/overview" || true
    if overview_has_at_least_status_samples "${overview_json}" "${agent_id}" "${min_count}"; then
      return 0
    fi
    sleep 0.5
  done

  echo "timed out waiting for ${agent_id} to report >=${min_count} status samples" >&2
  if [[ -f "${AGENT_STATUS_LOG}" ]]; then
    echo "agent daemon log:" >&2
    cat "${AGENT_STATUS_LOG}" >&2
  fi
  exit 1
}

validate_stored_status_metrics() {
  local store_file="$1"
  local agent_id="$2"
  local host_reports_resources="$3"
  python3 - "${store_file}" "${agent_id}" "${host_reports_resources}" <<'PY'
import json
import pathlib
import sys

store_path, agent_id, host_reports_resources = (
    pathlib.Path(sys.argv[1]),
    sys.argv[2],
    sys.argv[3] == "1",
)
snapshot = json.loads(store_path.read_text(encoding="utf-8"))
agent = (snapshot.get("agents") or {}).get(agent_id)
if not agent:
    raise SystemExit(f"agent {agent_id} not found in admin store")
history = agent.get("metrics_history") or []
if len(history) < 2:
    raise SystemExit(f"expected >= 2 status samples in admin store, got {len(history)}")
if agent.get("last_admin_latency_ms") is None:
    raise SystemExit("last_admin_latency_ms was not persisted")
last = history[-1]
if last.get("admin_latency_ms") is None:
    raise SystemExit("latest status sample is missing admin_latency_ms")
if host_reports_resources:
    memory = agent.get("last_memory_bytes")
    if not isinstance(memory, int) or memory <= 0:
        raise SystemExit("last_memory_bytes was not persisted on this host")
    cpu = agent.get("last_cpu_percent")
    if not isinstance(cpu, (int, float)):
        raise SystemExit("last_cpu_percent was not persisted on this host")

print("stored status metrics ok:")
print(
    json.dumps(
        {
            "last_memory_bytes": agent.get("last_memory_bytes"),
            "last_cpu_percent": agent.get("last_cpu_percent"),
            "last_admin_latency_ms": agent.get("last_admin_latency_ms"),
            "sample_count": len(history),
        },
        indent=2,
        sort_keys=True,
    )
)
PY
}

fetch_overview_final() {
  if ! curl -sS \
    --noproxy "${NOPROXY_HOSTS}" \
    -H "authorization: Bearer ${ADMIN_API_TOKEN}" \
    -o "${OVERVIEW_JSON}" \
    "${ADMIN_BASE_URL%/}/api/v1/admin/agents/overview"; then
    echo "failed to fetch final agent overview" >&2
    exit 1
  fi
  if ! python3 - "${OVERVIEW_JSON}" <<'PY'
import json
import pathlib
import sys

json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
PY
  then
    echo "admin overview response was not valid JSON" >&2
    cat "${OVERVIEW_JSON}" >&2
    exit 1
  fi
}

run_frontend_display_test() {
  local overview_json="$1"
  local agent_id="$2"
  local host_reports_resources="$3"
  local host_flag=""
  local args=()
  if [[ "${host_reports_resources}" == "1" ]]; then
    host_flag="resources"
  fi
  if ! command -v npx >/dev/null 2>&1; then
    echo "skipping frontend normalizer display test (npx not found)" >&2
    return 0
  fi
  if [[ ! -d "${REPO_ROOT}/crates/warp-insight-admin-web/node_modules" ]]; then
    echo "skipping frontend normalizer display test (admin-web node_modules missing)" >&2
    return 0
  fi
  args+=("${overview_json}" "${agent_id}")
  if [[ -n "${host_flag}" ]]; then
    args+=("${host_flag}")
  fi
  (
    cd "${REPO_ROOT}/crates/warp-insight-admin-web"
    npx tsx tests/overview-display.test.ts "${args[@]}"
  )
}

stop_agent_daemon() {
  if [[ -n "${DAEMON_PID}" ]] && kill -0 "${DAEMON_PID}" 2>/dev/null; then
    kill "${DAEMON_PID}" 2>/dev/null || true
    wait "${DAEMON_PID}" 2>/dev/null || true
  fi
  DAEMON_PID=""
}

start_continuous_agent_daemon() {
  local renew_response="${TMP_ROOT}/agent-api-payloads/renew-response.json"
  local current_count
  local target_count

  if [[ -n "${DAEMON_PID}" ]] && kill -0 "${DAEMON_PID}" 2>/dev/null; then
    echo "agent daemon is already running pid=${DAEMON_PID}"
    return 0
  fi

  # The credential-route checks renewed the agent credential, which invalidates
  # the token stored in agent_runtime.json at enrollment time. The daemon reads
  # its control-plane bearer token from that file at startup, so point it at
  # the renewed credential before launching the continuous daemon.
  if [[ ! -f "${renew_response}" ]]; then
    echo "warning: renewed credential response is missing (${renew_response});" >&2
    echo "the continuous daemon will use the runtime state credential." >&2
  else
    echo "updating agent runtime state with the renewed bearer credential..."
    python3 - "${STATE_PATH}" "${renew_response}" <<'PY'
import json
import pathlib
import sys

state_path = pathlib.Path(sys.argv[1])
renew_path = pathlib.Path(sys.argv[2])
renewed = json.loads(renew_path.read_text(encoding="utf-8"))
bundle = renewed.get("credential_bundle") or {}
token = bundle.get("bearer_token")
if not token:
    raise SystemExit("renewed credential bundle is missing bearer_token")
runtime = json.loads(state_path.read_text(encoding="utf-8"))
runtime["bearer_token"] = token
state_path.write_text(json.dumps(runtime, indent=2, sort_keys=True) + "\n", encoding="utf-8")
state_path.chmod(0o600)
print("agent runtime state bearer_token updated")
PY
  fi

  fetch_overview_final
  current_count="$(python3 - "${OVERVIEW_JSON}" "${AGENT_ID}" <<'PY'
import json
import pathlib
import sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
agent_id = sys.argv[2]
for agent in payload.get("recent_online_agents") or payload.get("recentOnlineAgents") or []:
    if (agent.get("agent_id") or agent.get("agentId")) == agent_id:
        print(len(agent.get("metrics_history") or agent.get("metricsHistory") or []))
        break
else:
    print("0")
PY
)"
  target_count=$(( current_count + 1 ))

  start_agent_daemon
  wait_for_status_samples "${AGENT_ID}" "${OVERVIEW_JSON}" "${target_count}"
  echo "agent daemon is continuously reporting (>= ${target_count} status samples)"
}

wait_for_started_services() {
  local pids=()
  if [[ -n "${ADMIN_PID}" ]]; then
    pids+=("${ADMIN_PID}")
  fi
  if [[ -n "${ADMIN_WEB_PID}" ]]; then
    pids+=("${ADMIN_WEB_PID}")
  fi
  if [[ "${#pids[@]}" -gt 0 ]]; then
    wait "${pids[@]}"
  fi
}

require_cmd curl
require_cmd python3
require_cmd openssl
require_https_admin_base_url

echo "testing install/enrollment against ${ADMIN_BASE_URL}"
echo "admin api token: ${ADMIN_API_TOKEN}"
if [[ "${SKIP_ADMIN_WEB}" != "1" ]]; then
  echo "testing admin web at ${ADMIN_WEB_BASE_URL}"
fi
echo "workspace: ${TMP_ROOT}"
echo "agent home: ${INSTALL_HOME}"

echo "checking admin install-code endpoint..."
ensure_install_code_endpoint
INSTALL_URL="$(extract_install_url)"
INSTALL_TOKEN="$(extract_install_token)"
extract_install_public_key >"${INSTALL_PUBLIC_KEY}"
echo "install url: ${INSTALL_URL}"

echo "checking admin enrollment route..."
check_enrollment_route

if [[ "${SKIP_ADMIN_WEB}" != "1" ]]; then
  echo "checking admin web service..."
  ensure_admin_web_service
fi

echo "downloading install script..."
curl_get "${INSTALL_URL}" >"${INSTALL_SCRIPT}"
curl_get "${INSTALL_URL}.sig" >"${INSTALL_SIGNATURE}"
openssl pkeyutl -verify -pubin -inkey "${INSTALL_PUBLIC_KEY}" -rawin -in "${INSTALL_SCRIPT}" -sigfile "${INSTALL_SIGNATURE}"
chmod 0755 "${INSTALL_SCRIPT}"

echo "running install script..."
INSTALL_ENV=(
  "WARP_INSIGHT_HOME=${INSTALL_HOME}"
  "WARP_INSIGHT_ENROLLMENT_TOKEN=${INSTALL_TOKEN}"
  "WARP_INSIGHT_START=0"
  "NO_PROXY=${NOPROXY_HOSTS}"
  "no_proxy=${NOPROXY_HOSTS}"
  "HTTP_PROXY="
  "HTTPS_PROXY="
  "ALL_PROXY="
  "http_proxy="
  "https_proxy="
  "all_proxy="
)
if [[ -f "${ADMIN_TLS_CA_CERT}" ]]; then
  INSTALL_ENV+=("CURL_CA_BUNDLE=${ADMIN_TLS_CA_CERT}")
fi
env \
  "${INSTALL_ENV[@]}" \
  sh "${INSTALL_SCRIPT}"

BIN_PATH="${INSTALL_HOME}/bin/warp-insightd"
CONFIG_DIR="${INSTALL_HOME}/.warp-insightd"
CONFIG_PATH="${CONFIG_DIR}/insightd.toml"
STATE_PATH="${INSTALL_HOME}/state/agent_runtime.json"

if [[ ! -x "${BIN_PATH}" ]]; then
  echo "installed binary is missing or not executable: ${BIN_PATH}" >&2
  exit 1
fi
if [[ ! -f "${CONFIG_PATH}" ]]; then
  echo "installed config is missing: ${CONFIG_PATH}" >&2
  exit 1
fi

echo "validating installed initial config..."
validate_initial_config "${CONFIG_PATH}"
set_test_agent_instance_name "${CONFIG_PATH}"

echo "checking installed daemon executable..."
"${BIN_PATH}" help >/dev/null

echo "running installed daemon once for enrollment..."
env \
  WARP_INSIGHTD_RUN_ONCE=1 \
  NO_PROXY="${NOPROXY_HOSTS}" \
  no_proxy="${NOPROXY_HOSTS}" \
  HTTP_PROXY="" \
  HTTPS_PROXY="" \
  ALL_PROXY="" \
  http_proxy="" \
  https_proxy="" \
  all_proxy="" \
  "${BIN_PATH}" --config-dir "${CONFIG_DIR}"

if [[ ! -f "${STATE_PATH}" ]]; then
  echo "runtime state was not written: ${STATE_PATH}" >&2
  exit 1
fi

validate_runtime_state "${STATE_PATH}"

if grep -Eq '^[[:space:]]*enrollment_token[[:space:]]*=' "${CONFIG_PATH}"; then
  echo "installed config still contains enrollment_token after enrollment: ${CONFIG_PATH}" >&2
  exit 1
fi

echo "checking real status reporting from the installed daemon..."
AGENT_ID="$(python3 - "${STATE_PATH}" <<'PY'
import json
import pathlib
import sys

print(json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))["agent_id"])
PY
)"
HOST_REPORTS_RESOURCES="0"
case "$(uname -s)" in
  Linux|Darwin) HOST_REPORTS_RESOURCES="1" ;;
esac
start_agent_daemon
wait_for_status_samples "${AGENT_ID}" "${OVERVIEW_JSON}"
if [[ -n "${ADMIN_PID}" ]]; then
  validate_stored_status_metrics "${TMP_ROOT}/admin-state/admin-store.json" "${AGENT_ID}" "${HOST_REPORTS_RESOURCES}"
fi
fetch_overview_final
run_frontend_display_test "${OVERVIEW_JSON}" "${AGENT_ID}" "${HOST_REPORTS_RESOURCES}"
stop_agent_daemon

echo "checking bearer-authenticated agent API routes..."
check_agent_credential_routes "${STATE_PATH}"

echo "install/enrollment test passed"
echo "admin api url: ${ADMIN_BASE_URL}"
if [[ "${SKIP_ADMIN_WEB}" != "1" ]]; then
  echo "admin web url: ${ADMIN_WEB_BASE_URL}"
fi
if [[ -n "${ADMIN_PID}" ]]; then
  echo "started admin api pid: ${ADMIN_PID}"
  echo "admin api log: ${ADMIN_LOG}"
fi
if [[ -n "${ADMIN_WEB_PID}" ]]; then
  echo "started admin web pid: ${ADMIN_WEB_PID}"
  echo "admin web log: ${ADMIN_WEB_LOG}"
fi
if [[ "${WAIT_FOR_EXIT}" == "1" && -t 0 ]]; then
  start_continuous_agent_daemon
  echo ""
  echo "全部验证通过。Agent 持续上报状态中（当前心跳间隔 ~3s）。"
  echo "工作区: ${TMP_ROOT}"
  echo "页面:   admin web ${ADMIN_WEB_BASE_URL}   admin api ${ADMIN_BASE_URL}"
  echo "按回车退出并清理所有服务..."
  read -r _ || true
  echo "收到退出，清理中..."
  STOP_STARTED_SERVICES=1
else
  if [[ -n "${ADMIN_PID}${ADMIN_WEB_PID}" && "${STOP_STARTED_SERVICES}" != "1" ]]; then
    echo "services started by this script are still running."
    echo "keep this terminal open while viewing the page."
    echo "stop them manually with: kill ${ADMIN_PID:-} ${ADMIN_WEB_PID:-}"
    echo "or run with STOP_STARTED_SERVICES=1 to clean them up automatically."
    if [[ "${WAIT_FOR_STARTED_SERVICES}" == "1" ]]; then
      wait_for_started_services
    fi
  fi
fi
