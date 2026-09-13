# warp-insight 项目约定

## 网关控制台（crates/gateway-web）UI 约定

- **主题**：深色控制台，沿用 `index.css` 里的设计令牌（分层表面 / 语义状态色 / `--series-*` 图表色板 /
  `--content-max` 内容区宽度）。不要硬编码颜色，不要新增第二套色板。
- **语义色分工**：`--crit` = 真的在丢数据 / 失败；`--warn` = 观测链路或能力降级（服务本身没坏）；
  `--ok` = 健康；`--text-4` = 降权但仍需可见。同色不同因会让操作者做错动作。
- **数字**：等宽数字（`font-variant-numeric: tabular-nums`）；带「万 / 亿」单位的数值不要用
  `--font-mono`（缺 CJK 字形会掉字号），用正文字体 + tabular-nums。
- **灰阶必须过 WCAG AA（正文 4.5:1）**，深色主题也一样。当前值（2026-09-13 提亮）：
  `--bg #101a2e` / `--bg-elevated #16223a` / `--surface #1c2a45` / `--surface-2 #24334f` /
  `--border #3d4e72`；文本 `--text #f4f7fc` / `--text-2 #c9d4e6` / `--text-3 #a3b1c9` / `--text-4 #8693ab`
  （对比度分别约 16 / 11.6 / 8.0 / 5.6:1）。
  改灰阶后要重新算这四个比值 —— 曾经的 `--text-4 #5a6780` 只有 2.95:1，KPI 口径说明、
  计数、图表轴标签都用它，整页看起来「暗淡」就是这个原因。
- **唯一的例外**：`ShellCode.module.css` 是语法主题本身，允许字面色值；其余样式一律用令牌。
  代码块底色用 `--code-bg #0d1828` + `--code-text #dbe6f7`，不要硬编码近黑（会在提亮后的页面里变黑洞）。
- **成熟度标记**：对外材料用 ✅已落地 / 🟡规划中 / ✗不做。

### 运维看板列表约定（`/pipeline`，页面名 **数据采集**，2026-09-13 由「采集管道」改名）

- **按「内容的量级」分节，不是按对称美观**：/pipeline 分两节 ——
  「接入与输出」（来源 + 输出并排，两者数量稳定且少）、「解析规则」（单独一节、铺满宽度）。
  依据是真实基数：**来源/输出通常只有几个，解析规则可能很多**。
  满宽那一节的列体用 `grid-template-columns: repeat(auto-fill, minmax(360px, 1fr))`
  把规则卡片自动分栏；分段控件要 `max-width: 320px`，别被拉满整行。

- 同一层的条目按 **「活跃 / 静默」tab 切换**（不是上下两段）：`速率 > 0` = 活跃，`速率 = 0` = 静默。
  一次只显示一组；tab 状态按层独立保存；数量为 0 的一侧保留但禁用（`静默 0` 本身就是健康信息）；
  某层没有活跃项时自动落到静默页，避免开出空白列表。
- 静默项**降权但不隐藏、不禁用交互** —— 「配了却没流量」本身往往就是要处理的信号。
  实现：`styles.silentItem`（透明背景 + 弱化边框 + `--text-3` 文字）。
- 静默项带 **「最后活跃 HH:MM」/「窗口内无数据」徽标**（从序列里最后一个非零采样点现场推导，
  不需要后端新增字段）。
- **恒定 0 的序列不要画成负值量程**：`TrendChart` 的退化量程补边在 `pad === 0` 时要贴 0（0…1），
  否则全 0 的速率会显示 `-1…1`，看起来像在 0 上下波动。

### 布局陷阱（已踩过）

- **严禁"跨阶段合计"**：wparse 三个计数器是**同一条流水线上同一批事件的三个阶段**
  （`wparse_receive_data` 接入 → `wparse_parse_all` 解析 → `wparse_send_to_sink` 输出）。
  把"收到多少"和"写出多少"相加不构成任何物理量 —— 2026-09-13 我犯过这个错，
  给一张画着「入流 + 落存储」两条线的图起名「本节合计速率」，被用户当场指出没有意义。
  **只有两类求和是合法的**：(a) 同阶段内跨节点求和（Σ各来源 = 接入吞吐；Σ各输出口 = 落盘吞吐）；
  (b) 逐阶段同口径对比（差额 = 丢在哪一步）。唯一可称"合计"的是「未落存储」= miss + residue + error
  （同阶段的三个兜底通道）。
- **页面上的图一律跟「选中项」走，不画固定总览**（2026-09-13 定稿）。
  「接入与输出」节内与底部「节点详情」各放一份 `SelectionCharts`（速率 + 累计量两图），
  点来源 / 出口 / 规则即切换。两份内容是同一组件、同一份数据 —— 用户明确要求"复制一份到当前位置"，
  **这是有意重复，不要再自作主张删掉其中一个**。
  **未选中任何条目时，节内那张图默认画「来源汇总」**（Σ各来源的速率与累计量，前端按时间戳求和）；
  再点一次已选中的条目即取消选择、回到汇总（`handleSelect` 内做反选）。
  曾经的固定总览图（「接入 / 落存储」两条线）与「解析阶段速率」图都已删除：
  前者是把两阶段塞进一张"合计"图（概念错误），后者 ≈ 接入速率（无信息量）。
  后端 `summary.ingressSeries / egressSeries / parseSeries` 仍在返回，前端已无引用。
- **解析规则节不放趋势图**（2026-09-13 按用户要求移除）。原因：解析阶段速率 ≈ 接入速率
  （每个事件都会被解析一次；实测 31 个采样点里 11 点完全相同、最大差 68 e/s），
  这张图回答不了任何问题。要恢复价值应改画**每条规则的速率**（多线 / Top N），或「解析未通过的量」——
  后者需先核实 miss 究竟是"没匹配到规则"还是"匹配了但解析报错"，未核实不画。
  `summary.parseSeries` 后端仍返回，前端已无引用（保留未删，需要时可用于规则级多线图）。
- **节点详情固定给两个图**：速率 + 累计量并排（`repeat(2, minmax(0,1fr))`，<1080px 改上下）。
  不要用「速率/累计量」分段控件切换 —— 切换会把另一半藏起来，而这两者是两种读法
  （一个看当下、一个看总量），应该同时可见。
- 坐标轴标签用 `formatAxis`：带「万」时 **≥100 万不留小数**（"1000.0万" 很难读）。

- **限高的纵向 flex 容器里，子项必须 `flex: 0 0 auto`**：`max-height + overflow-y: auto` 的列里，
  子元素若带 `overflow: hidden`，其自动最小尺寸会退化成 0 → 卡片被压扁、内容被横切，
  而不是让列滚动。`.group` / `.node` / `.empty` 都要加。
- 窄列（1250px 窗口下三列各约 250px）用**换行**而不是省略号保住名称：
  `.nodeBody { flex-wrap: wrap }` + `.nodeStats { margin-left: auto }`。

## 服务与数据面

- **看到 `VictoriaMetric periodic push failed` 先分清两种原因**（2026-09-13 两次实测）：
  1. **对端真的没了**（整机重启 / 容器停机）。此时报错是**正确的**，不是故障。判别方法：
     `sysctl -n kern.boottime` 看是否刚重启；`docker logs monitor-victoria-metrics-1 | grep -c "starting VictoriaMetrics"`
     看 VM 启动过几次（容器未重建则日志保留全部启动横幅）；`docker inspect <c> --format '{{.State.FinishedAt}}'`
     看停机时刻。
  2. **长驻进程的客户端卡死**（VM 全程在跑、端点健康、新进程能写）。表现：错误 1 次/秒且是**即时**错误
     （`timeout_secs = 5`，若是超时会 5 秒一次，实测是 1 秒一次 → 内核立即返回错误，不是写阻塞）；
     到 18429 的连接保持 ESTABLISHED 但请求全失败。**唯一恢复手段是重启写入进程**
     （wparse 与 warp-gateway 都会中招）。**根因未证实**：14:09 那次的 OrbStack/转发进程已随 17:18 重启消失，
     无法回溯；已证实的是"失败后不重建连接 → 永久失败"这个机制。
- **wparse / warp-agentd / warp-gateway 都没有开机自启，也没有自愈**（launchd 里没有任何相关任务，
  只有手写 `start-wparse.sh` / `stop-wparse.sh`）。所以：整机重启后指标会一直空着直到人工启动
  （2026-09-13 17:18 开机 → 17:32 才手动启动，空窗 14 分钟）；客户端卡死也只能人工重启。
- **VM 指标导出粒度 60 秒**，所以速率一律按 1 分钟粒度聚合，不做「实时 e/s」。
- **admin token 会随重新 onboard 轮换**：见 `.run/gateways/a4/inst-a4/warp-gateway.toml` 的
  `admin_api_token`。连续用旧 token 探测会触发失败认证限流（429）。
- **重启数据面前先查 `topology/sources/`**：`file_1.toml` 这类演示源（`file = "gen*.dat"`）在
  `enable = false` 时**依然会被构建**，通配符匹配不到文件就会让 wparse 校验失败 `exit 100` ——
  即「重启一次就死」。该源已移除。
- `bin/` 下 `wparse` / `wpadm` / `wpgen` / `wprescue` 是下载的未入库二进制，曾带
  `com.apple.quarantine`（Gatekeeper 会静默杀掉进程：秒退 + 零日志 + 不监听端口）。

## 未解决的遗留

- **日志只落本地 JSON**（`data-plane/data/out_dat/macos-agent.json`，约 16.5 GB/天，无轮转）。
  已就绪的根治路径：`monitor/README.md` 与 `miss.toml` 提到的 `victorialogs_output`，
  连接器 `connectors/sink.d/19-victorilogls_sink.toml` 已在、VictoriaLogs 跑在 19429。
- `models/wpl/parse.wpl` 里的演示包 `package /nginx/ { rule example {...} }` 在生产数据面被加载
  但从不命中，会让 /pipeline 多出一个静默的 `/nginx/`。
- `crates/warp-gateway/src/api/mod.rs` 带 `// @jumo generated` 头，而 `/pipeline` 路由是
  **模型外手工添加**（`NOTE(hand-added)`）。重新生成控制面代码会丢掉这条路由 ——
  与当年的 `host-metrics` 同样情况，应把 entry 补进 `jumo/model/static/control/binding.mju`。
