#!/usr/bin/env bash
# 启动 warp-gateway data-plane 观测栈（victoria-metrics + victoria-logs + wp-monitor）。
#
# 用法：
#   ./start.sh        首次启动（无 .env 时按 .env.example 生成默认 .env）
#   ./start.sh -f     强制按 .env.example 重新生成 .env 后再启动
# 停止：./stop.sh（-v 连数据卷一起删）
#
# 入口：
#   wparse 观测平台 : http://localhost:10428
#   victoria-metrics: http://localhost:18429
#   victoria-logs    : http://localhost:19429
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.yml"
ENV_FILE="${SCRIPT_DIR}/.env"
ENV_EXAMPLE_FILE="${SCRIPT_DIR}/.env.example"
COMPOSE_CMD=()

usage() {
  echo "用法: ./start.sh [-f]" >&2
  exit 1
}

ensure_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "未检测到 docker，请先安装 Docker Desktop 或 docker CLI。" >&2
    exit 1
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "检测到 docker 命令，但 Docker 未启动，请先启动 Docker。" >&2
    exit 1
  fi
}

resolve_compose_cmd() {
  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
    return 0
  fi
  if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
    return 0
  fi
  echo "未检测到 docker compose 或 docker-compose，请先安装 Docker Compose。" >&2
  exit 1
}

# Docker Hub 在当前网络环境常不可达；配置镜像加速源作为拉取失败时的兜底。
# 留空可禁用兜底（REGISTRY_MIRROR=）。
REGISTRY_MIRROR="${REGISTRY_MIRROR:-docker.m.daocloud.io}"

# 需要从 Docker Hub 拉取的镜像（与 docker-compose.yml 中的 image 保持一致）。
DOCKERHUB_IMAGES=(
  "victoriametrics/victoria-metrics:v1.133.0"
  "victoriametrics/victoria-logs:v1.43.0"
)

# 确保镜像本地存在：先直连 Docker Hub，失败再走 REGISTRY_MIRROR 拉取并重打回原 tag。
ensure_image() {
  local image="$1"
  if docker image inspect "$image" >/dev/null 2>&1; then
    return 0
  fi
  echo "镜像不存在，尝试从 Docker Hub 拉取：${image}"
  if docker pull "$image"; then
    return 0
  fi
  if [[ -n "${REGISTRY_MIRROR}" ]]; then
    echo "Docker Hub 拉取失败，改用镜像加速源 ${REGISTRY_MIRROR} 拉取并重打 tag ..."
    docker pull "${REGISTRY_MIRROR}/${image}"
    docker tag "${REGISTRY_MIRROR}/${image}" "${image}"
  else
    echo "拉取 ${image} 失败，且未配置 REGISTRY_MIRROR。" >&2
    return 1
  fi
}

ensure_env() {
  local force_render="${1:-0}"
  if [[ ! -f "${ENV_EXAMPLE_FILE}" ]]; then
    echo "未找到 .env.example：${ENV_EXAMPLE_FILE}" >&2
    exit 1
  fi
  if [[ -f "${ENV_FILE}" && "${force_render}" != "1" ]]; then
    echo "检测到已存在的 .env，跳过生成（需要重新生成请加 -f）。"
    return 0
  fi
  if [[ -f "${ENV_FILE}" ]]; then
    echo "检测到 -f，按 .env.example 重新生成 .env。"
  fi
  cp "${ENV_EXAMPLE_FILE}" "${ENV_FILE}"
  echo "配置已保存到 .env（默认值来自 .env.example，可直接编辑后重启）。"
}

main() {
  local force_render="0"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -f) force_render="1" ;;
      -h|--help) usage ;;
      *) echo "不支持的参数: $1" >&2; usage ;;
    esac
    shift
  done

  [[ -f "${COMPOSE_FILE}" ]] || { echo "未找到 compose 文件: ${COMPOSE_FILE}" >&2; exit 1; }

  ensure_docker
  resolve_compose_cmd
  ensure_env "${force_render}"

  for img in "${DOCKERHUB_IMAGES[@]}"; do
    ensure_image "$img"
  done

  echo "开始启动服务（${COMPOSE_FILE}）..."
  "${COMPOSE_CMD[@]}" -f "${COMPOSE_FILE}" up -d

  echo
  echo "访问入口："
  echo "  - wparse 观测平台 : http://localhost:10428"
  echo "  - victoria-metrics: http://localhost:18429"
  echo "  - victoria-logs    : http://localhost:19429"
  echo "停止：./stop.sh"
}

main "$@"
