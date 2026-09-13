#!/usr/bin/env bash
# 网关运行期上报演示（自包含）：清空 .run 后运行本脚本，自动
#   1. 起 center（数据落 .run/center/store.json）
#   2. 创建网关（取 bootstrap + 实例 ID）
#   3. simulator 用 --fetch-config 程序运行时 onboarding：
#      自动落盘 .run/gateways/<gw>/<instance>/{config.toml,runtime-token}
#   4. 持续上报 → 刷新展示状态，直到 Ctrl+C
# 无需输入任何 token。
#
# 用法：
#   ./sysrun/warp-gateway/ctrl-plane/demo-gateway.sh                          # 默认 gw-demo / tok-demo
#   ./sysrun/warp-gateway/ctrl-plane/demo-gateway.sh gw-prod tok-xxxx         # 自定义网关名与 bootstrap token
#   DEMO_DURATION=30 ./sysrun/warp-gateway/ctrl-plane/demo-gateway.sh        # 跑 30 秒后自动退出
#
# 可覆盖 env：CENTER_URL / WEB_URL / GATEWAY_NAME / TOKEN / REPORT_INTERVAL / DEMO_DURATION / DEMO_REFRESH / SKIP_CENTER_WEB
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# 本脚本位于 sysrun/warp-gateway/ctrl-plane/：仓库根 = SCRIPT_DIR/../../..
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
# wist-agentd 已迁出 warp-insight workspace，独立 crate 位于仓库根的上级目录
AGENTD_CRATE="${REPO_ROOT}/../wist-agentd"
RUN_DIR="${REPO_ROOT}/.run"
mkdir -p "${RUN_DIR}/center" "${RUN_DIR}/gateways"

CENTER_BIN="${REPO_ROOT}/target/debug/wist-center"
SIM_BIN="${REPO_ROOT}/target/debug/insight-simulator"

CENTER_URL="${CENTER_URL:-http://127.0.0.1:3200}"
# 独立 WEB dev 端口（5174），避免与 demo-insight-center 的 5173 共用环境。
WEB_URL="${WEB_URL:-http://127.0.0.1:5174}"
SKIP_CENTER_WEB="${SKIP_CENTER_WEB:-0}"
GATEWAY_NAME="${GATEWAY_NAME:-${1:-gw-demo}}"
INSTANCE_ID="${INSTANCE_ID:-inst-${GATEWAY_NAME#gw-}}"
TOKEN="${TOKEN:-${2:-tok-demo}}"
# 若未显式传网关名，且 .run/gateways 只有唯一网关 → 自动复用最近 onboard 的（如刚 init 的网关）。
if [[ -z "${1:-}" && -z "${GATEWAY_NAME_ENV:-}" ]]; then
  gw_candidates=("${RUN_DIR}"/gateways/*/)
  if [[ -e "${gw_candidates[0]:-}" ]]; then
    if [[ ${#gw_candidates[@]} -eq 1 ]]; then
      GATEWAY_NAME="$(basename "${gw_candidates[0]}")"
      inst_candidates=("${RUN_DIR}"/gateways/"${GATEWAY_NAME}"/*/)
      if [[ -e "${inst_candidates[0]:-}" && ${#inst_candidates[@]} -eq 1 ]]; then
        INSTANCE_ID="$(basename "${inst_candidates[0]}")"
      fi
    fi
  fi
fi
# 复用模式：网关配置已存在（之前 onboard 过）→ 跳过 create/onboard，直接用现有配置。
REUSE=0
if [[ -f "${RUN_DIR}/gateways/${GATEWAY_NAME}/${INSTANCE_ID}/runtime-token" && \
      -f "${RUN_DIR}/gateways/${GATEWAY_NAME}/${INSTANCE_ID}/warp-gateway.toml" ]]; then
  REUSE=1
fi
REPORT_INTERVAL="${REPORT_INTERVAL:-3}"
DEMO_DURATION="${DEMO_DURATION:-0}"   # 0 = 持续直到 Ctrl+C
DEMO_REFRESH="${DEMO_REFRESH:-5}"
TMP_DIR="$(mktemp -d)"

CENTER_STARTED_BY_SCRIPT=0
CENTER_PID=""
CENTER_WEB_PID=""
SIM_PID=""
GATEWAY_PID=""

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

cleanup() {
  if [[ -n "${GATEWAY_PID}" ]] && kill -0 "${GATEWAY_PID}" 2>/dev/null; then
    kill "${GATEWAY_PID}" 2>/dev/null || true
    wait "${GATEWAY_PID}" 2>/dev/null || true
  fi
  if [[ -n "${SIM_PID}" ]] && kill -0 "${SIM_PID}" 2>/dev/null; then
    kill "${SIM_PID}" 2>/dev/null || true
    wait "${SIM_PID}" 2>/dev/null || true
  fi
  if [[ -n "${CENTER_WEB_PID}" ]] && kill -0 "${CENTER_WEB_PID}" 2>/dev/null; then
    kill "${CENTER_WEB_PID}" 2>/dev/null || true
    wait "${CENTER_WEB_PID}" 2>/dev/null || true
  fi
  if [[ "${CENTER_STARTED_BY_SCRIPT}" == "1" && -n "${CENTER_PID}" ]] && kill -0 "${CENTER_PID}" 2>/dev/null; then
    kill "${CENTER_PID}" 2>/dev/null || true
    wait "${CENTER_PID}" 2>/dev/null || true
  fi
  rm -rf "${TMP_DIR}"
  echo
  echo "演示结束，已清理 simulator / center-web / center 进程。"
}
trap cleanup EXIT

center_ready() {
  local code
  code="$(curl -s -o /dev/null -w "%{http_code}" "${CENTER_URL%/}/api/v1/admin/gateways" || true)"
  [[ "${code}" == "200" || "${code}" == "401" ]]
}

center_web_status() {
  curl -s -o /dev/null -w "%{http_code}" "${WEB_URL%/}/" || true
}

wait_until() {
  # wait_until <描述> <cmd...>
  local desc="$1"
  shift
  echo "等待 ${desc} 就绪..."
  for _ in {1..100}; do
    if "$@" >/dev/null 2>&1; then
      echo "  ${desc} 就绪"
      return 0
    fi
    sleep 0.2
  done
  echo "  ${desc} 未就绪，中止。" >&2
  exit 1
}

# ── 启动组件 ──

ensure_center() {
  echo "== 1. 启动 wist-center（数据落 .run/center/store.json）=="
  require_cmd lsof
  # 独占目标端口：清掉该端口上的残留 center（可能是旧 store），确保使用 .run/center/store.json。
  local port
  port="$(python3 -c "from urllib.parse import urlparse; print(urlparse('${CENTER_URL}').port or 80)")"
  local stale_pids
  stale_pids="$(lsof -ti "tcp:${port}" 2>/dev/null || true)"
  if [[ -n "${stale_pids}" ]]; then
    echo "  清理 ${CENTER_URL} 端口上的残留进程：${stale_pids}"
    kill ${stale_pids} 2>/dev/null || true
    sleep 0.5
  fi
  require_cmd cargo
  cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" -p wist-center >/dev/null
  host="$(python3 -c "from urllib.parse import urlparse; u=urlparse('${CENTER_URL}'); print(u.hostname or '127.0.0.1')")"
  port="$(python3 -c "from urllib.parse import urlparse; u=urlparse('${CENTER_URL}'); print(u.port or 80)")"
  WARP_INSIGHT_CENTER_LISTEN="${host}:${port}" \
  WARP_INSIGHT_CENTER_STORE_PATH="${RUN_DIR}/center/store.json" \
  WARP_INSIGHT_CENTER_PUBLIC_URL="${CENTER_URL}" \
    "${CENTER_BIN}" >/tmp/warp-gateway-demo-center.log 2>&1 &
  CENTER_PID=$!
  CENTER_STARTED_BY_SCRIPT=1
  wait_until "center" center_ready
}

setup_gateway() {
  echo "== 2. 创建网关实例（取 bootstrap + 实例 ID）=="
  local code
  code="$(curl -s -o "${TMP_DIR}/create.json" -w "%{http_code}" \
    -X POST "${CENTER_URL%/}/api/v1/admin/gateways/instances" \
    -H 'content-type: application/json' \
    -d "{\"gateway_name\":\"${GATEWAY_NAME}\",\"requested_by\":\"demo\",\"token\":\"${TOKEN}\"}")"
  if [[ "${code}" == "201" ]]; then
    echo "  创建成功 (201)"
  elif [[ "${code}" == "409" ]]; then
    echo "  已存在 (409，复用)"
  else
    echo "  创建失败: HTTP ${code}" >&2
    cat "${TMP_DIR}/create.json" >&2
    exit 1
  fi
  INSTANCE_ID="$(python3 - "${TMP_DIR}/create.json" <<'PY'
import json, sys
try:
    print(json.load(open(sys.argv[1]))["instance"]["instance_id"])
except Exception:
    print("")
PY
)"
  if [[ -z "${INSTANCE_ID}" ]]; then
    # FileStore create 暂不赋 instance_id；沿用约定 inst-<gw>。
    INSTANCE_ID="inst-${GATEWAY_NAME#gw-}"
    echo "  响应无 instance_id，使用派生：${INSTANCE_ID}"
  else
    echo "  instance_id = ${INSTANCE_ID}"
  fi
}

start_gateway_server() {
  echo "== 3. 启动 warp-gateway 服务（3000，供 gateway-web 代理 /api）=="
  require_cmd lsof
  # 独占 3000：清掉端口上的残留 warp-gateway（可能是旧网关的，避免 gateway-web 打到旧实例）。
  local stale_gw
  stale_gw="$(lsof -ti tcp:3000 2>/dev/null || true)"
  if [[ -n "${stale_gw}" ]]; then
    echo "  清理 3000 端口残留进程：${stale_gw}"
    kill ${stale_gw} 2>/dev/null || true
    sleep 0.5
  fi
  local dir="${RUN_DIR}/gateways/${GATEWAY_NAME}/${INSTANCE_ID}"
  local gw_bin="${REPO_ROOT}/target/debug/warp-gateway"
  if [[ ! -x "${gw_bin}" ]]; then
    require_cmd cargo
    cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" -p warp-gateway >/dev/null
  fi
  local state_dir="${dir}/state"
  mkdir -p "${state_dir}"
  if [[ ! -f "${state_dir}/admin-tls.crt.pem" ]]; then
    require_cmd openssl
    openssl req -x509 -newkey rsa:2048 -nodes \
      -keyout "${state_dir}/admin-tls.key.pem" \
      -out "${state_dir}/admin-tls.crt.pem" -days 365 -subj "/CN=localhost" \
      -addext "subjectAltName=IP:127.0.0.1,DNS:localhost" >/dev/null 2>&1
  fi
  # warp-gateway 启动校验 agent.package_file 存在；模板相对路径在 .run 下会解析错，
  # 改为仓库绝对路径，并确保 wist-agentd 已构建。
  if [[ ! -x "${AGENTD_CRATE}/target/debug/wist-agentd" ]]; then
    require_cmd cargo
    cargo build --manifest-path "${AGENTD_CRATE}/Cargo.toml" >/dev/null
  fi
  sed -i '' "s|^package_file = .*|package_file = \"${AGENTD_CRATE}/target/debug/wist-agentd\"|" "${dir}/warp-gateway.toml"
  # agent 安装期通过脚本内嵌 trust_bundle（--cacert）校验网关 TLS；
  # demo 用自签证书，直接把该证书本身嵌为信任锚（install.sh 内嵌 CA PEM 不能是占位符）。
  python3 - "${state_dir}/admin-tls.crt.pem" "${dir}/warp-gateway.toml" <<'PY'
import re, sys
nl = chr(10)
cert = open(sys.argv[1]).read().strip()
path = sys.argv[2]
text = open(path).read()
block = 'trust_bundle = """' + nl + cert + nl + '"""'
text = re.sub(
    r'(?ms)^trust_bundle = (""".*?"""|".*?")\s*\n',
    block + '\n',
    text,
    count=1,
)
open(path, "w").write(text)
PY
  WARP_GATEWAY_CONFIG="${dir}/warp-gateway.toml" \
    "${gw_bin}" >"/tmp/warp-gateway-demo-server.log" 2>&1 &
  GATEWAY_PID=$!
  echo "  warp-gateway 已启动 (pid=$!)，配置 ${dir}/warp-gateway.toml"
}

generate_gateway_admin_config() {
  echo "== 3. 生成网关自管配置（warp-gateway.toml，含 admin token）=="
  local gw_bin="${REPO_ROOT}/target/debug/warp-gateway"
  if [[ ! -x "${gw_bin}" ]]; then
    require_cmd cargo
    cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" -p warp-gateway >/dev/null
  fi
  local dir="${RUN_DIR}/gateways/${GATEWAY_NAME}/${INSTANCE_ID}"
  mkdir -p "${dir}"
  "${gw_bin}" init-config "${dir}/warp-gateway.toml"
  echo "  warp-gateway.toml 已生成：${dir}/warp-gateway.toml"
}

merge_admin_into_config() {
  local dir="${RUN_DIR}/gateways/${GATEWAY_NAME}/${INSTANCE_ID}"
  local cfg="${dir}/config.toml"
  local admin="${dir}/warp-gateway.toml"
  # 等 simulator onboarding 写入中心版 config.toml，然后把网关自管 [server]/[agent] 并入。
  for _ in $(seq 1 50); do
    [[ -f "${cfg}" ]] && break
    sleep 0.2
  done
  if [[ -f "${cfg}" && -f "${admin}" ]]; then
    {
      echo
      cat "${admin}"
    } >> "${cfg}"
    echo "  config.toml 已并入网关自管配置（含 admin_api_token）"
  else
    echo "  warn: config.toml / warp-gateway.toml 未就绪，未合并" >&2
  fi
}

ensure_gateway_web() {
  echo "== 3. 启动网关管理前端 gateway-web =="
  if [[ "${SKIP_CENTER_WEB}" == "1" ]]; then
    echo "  已跳过（SKIP_CENTER_WEB=1）"
    return
  fi
  if [[ "$(center_web_status)" == "200" ]]; then
    echo "  gateway-web 已在运行（${WEB_URL}），复用。"
    return
  fi
  require_cmd npm
  local web_dir="${REPO_ROOT}/crates/gateway-web"
  if [[ ! -d "${web_dir}/node_modules" ]]; then
    echo "  gateway-web 依赖缺失：${web_dir}/node_modules（先 cd 到该目录执行 npm install）" >&2
    exit 1
  fi
  host="$(python3 -c "from urllib.parse import urlparse; print(urlparse('${WEB_URL}').hostname or '127.0.0.1')")"
  port="$(python3 -c "from urllib.parse import urlparse; print(urlparse('${WEB_URL}').port or 80)")"
  (
    cd "${web_dir}"
    exec nohup npm run dev -- --host "${host}" --port "${port}" --strictPort \
      >/tmp/warp-gateway-demo-web.log 2>&1
  ) &
  CENTER_WEB_PID=$!
  echo "  启动 gateway-web：${WEB_URL} (pid=$!)"
  local ok=0
  for _ in {1..100}; do
    if [[ "$(center_web_status)" == "200" ]]; then
      ok=1
      break
    fi
    if ! kill -0 "${CENTER_WEB_PID}" 2>/dev/null; then
      break
    fi
    sleep 0.2
  done
  if [[ "${ok}" == "1" ]]; then
    echo "  gateway-web 就绪"
  else
    echo "  gateway-web 未就绪（日志 /tmp/warp-gateway-demo-web.log），跳过，不影响演示。"
    CENTER_WEB_PID=""
  fi
}

start_simulator() {
  if [[ "${REUSE}" == "1" ]]; then
    echo "== 4. 启动 simulator（复用现有 RUNTIME_TOKEN 上报，不重新 onboarding）=="
  else
    echo "== 4. 启动 simulator（--fetch-config 程序运行时 onboarding，自动落盘 .run/gateways/${GATEWAY_NAME}/${INSTANCE_ID}/）=="
  fi
  require_cmd cargo
  if [[ ! -x "${SIM_BIN}" ]]; then
    cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" -p insight-simulator >/dev/null
  fi
  local sim_token="${TOKEN}"
  local fetch_arg=""
  if [[ "${REUSE}" == "1" ]]; then
    sim_token="$(cat "${RUN_DIR}/gateways/${GATEWAY_NAME}/${INSTANCE_ID}/runtime-token")"
    fetch_arg=""
  else
    fetch_arg="--fetch-config"
  fi
  "${SIM_BIN}" gateway \
    --center-url "${CENTER_URL}" \
    --gateway-id "${GATEWAY_NAME}" --instance-id "${INSTANCE_ID}" \
    --token "${sim_token}" --interval "${REPORT_INTERVAL}" \
    ${fetch_arg} --report-agents \
    >"/tmp/warp-gateway-demo-sim-${GATEWAY_NAME}.log" 2>&1 &
  SIM_PID=$!
  echo "  simulator[${GATEWAY_NAME}/${INSTANCE_ID}] pid=$! → 每 ${REPORT_INTERVAL}s 上报"
}

# ── 持续循环：刷新网关状态，直到 Ctrl+C ──

demo_tick() {
  local now_ts
  now_ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "----- ${now_ts} -----"

  local status_raw
  status_raw="$(curl -s "${CENTER_URL%/}/api/v1/admin/gateways/status")"
  python3 - "${status_raw}" <<'PY'
import json, sys
try:
    statuses = json.loads(sys.argv[1])["statuses"]
except Exception:
    statuses = []
if not statuses:
    print("  (尚无网关状态上报)")
else:
    for s in sorted(statuses, key=lambda x: x.get("gateway_id", "")):
        print(f"  {s.get('gateway_id','?')} {s.get('status','?')}/{s.get('health','?')} last_seen={s.get('last_seen_at','?')}")
PY

  local agents_raw
  agents_raw="$(curl -s "${CENTER_URL%/}/api/v1/admin/gateways/${GATEWAY_NAME}/agents")"
  python3 - "${agents_raw}" <<'PY'
import json, sys
try:
    agents = json.loads(sys.argv[1])
except Exception:
    agents = []
if not agents:
    print("  (无 Agent 上报)")
else:
    parts = [f"{a['agent_id']}={a['status']}/{a['health']}" for a in agents]
    print(f"  Agents: " + " ".join(parts))
PY
}

demo_loop() {
  echo
  echo "== 5. 持续演示（每 ${DEMO_REFRESH}s 刷新）=="
  if [[ "${SKIP_CENTER_WEB}" != "1" ]]; then
    echo "  管理页面：${WEB_URL}（网关列表 / 初始化 / 状态卡片）"
  fi
  echo "  按 Ctrl+C 停止"
  echo
  local elapsed=0
  while true; do
    demo_tick
    echo
    if [[ "${DEMO_DURATION}" != "0" && "${elapsed}" -ge "${DEMO_DURATION}" ]]; then
      echo "演示时长 ${DEMO_DURATION}s 已到，结束。"
      break
    fi
    sleep "${DEMO_REFRESH}"
    elapsed=$((elapsed + DEMO_REFRESH))
  done
}

# ── 主流程 ──

require_cmd curl
require_cmd python3

echo "网关运行期上报演示（自包含：清空 .run 后运行即自动建立配置）"
echo "  center: ${CENTER_URL}（数据 .run/center/store.json）"
echo "  web:    ${WEB_URL}（SKIP_CENTER_WEB=1 可跳过）"
echo "  gateway: ${GATEWAY_NAME} / bootstrap token: ${TOKEN}"
echo "  .run 布局: .run/center + .run/gateways/<gw>/<instance>/"
echo "  上报间隔: ${REPORT_INTERVAL}s"
echo

ensure_center
if [[ "${REUSE}" == "1" ]]; then
  echo "  复用已 onboard 网关：${GATEWAY_NAME}/${INSTANCE_ID}（跳过 create/onboard）"
else
  setup_gateway
  generate_gateway_admin_config
fi
start_gateway_server
start_simulator
if [[ "${REUSE}" != "1" ]]; then
  merge_admin_into_config
fi
ensure_gateway_web
demo_loop
