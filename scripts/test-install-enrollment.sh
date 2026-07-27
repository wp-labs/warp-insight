#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/warp-insight-install-enroll.XXXXXX")"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  if [[ "${KEEP_WARP_INSIGHT_ENROLL_TEST:-0}" != "1" ]]; then
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

require_cmd cargo
require_cmd python3

SERVER_DIR="${TMP_ROOT}/server"
AGENT_DIR="${TMP_ROOT}/agent"
CONFIG_DIR="${TMP_ROOT}/config"
mkdir -p "${SERVER_DIR}" "${AGENT_DIR}" "${CONFIG_DIR}"

python3 -u - "${SERVER_DIR}" <<'PY' &
import json
import pathlib
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

server_dir = pathlib.Path(sys.argv[1])
server_dir.mkdir(parents=True, exist_ok=True)

class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        return

    def do_GET(self):
        if self.path == "/health":
            self.send_response(204)
            self.send_header("content-length", "0")
            self.end_headers()
            return
        self.send_response(404)
        self.send_header("content-length", "0")
        self.end_headers()

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        raw = self.rfile.read(length)
        (server_dir / "enrollment-request.raw").write_bytes(raw)
        request = json.loads(raw.decode("utf-8"))
        (server_dir / "enrollment-request.json").write_text(
            json.dumps(request, indent=2, sort_keys=True),
            encoding="utf-8",
        )

        if self.path != "/api/v1/agent/enroll":
            self.send_response(404)
            self.end_headers()
            return

        body = {
            "result": {
                "status": "accepted",
                "reason_code": None,
                "agent_id": "agent-shell-test",
                "instance_id": "instance-shell-test",
                "issued_identity": {
                    "agent_id": "agent-shell-test",
                    "instance_id": "instance-shell-test",
                    "tenant_id": "tenant-shell-test",
                    "environment_id": "env-shell-test",
                    "node_id": request["host_profile"]["node_id"],
                    "issued_at": "2026-07-27T00:00:00Z",
                    "expires_at": None,
                    "status": "active",
                },
                "credential_bundle": None,
                "initial_config": {
                    "schema_version": "v1",
                    "mode": "managed",
                    "gateway_endpoint": "http://127.0.0.1",
                    "policy_version": "v1",
                    "telemetry_output": None,
                },
                "policy_binding": {
                    "agent_id": "agent-shell-test",
                    "policy_id": "default-agent-policy",
                    "policy_version": "v1",
                    "bound_at": "2026-07-27T00:00:00Z",
                },
            }
        }
        encoded = json.dumps(body).encode("utf-8")
        self.send_response(201)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
httpd.timeout = 10
endpoint = f"http://127.0.0.1:{httpd.server_address[1]}"
(server_dir / "endpoint").write_text(endpoint, encoding="utf-8")
try:
    for _ in range(8):
        httpd.handle_request()
        if (server_dir / "enrollment-request.json").exists():
            break
except Exception as exc:
    (server_dir / "server-error.txt").write_text(repr(exc), encoding="utf-8")
    raise
PY
SERVER_PID=$!

for _ in {1..100}; do
  if [[ -s "${SERVER_DIR}/endpoint" ]]; then
    break
  fi
  sleep 0.05
done

if [[ ! -s "${SERVER_DIR}/endpoint" ]]; then
  echo "fake enrollment server did not start" >&2
  exit 1
fi

CONTROL_ENDPOINT="$(cat "${SERVER_DIR}/endpoint")"

python3 - "${CONTROL_ENDPOINT}/health" <<'PY'
import sys
import urllib.request

request = urllib.request.Request(sys.argv[1], method="GET")
with urllib.request.urlopen(request, timeout=5) as response:
    if response.status != 204:
        raise SystemExit(f"unexpected health status: {response.status}")
PY

cat >"${CONFIG_DIR}/insightd.toml" <<EOF
schema_version = "v1"

[agent]
instance_name = "shell-install-host"

[control_plane]
enabled = true
endpoint = "${CONTROL_ENDPOINT}"
enrollment_token = "token-shell-test"
credential_request = "none"
tls_mode = "http"
auth_mode = "enrollment_token"

[paths]
root_dir = "${AGENT_DIR}"
run_dir = "run"
state_dir = "state"
log_dir = "log"

[execution]
max_running_actions = 1
cancel_grace_ms = 5000
default_stdout_limit_bytes = 1048576
default_stderr_limit_bytes = 1048576

[discovery]
host_enabled = true
network_enabled = false
endpoint_enabled = false
process_enabled = false
container_enabled = false

[telemetry.logs]
spool_dir = "state/spool/logs"

[telemetry.logs.output]
kind = "file"

[telemetry.logs.output.file]
path = "log/warp-parse-records.ndjson"
EOF

echo "building warp-insightd and warp-insight-exec..."
cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" -p warp-insightd -p warp-insight-exec

echo "running install/enrollment smoke test..."
if ! env \
  WARP_INSIGHTD_RUN_ONCE=1 \
  WARP_INSIGHT_EXEC_BIN="${REPO_ROOT}/target/debug/warp-insight-exec" \
  NO_PROXY="127.0.0.1,localhost" \
  no_proxy="127.0.0.1,localhost" \
  HTTP_PROXY="" \
  HTTPS_PROXY="" \
  ALL_PROXY="" \
  http_proxy="" \
  https_proxy="" \
  all_proxy="" \
  "${REPO_ROOT}/target/debug/warp-insightd" --config-dir "${CONFIG_DIR}"; then
  if [[ -f "${SERVER_DIR}/server-error.txt" ]]; then
    echo "fake server error:" >&2
    cat "${SERVER_DIR}/server-error.txt" >&2
  fi
  exit 1
fi

RUNTIME_STATE="${AGENT_DIR}/state/agent_runtime.json"
REQUEST_JSON="${SERVER_DIR}/enrollment-request.json"

python3 - "${RUNTIME_STATE}" "${REQUEST_JSON}" <<'PY'
import json
import pathlib
import sys

runtime_path = pathlib.Path(sys.argv[1])
request_path = pathlib.Path(sys.argv[2])

runtime = json.loads(runtime_path.read_text(encoding="utf-8"))
request = json.loads(request_path.read_text(encoding="utf-8"))

assert runtime["agent_id"] == "agent-shell-test", runtime
assert runtime["instance_id"] == "instance-shell-test", runtime
assert request["token"] == "token-shell-test", request
assert request["host_profile"]["node_id"] == "shell-install-host", request
assert request["kind"] == "submit_enrollment_request", request

print("enrollment request:")
print(json.dumps(request, indent=2, sort_keys=True))
print("runtime state:")
print(json.dumps(runtime, indent=2, sort_keys=True))
PY

echo "install/enrollment smoke test passed"
