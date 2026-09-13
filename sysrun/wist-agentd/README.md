# sysrun/wist-agentd —— wist-agentd 运行目录

本目录承载 wist-agentd 的运行配置与启停脚本（与 `sysrun/warp-gateway/{bin,data-plane,monitor,ctrl-plane}` 同属运行资产，区别于源码 crate）。

## 布局

```
sysrun/wist-agentd/
├── agentd.toml           # 运行设定（[agent]/[control_plane]/[telemetry.logs]/[paths]）
├── tasks/macos-p0.toml   # 采集工作任务清单（agentd.toml 的 file_inputs_file 引用）
├── start.sh              # 启动（后台常驻 / --foreground）
├── stop.sh               # 停止
├── bin/                  # 可选：独立 wist-agentd 可执行文件（脱离仓库运行时放置）
├── conf/                 # 可选：备用配置目录
└── run|state|log/        # 运行时产物（已 gitignore）
```

配置分层：`agentd.toml` 只放稳定度较高的运行设定；采集任务（tail 哪些文件、head/tail 策略）
是低稳定度内容，独立在 `tasks/macos-p0.toml`，增删任务不必动运行设定。

## 用法

```bash
# 启动（默认二进制：bin/wist-agentd → crate 根 target/debug/wist-agentd）
./start.sh
# 前台联调
./start.sh --foreground
# 停止
./stop.sh
```

可覆盖 env：

```bash
WIST_AGENTD_BIN=/path/to/wist-agentd ./start.sh          # 指定二进制
WIST_AGENTD_CONFIG_DIR=/path/to/config ./start.sh        # 指定配置目录（须含 agentd.toml）
```

## 当前实例（macOS P0 采集）

- 采集 `/var/log/{install.log, com.apple.xpc.launchd/launchd.log, shutdown_monitor.log, fsck_apfs.log, wifi.log}`（全部 tail，任务清单见 `tasks/macos-p0.toml`）
- TCP → 本机数据面 `sysrun/warp-gateway/data-plane`（127.0.0.1:9000，macos_agent 规则 → `macos-agent.json`）
- 注册到本机 gateway 控制面（`[control_plane] endpoint=https://127.0.0.1:3000`；注册成功会自动清除配置文件中的 enrollment_token 行）
- 日志：`log/agentd.out`；checkpoint/状态：`state/`（重启不重放）
