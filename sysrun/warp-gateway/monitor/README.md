# warp-gateway data-plane 观测栈（monitor）

参考 `wparse/wp-monitor/install/docker`（warp-observing）适配到 warp-insight。
用 Docker Compose 在本机快速拉起 3 个服务，用来观测
`sysrun/warp-gateway/data-plane`（wparse 0.25.21，宿主进程跑在 9000/1514）：

- `victoria-metrics`：指标存储，默认暴露宿主端口 `18429`（容器内 8428）
- `victoria-logs`：日志存储，默认暴露宿主端口 `19429`（容器内 9428）
- `wp-monitor`：wparse 监控面板，默认暴露宿主端口 `10428`（容器内 18080）

## 环境变量（.env / .env.example）

```env
RETENTION_PERIOD=15d
VLOG_MAX_DISK_SPACE_USAGE_BYTES=50GiB
```

- `RETENTION_PERIOD`：指标与日志数据保留时间
- `VLOG_MAX_DISK_SPACE_USAGE_BYTES`：日志最大磁盘空间，超过后触发清理

> Docker Hub 拉取失败时，`start.sh` 会自动回退到镜像加速源并重打回原 tag。
> 默认加速源为 `docker.m.daocloud.io`，可用环境变量 `REGISTRY_MIRROR` 覆盖；设为空值可禁用该兜底。

## 启动 / 停止

```bash
# 首次启动（无 .env 时自动按 .env.example 生成）
./start.sh
# 强制按 .env.example 重新生成 .env 后再启动
./start.sh -f

# 停止（保留数据卷）
./stop.sh
# 停止并删除数据卷（清空指标/日志）
./stop.sh -v
```

访问入口：

```
wparse 观测平台 : http://localhost:10428
victoria-metrics: http://localhost:18429
victoria-logs    : http://localhost:19429
```

> 面板容器通过 compose 内部服务名（`victoria-metrics:8428` / `victoria-logs:9428`）访问存储，
> 见 `wp-monitor/config/app.toml`；宿主机上 wparse 的 sink 则走上面映射出的宿主端口。

## 接入数据面（sysrun/warp-gateway/data-plane）

`sysrun/warp-gateway/data-plane/connectors/sink.d/` 已内置连接器
`20-victoriametrics_sink.toml` / `19-victorialogs_sink.toml`，只需在 sink 分组里接线：

### 1) 指标 → monitor 分组

在 `sysrun/warp-gateway/data-plane/topology/sinks/infra.d/monitor.toml` 的 `[[sink_group.sinks]]` 增加（可保留原
`file_proto_text_sink` 的本地文本镜像，或按需替换为纯远程）：

```toml
[[sink_group.sinks]]
name = "metrics_vmetrics_sink"
connect = "victoriametrics_sink"
tags = ["sink:vmetrics"]

[sink_group.sinks.params]
endpoint = "http://127.0.0.1:18429"   # api_path 默认 /api/v1/import/prometheus，可省略
```

### 2)（可选）miss/残差镜像 → victoria-logs

在 `sysrun/warp-gateway/data-plane/topology/sinks/infra.d/miss.toml` 增加：

```toml
[[sink_group.sinks]]
name = "victorialogs_output"
connect = "victorialogs_sink"
tags = ["wp_stage:miss"]

[sink_group.sinks.params]
endpoint = "http://127.0.0.1:19429"    # api_path 默认 /insert/jsonline，可省略
```

改完 topology 后重启数据面让新 sink 生效：

```bash
sysrun/warp-gateway/data-plane/stop-wparse.sh
sysrun/warp-gateway/data-plane/start-wparse.sh
```

## 备注

- wp-monitor 镜像（`ghcr.io/wp-labs/wp-monitor:latest`）需能访问 ghcr.io；镜像 tag 版本由
  wp-labs 发布节奏决定，如需固定版本可改 `docker-compose.yml` 中 `image`。
- 数据卷：`metrics_data` / `logs_data`（本地 named volume），`stop.sh` 不删卷，`-v` 才删。
