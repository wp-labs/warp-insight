# Changelog

本项目的显著变更按版本记录，格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

## [0.1.1] - 2026-08-03

### 新增
- **状态指标链路**：admin store 为每个 Agent 保留最近 100 条状态样本（滚动窗口，3s 心跳下约 5 分钟）；
  daemon 周期自上报内存/CPU/管理端延时（`/api/v1/agent/status`）；overview 返回历史并渲染趋势线
- **macOS 资源测量**：daemon 的 `current_rss_bytes` / `cpu_ticks` 增加 macOS 实现
  （`proc_pidinfo` / `getrusage`），此前仅 Linux 可测，macOS 上内存/CPU 恒为 null
- **测试脚本真实上报验证**：`scripts/test-install-enrollment.sh` 启动真实 daemon 心跳，
  验证状态样本落入 admin store、overview API 与真实前端归一化链路
  （新增 `crates/warp-insight-admin-web/tests/overview-display.test.ts` 防回归）
- **持续运行模式**：测试脚本（`WAIT_FOR_EXIT`，默认开启）验证通过后保持 daemon 持续上报，
  等待用户按回车退出并统一清理
- **服务网络访问**：admin/web 默认监听 `0.0.0.0`，自动探测主局域网 IP（跳过隧道/CGNAT/
  benchmark 段）并写入 TLS 证书 SAN，URL 默认使用局域网地址（可用
  `ADMIN_BASE_URL` / `ADMIN_WEB_BASE_URL` 覆盖）
- **前端限流封禁提示**：新增 `RateLimitNotice`，当接口返回 HTTP 429 时页面显示带倒计时的
  封禁提示（首页与安装页）

### 修复
- **Agent 状态数据显示**：前端 `normalizeRecentOnlineAgent` 丢弃了后端返回的
  `memoryBytes` / `cpuPercent` / `adminLatencyMs` / `metricsHistory` 字段，
  导致卡片上内存/CPU/延时恒显示 "—"、趋势线为空
- **IP 限流误封**：缺失 token 的请求不再计入认证失败（此前 web 填 token 前的轮询会累计
  5 次失败把 IP 封禁 60s，导致填对 token 后长时间无输出）
- **复制按钮失效**：`CopyButton` 在非安全上下文（局域网 HTTP）下 `navigator.clipboard`
  不可用，改用 `document.execCommand("copy")` 降级，失败时显示「复制失败」
- **远程安装 SSL**：引导命令下载脚本改用 `curl -fsSLk`（脚本经命令内嵌公钥的
  签名校验兜底）；install.sh 内嵌管理端 CA，包与初始配置下载用 `--cacert` 严格校验
  （配置含单次 enrollment token，不做裸 `-k`）
- **脚本启动即退出**：`set -euo pipefail` 下 token 生成管道触发 SIGPIPE(141) 导致
  `test-install-enrollment.sh` 在开头就退出，已用有界输入 + `|| true` 修复

### 变更
- **Admin token**：测试脚本生成 10 位混合大小写字母数字（约 60bit 熵）；
  admin 配置新增强度校验（长度 ≥8、估算熵 ≥40bit、去重字符 ≥3），弱 token 启动即报错
  （`MIN_ADMIN_API_TOKEN_BYTES` 由 16 调整为 8，配合熵校验把关）
- 测试脚本 curl 的 `--noproxy` 与 daemon 的 `NO_PROXY` 统一纳入探测到的局域网 IP

### 新增文件
- `crates/warp-insight-admin-web/src/components/RateLimitNotice.tsx`
- `crates/warp-insight-admin-web/src/components/RateLimitNotice.module.css`
- `crates/warp-insight-admin-web/tests/overview-display.test.ts`
- `CHANGELOG.md`
