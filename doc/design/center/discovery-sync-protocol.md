# warp-insight Discovery Sync 协议设计

## 1. 文档目的

本文档定义边缘 `warp-insightd` 通过 `warp-parse` discovery receiver 向中心同步 discovery 本地快照时的协议行为。

重点回答：

- discovery sync 在 ingress 协议里属于哪类消息
- 边缘何时发送 `ReportDiscoverySnapshot`
- `warp-parse` 如何返回 `DiscoveryIngestAck`
- 断线重连、重复投递、revision 倒退和局部失败如何处理

相关文档：

- [`report-discovery-snapshot-schema.md`](report-discovery-snapshot-schema.md)
- [`control-center-storage-schema.md`](control-center-storage-schema.md)
- [`../telemetry/telemetry-uplink-and-warp-parse.md`](../telemetry/telemetry-uplink-and-warp-parse.md)
- [`../edge/resource-discovery-runtime.md`](../edge/resource-discovery-runtime.md)
- [`../foundation/glossary.md`](../foundation/glossary.md)

---

## 2. 核心结论

当前阶段固定以下结论：

- discovery sync 属于 `warp-parse` 的 discovery ingress 协议
- 边缘发送对象是 `ReportDiscoverySnapshot`
- `warp-parse` 返回对象是 `DiscoveryIngestAck`
- 第一版采用 agent 主动推送到 discovery 端口，不要求中心主动拉取
- 第一版采用完整快照幂等替换模型，不做 delta patch 协议
- discovery sync 与 telemetry uplink 分离，但可复用同一 ingress/runtime 家族

一句话说：

- 一个独立 discovery 端口
- 一套独立的 discovery sync 消息
- 一次一份完整快照
- 由 ack 驱动重试和清理

---

## 3. 协议角色

### 3.1 边缘 `warp-insightd`

负责：

- 维护本地 `DiscoverySnapshot`
- 判断是否形成新的待同步快照
- 生成 `ReportDiscoverySnapshot`
- 处理 `DiscoveryIngestAck`

不负责：

- 中心侧资源目录归并
- 跨节点资源去重
- discovery sync 最终审计裁决

### 3.2 `warp-parse` discovery receiver

负责：

- 接收 discovery sync 上行消息
- 读取并校验 `proto head`
- 校验 envelope 基本字段
- 执行幂等判定
- 返回 `DiscoveryIngestAck`
- 把已接收快照转交给中心存储与归并链路

不负责：

- 修改 discovery 快照内容
- 推断边缘应该发现什么
- 在网关层完成完整资源归并

### 3.3 中心存储与目录归并层

负责：

- receipt 落库
- agent 最近已确认快照索引更新
- 资源目录归并

---

## 4. 消息分类

discovery sync 使用以下两类消息：

### 4.1 上行消息

边缘到中心：

- `report_discovery_snapshot`

对应对象：

- `ReportDiscoverySnapshot`

### 4.2 返回消息

`warp-parse` 到边缘：

- `discovery_ingest_ack`

对应对象：

- `DiscoveryIngestAck`

### 4.3 不引入的新消息

第一版不引入：

- `request_discovery_snapshot`
- `request_discovery_refresh`
- `discovery_snapshot_patch`

原因：

- 先把边缘主动上送主链路跑通
- 避免在 discovery 仍快速演进阶段过早引入拉模式和 patch 复杂度

---

## 5. 端口与 `proto head`

### 5.1 discovery 端口

第一版建议：

- `warp-parse` 为 discovery 提供独立 recv 端口
- discovery 端口只接 discovery 协议族消息

### 5.2 为什么端口分离后仍需要 `proto head`

即使 discovery 使用独立端口，仍建议保留 `proto head`，用于：

- 快速拒绝发错端口的流量
- 协议版本识别
- `message_kind` 识别
- 编码与压缩方式识别
- body 长度边界校验

### 5.3 `proto head` 格式

第一版建议固定：

- `proto head` 固定长度 `64 bytes`
- 编码固定为 `ASCII`
- 字段名与字段值统一使用全大写
- 字段顺序固定
- 不足 `64 bytes` 时用右侧空格补齐
- body 紧跟在 head 之后

head 采用单行文本格式：

```text
WII1;V=1;K=DSNAP;E=JSON;C=NONE;L=000018432;F=00;
```

说明：

- 上例只是未补齐前的可读形式
- 实际线上传输时，整段需要右侧补空格到 `64 bytes`

### 5.4 字段定义

```text
ProtoHead {
  MAGIC
  V
  K
  E
  C
  L
  F
}
```

第一版 discovery 建议固定：

- `MAGIC = WII1`
- `V = 1`
- `K = DSNAP`
- `E = JSON`
- `C = NONE`
- `F = 00`

字段说明：

- `MAGIC`
  固定魔数，第一版为 `WII1`
- `V`
  协议版本
- `K`
  消息种类
- `E`
  body 编码方式
- `C`
  body 压缩方式
- `L`
  body 长度，使用十进制零填充
- `F`
  预留 flags，第一版固定 `00`

### 5.5 取值约束

第一版建议：

- `K=DSNAP`
  表示 `ReportDiscoverySnapshot`
- `E=JSON`
  表示 body 使用 JSON 编码
- `C=NONE`
  表示 body 不压缩

后续可扩展但当前不要求：

- `K=DACK`
- `C=GZIP`
- `C=ZSTD`

### 5.6 格式约束

第一版建议固定以下规则：

- 字段顺序固定，不允许乱序
- 不允许重复字段
- 不允许未知字段
- 不做大小写兼容，必须全大写
- `L` 表示 body 实际传输长度
- 若未来启用压缩，`L` 表示压缩后的 body 长度

### 5.7 body

当 `K=DSNAP` 时：

- body 直接按 `ReportDiscoverySnapshot` 解码

### 5.8 接收规则

`warp-parse` discovery receiver 建议按如下顺序处理：

1. 固定读取 `64 bytes` head
2. 去掉右侧 padding 空格
3. 校验是否以 `WII1;` 开头
4. 按 `;` 切分字段
5. 严格按固定顺序解析 `V/K/E/C/L/F`
6. 校验 `V=1`
7. 校验 discovery 端口是否允许 `K=DSNAP`
8. 校验 `E=JSON`
9. 校验 `C` 是否为当前 receiver 支持的压缩方式
10. 校验 `L` 是否合法且未超过 discovery 端口上限
11. 再按 `L` 读取 body
12. 若 `C != NONE`，先解压
13. 按 `E=JSON` 解码并反序列化为 `ReportDiscoverySnapshot`

### 5.9 示例

示例 head：

```text
WII1;V=1;K=DSNAP;E=JSON;C=NONE;L=000018432;F=00;
```

示例 wire layout：

```text
[64 bytes ASCII HEAD][JSON BODY]
```

---

## 6. 基本时序

### 6.1 首次成功快照上送

```text
warp-insightd
  -> warp-parse discovery port:
     proto head(message_kind=report_discovery_snapshot)
  -> warp-parse discovery port:
     body ReportDiscoverySnapshot(snapshot_id=s1, revision=1)
warp-parse
  -> warp-insightd: discovery_ingest_ack(snapshot_id=s1, ack_status=accepted)
```

### 6.2 revision 前进后的上送

```text
warp-insightd
  -> local discovery refresh
  -> new DiscoverySnapshot(revision=2, snapshot_id=s2)
  -> warp-parse discovery port: report_discovery_snapshot(snapshot_id=s2, revision=2)
warp-parse
  -> warp-insightd: discovery_ingest_ack(snapshot_id=s2, ack_status=accepted)
```

### 6.3 断线恢复后的补发

```text
warp-insightd
  -> local discovery refresh creates snapshot s3
  -> send s3
  -> connection lost before ack
  -> reconnect to discovery port
  -> resend s3 with report_attempt + 1
warp-parse
  -> warp-insightd: discovery_ingest_ack(snapshot_id=s3, ack_status=accepted|duplicate)
```

---

## 7. 发送规则

### 7.1 发送前提

边缘发送 `ReportDiscoverySnapshot` 前至少应满足：

- discovery 端口可连接
- 本地已形成成功的 `DiscoverySnapshot`
- 该快照已经进入待上送 reporting 状态

### 7.2 何时创建待上送项

第一版建议：

1. 首轮成功 refresh 后，创建一条待上送项
2. 之后仅当 `DiscoverySnapshot.revision` 前进时创建新待上送项
3. 同一 `snapshot_id` 未确认前，可重试，不再生成并行重复项

### 7.3 不建议的行为

第一版不建议：

- 每次 daemon tick 都重发当前快照
- 为单个 resource 变化即时独立上送
- 在未建立会话时无限制堆积多个未确认 revision

### 7.4 待发送窗口

第一版建议边缘本地只需保证：

- 至少保留“最近一个未确认快照”

可选增强：

- 保留一个很小的 pending window，例如最近 `N=2..4` 个未确认快照

但在 discovery 当前阶段，不建议把它扩成复杂日志式队列。

---

## 8. discovery receiver 处理流程

`warp-parse` discovery receiver 收到 `ReportDiscoverySnapshot` 后建议按如下顺序处理：

1. 读取并校验 `proto head`
2. 校验 `kind / api_version`
3. 校验 `snapshot_id / revision / reported_at`
4. 校验 `snapshot.snapshot_id` 与 envelope `snapshot_id` 是否一致
5. 校验 `snapshot.revision` 与 envelope `revision` 是否一致
6. 执行幂等判定
7. ingress receipt 落库
8. 更新 agent 当前 discovery snapshot 索引
9. 触发异步资源目录归并
10. 返回 `DiscoveryIngestAck`

### 8.1 receiver 可拒绝的情况

建议至少支持以下拒绝原因：

- `proto head` 非法
- `message_kind` 不匹配
- schema 非法
- `snapshot_id` 与 `snapshot.snapshot_id` 不一致
- `revision` 与 `snapshot.revision` 不一致

### 8.2 receiver 不应做的事

receiver 不应：

- 修改 `resources[]`
- 修改 `targets[]`
- 依赖某个单独资源字段做业务推断

---

## 9. Ack 语义

### 9.1 `accepted`

表示：

- 当前快照已被 discovery receiver 接收
- ingress receipt 已建立
- 边缘可以清理本地待确认 reporting 状态

注意：

- `accepted` 不等于中心全局资源目录已经完全归并完成
- 归并可以异步继续推进

### 9.2 `duplicate`

表示：

- 相同 `snapshot_id` 或等价上送已处理

边缘处理原则：

- 将其视为成功确认
- 清理本地待确认项

### 9.3 `stale`

表示：

- 当前快照 revision 落后于中心已确认的 agent 最新 revision

边缘处理原则：

- 不再重试该快照
- 记录本地控制面可见告警或统计

### 9.4 `rejected`

表示：

- envelope 非法或会话不合法

边缘处理原则：

- 不应盲目无限重试
- 应等待会话修复、配置修复或版本修复后再重发

---

## 10. 幂等与顺序

### 9.1 幂等对象

discovery sync 需要同时处理两类幂等：

- 消息投递幂等
- agent 快照推进顺序

### 9.2 推荐唯一约束

ingest 层至少建议建立：

- `discovery_report_receipts(report_id)`
- `agent_discovery_snapshots(agent_id, snapshot_id)`

### 9.3 revision 判定

第一版建议：

- 同一 `agent_id` 维护一条“最新已确认 revision”
- 收到更小 revision 时返回 `stale`
- 收到相同 revision 且 `snapshot_id` 相同，可返回 `duplicate`

### 9.4 不强依赖严格连续

第一版不强制要求：

- `revision` 必须连续无缺口

原因：

- 边缘可能仅保留最近一份待确认快照
- 断线期间可能跳过一些未发送或未保留的本地 revision

因此中心更应该保证：

- 最近一次已确认快照可见

而不是要求：

- 每一次中间 revision 都必须到达

---

## 11. 失败与重试

### 10.1 发送失败

若边缘发送失败且未收到 ack：

- 保留本地待确认项
- 增加 `report_attempt`
- 按退避策略重试

### 10.2 ack 超时

若发送成功但 ack 超时：

- 边缘不得假设中心未收到
- 后续应按原 `snapshot_id` 重发
- ingress 层可返回 `accepted` 或 `duplicate`

### 10.3 会话中断

会话中断后：

- 不删除待确认项
- 会话恢复后优先补发最近未确认快照

### 10.4 中心内部归并失败

若 discovery receiver 已 receipt 落库，但后续资源目录归并失败：

- 仍可返回 `accepted`

原因：

- 边缘需要确认“快照已被 ingest 层可靠接收”
- 资源目录归并属于中心后续异步处理阶段

---

## 12. 流控与预算

### 11.1 第一版目标

discovery sync 第一版优先保证：

- 正确性
- 幂等
- 连接恢复后的最终一致

不优先追求：

- 高频实时性
- 超低延迟 patch 流

### 11.2 流控建议

第一版建议：

- 同一 agent 同时最多只保留一个 in-flight discovery report
- 未确认时不再并发发送第二个快照
- 若本地新 revision 产生，可用“新 revision 覆盖旧待发送项”的方式收敛

### 11.3 与 telemetry 隔离

必须明确：

- discovery sync 不应与 telemetry uplink 共享同一个拥塞预算
- telemetry 拥塞不应阻塞 discovery sync 完全失联
- discovery sync 也不应反向挤占主要 telemetry 数据面预算

第一版可先通过：

- 独立消息类型
- 独立端口
- 独立发送队列或独立调度优先级

来实现基本隔离。

---

## 13. 安全要求

discovery sync 至少应满足：

- `proto head` 和 body 都必须可校验
- `agent_id / instance_id` 不允许伪造漂移
- schema 校验失败必须显式拒绝

第一版可先不要求：

- 对 `ReportDiscoverySnapshot` 单独签名

但若后续引入对象级签名，也应保持：

- envelope 签名是传输增强
- 不改变 `DiscoverySnapshot` 本地对象语义

---

## 14. 与其它协议对象的关系

### 14.1 与 telemetry uplink

- telemetry uplink 可继续走自己的 receiver / 端口
- discovery sync 走 discovery receiver / discovery 端口

两者可以同属 `warp-parse` ingress，但不能混为同一 payload 语义。

### 14.2 与 `ReportActionResult`

二者共同点：

- 都是边缘到中心的上行对象
- 都需要幂等和 ack

二者差异：

- `ReportActionResult` 面向执行闭环
- `ReportDiscoverySnapshot` 面向资源事实同步

### 14.3 与 `Agent Gateway`

- `Agent Gateway` 仍负责 register / heartbeat / action dispatch / result reporting
- discovery sync 不再经过 `Agent Gateway`

### 14.4 与心跳

心跳不携带完整 discovery 快照。

第一版建议最多只在心跳摘要里暴露：

- 当前已确认 discovery revision
- 是否存在待确认 discovery report

不建议在心跳中嵌入完整 `snapshot`

---

## 15. 第一版实现建议

### 14.1 Step 1

先实现 discovery ingress 协议对象与最小 `proto head`：

- `report_discovery_snapshot`
- `discovery_ingest_ack`

### 14.2 Step 2

再实现边缘 reporting 状态：

- pending snapshot
- `report_attempt`
- ack 后清理

### 14.3 Step 3

再实现 discovery receiver 幂等接收：

- receipt
- current snapshot index
- ack 返回

### 14.4 Step 4

最后再实现中心目录归并和查询

---

## 16. 当前决定

当前阶段固定以下结论：

- discovery sync 走 `warp-parse` discovery receiver
- discovery 端口和 telemetry 端口分离
- discovery 端口仍保留 `proto head`
- 边缘主动上送 `ReportDiscoverySnapshot`
- `warp-parse` 返回 `DiscoveryIngestAck`
- 第一版只做完整快照同步，不做 patch
- ack 只确认 ingest 层已接收，不承诺资源目录归并已完成
