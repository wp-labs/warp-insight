# macOS 安全/审计日志 WPL 样本集

对应设计文档：[`doc/design/telemetry/macos-security-audit-log-sources.md`](../../../../../../doc/design/telemetry/macos-security-audit-log-sources.md)

每个子目录代表一类值得采集的 macOS 安全/审计日志。目前仅保留**真机实采**样本：

- 目录内提供 `sample.dat`：该类日志的真实文本样本（供后续 WPL 开发 / `wpgen sample` 回放 / 联调用）
- 暂无真实样本的目录**不放置任何构造内容**，待真机采集后补充（见下表）

> ⚠️ **不要把开发期 `parse.wpl`/OML 草稿放在本目录**：数据面引擎会**递归加载 `models/wpl/` 下所有规则**，
> `mac/` 里的草稿一旦放进来就会在运行时生效，匹配真实日志产生错误分类与 `default`/`residue` 杂音（已实测）。
> 约定：`mac/<类>/` 只保留 `sample.dat` 素材；规则草稿统一放 [`models/mac-drafts/<类>/`](../../mac-drafts/)，
> 联调接入时再合入正式规则目录（上送帧规则在 `models/wpl/macos_agent/parse.wpl`）。

## 目录 → 类别/来源映射

| 目录 | 类别（设计文档 §5） | 主要来源位置 |
|---|---|---|
| `login-session/` | 登录、退出与会话（§5.1） | `last`/`/var/log/wtmp`、`btmp`、OpenBSM `lo,la`、统一日志 `loginwindow`/`sshd` |
| `privilege-execution/` | 提权与命令执行（§5.2） | `sudo`（system.log/统一日志）、OpenBSM `ex/pc` |
| `software-change/` | 系统与软件变更（§5.3） | `/var/log/install.log`、`softwareupdated`、MDM/profile |
| `crash-panic/` | 崩溃与内核 panic（§5.4） | `/Library/Logs/DiagnosticReports/*.ips`、`.panic` |
| `network-firewall/` | 网络、防火墙与 WiFi（§5.5） | 统一日志 `com.apple.alf`、`/var/log/wifi.log`、`mDNSResponder` |
| `privacy-tcc/` | 隐私授权与 Keychain（§5.6） | 系统/用户 `TCC.db` 事件（`tccd`）、`securityd` |
| `gatekeeper-quarantine/` | Gatekeeper/隔离（§5.6） | `syspolicyd`、LaunchServices、`com.apple.quarantine` |
| `launchd-service/` | 服务生命周期（§5.3） | `/var/log/com.apple.xpc.launchd/launchd.log*` |
| `reboot-power/` | 关机/重启（§5.3/§5.4） | `/var/log/shutdown_monitor.log`、`last reboot` |
| `misc-system/` | 系统杂项（§5.7） | `fsck_apfs*`、`cups/`、`apache2/` 等 |

## 样本来源与状态（写作时，macOS 26.6 本机）

> 原则：目录内只保留真机实采的 `sample.dat`；构造内容一律不保留，
> 无真实事件可采的目录暂不提供样本文件。

| 目录 | `sample.dat` |
|---|---|
| `login-session/` | 真实：本机 `last -n 8` |
| `software-change/` | 真实：`/var/log/install.log` 近期（installer/softwareupdated） |
| `crash-panic/` | 真实：`/Library/Logs/DiagnosticReports/*.ips` 首行（Jetsam 元数据） |
| `network-firewall/` | 真实：`/var/log/wifi.log` 近期行 |
| `launchd-service/` | 真实：`launchd.log` 近期行 |
| `reboot-power/` | 真实：`last reboot` / `last shutdown` |
| `misc-system/` | 真实：`/var/log/fsck_apfs.log` |
| `privilege-execution/` | 暂无（本机近期无 sudo/sshd 事件；需 root 或制造事件后采集） |
| `privacy-tcc/` | 暂无（需真实授权动作后采集） |
| `gatekeeper-quarantine/` | 暂无（需下载隔离应用等真实事件后采集） |

刷新方式：按设计文档 §10 的核对命令在目标版本真机重跑后覆盖 `sample.dat`，例如：

```bash
last -n 20 > login-session/sample.dat
tail -n 50 /var/log/install.log > software-change/sample.dat
head -n 1 /Library/Logs/DiagnosticReports/*.ips > crash-panic/sample.dat
```

## 样本使用约定

- 目录内 `sample.dat` 均为真机实采（见上方来源表），未放置任何构造内容。
- 不同 macOS 小版本、不同来源（文件 vs 统一日志 vs OpenBSM 导出）的**字段与格式会变化**，接入前请在目标版本真机重新核对。
- `sample.dat` 的用途：

  1. 为该类编写 `parse.wpl` 时的输入样本；
  2. `wpgen sample` 回放验证解析/路由；
  3. 作为接入配置（`file_inputs` glob）的冒烟数据。

- 暂无样本的目录（见上表）待真机采集后再补 `sample.dat`。
- OpenBSM audit（`/var/audit`）与统一日志原始记录不是可直接 tail 的文本，样本需由“导出器”生成文本后再进本目录；当前阶段优先覆盖可直接 tail 的文件型样本。

## 目录级 WPL 开发流程（每类规则草稿放 `models/mac-drafts/<类>/`）

1. 在 `models/mac-drafts/<类>/` 内新建 `parse.wpl`（语法校验：`wpadm rule parse --wpl <dir>` 或 `wpadm check`）；
2. 用本目录的 `sample.dat` 做样例回放：`wpgen sample --wpl <dir>`；
3. 命中并核对字段/分类后，把规则合入正式数据面规则目录并配 sink 组：
   - wist-agentd 上送帧（macos P0 采集）→ 合并进 `models/wpl/macos_agent/parse.wpl`（sink：`macos-agent`）；
   - 未来新增独立来源类 → 新建 `models/wpl/<pkg>/parse.wpl` 包目录 + 对应 OML 与 `topology/sinks` 分组；
4. 重启数据面（`stop-wparse.sh` / `start-wparse.sh`）后核对 `data/out_dat/` 与 `data/rescue/`。

> `models/wpl/parse.wpl`（示例 nginx 规则）与 `models/wpl/macos_agent/`（生产上送规则）不受本目录影响。
