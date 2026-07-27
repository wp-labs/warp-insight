# warp-insight 管理平台容器与 Agent Bootstrap 设计

## 1. 文档目的

本文档记录第一版 `warp-insight` 管理平台容器如何作为 agent 安装、注册和控制面接入入口。

重点回答：

- agent 如何知道管理节点地址
- 是否需要端口扫描发现管理节点
- 管理平台容器应包含哪些逻辑能力
- 安装 URL 应如何携带 enrollment 信息
- `advertise_url`、`enrollment_url`、`gateway_url` 如何区分

相关文档：

- [`identity-enrollment-protocol.md`](identity-enrollment-protocol.md)
- [`agent-gateway-protocol.md`](agent-gateway-protocol.md)
- [`control-center-architecture.md`](control-center-architecture.md)
- [`control-plane.md`](control-plane.md)
- [`../foundation/security-model.md`](../foundation/security-model.md)

---

## 2. 核心结论

第一版建议采用：

- 用户先运行管理平台容器
- 管理平台容器提供 Web UI / API 和安装引导入口
- 管理平台生成带短期 token 的 agent 安装 URL
- agent 安装脚本写入明确的 `enrollment_url`、`gateway_url` 和 trust bundle
- agent 首次注册使用 `HTTPS server TLS + enrollment_token + CSR`
- 注册成功后使用平台签发的 client certificate 走 mTLS 控制面

一句话说：

agent 不主动扫描网络找管理节点；管理平台容器生成可信安装入口，安装入口把控制面地址明确写入 agent bootstrap 配置。

---

## 3. 为什么不使用端口扫描

端口扫描不应作为默认发现机制。

原因：

- 容易触发企业安全告警
- 跨网段、Kubernetes、云 VPC、防火墙环境中不可靠
- 扫到开放端口也不能证明对方是合法控制中心
- 发现过程不可审计，难以做最小权限约束
- 与 `enrollment_token + trust_bundle + mTLS` 的可信注册模型不匹配

允许保留一个显式开启的实验室模式：

- 必须配置 allowlist CIDR
- 必须配置固定端口集合
- 必须校验 server TLS 和平台身份
- 默认关闭

生产默认路径应是：

```text
management platform install URL -> agent bootstrap config -> enrollment -> mTLS control plane
```

---

## 4. 管理平台容器定位

管理平台容器不是一个被 agent 自动发现的 Docker image。

它应被定义为：

```text
warp-insight management platform container
  = Control Center
  + Enrollment Gateway
  + Agent Gateway
  + Install Bootstrap Endpoint
  + Agent Registry
  + Token / Certificate Service
  + Embedded Metadata Store
```

第一版可以做成 all-in-one 容器：

```text
warp-insight-platform
  ├─ web-ui / northbound-api
  ├─ enrollment-service
  ├─ agent-gateway
  ├─ agent-registry
  ├─ token-cert-service
  ├─ control-center
  ├─ embedded-db
  └─ optional telemetry receiver / warp-parse receiver
```

但必须保持逻辑边界：

- 控制面负责注册、心跳、能力、任务、证书、审计
- 数据面负责 logs / metrics / discovery 等 telemetry 接入
- 即使物理上同容器部署，控制面和数据面也不应混成同一协议

---

## 5. `advertise_url`

管理平台容器必须在运行时知道自己的对外访问地址。

原因：

- Docker image 构建时不知道最终运行在哪台机器
- 容器内部 IP 不能作为 agent 的默认控制面地址
- 同一平台可能通过宿主机 IP、域名、Load Balancer 或 Ingress 暴露

第一版建议使用显式配置：

```bash
docker run -d \
  --name warp-insight-platform \
  -p 8443:8443 \
  -v wpi-data:/var/lib/warp-insight \
  -e WPI_ADVERTISE_URL=https://10.0.0.25:8443 \
  -e WPI_BOOTSTRAP_TOKEN_TTL=30m \
  warp-insight-platform:latest
```

`WPI_ADVERTISE_URL` 用途：

- 生成安装 URL
- 生成 agent bootstrap 配置中的 `enrollment_url`
- 生成 agent bootstrap 配置中的 `gateway_url`
- 写入平台证书或服务端身份校验提示

如果没有显式 `advertise_url`，平台可以在本地开发模式提示候选地址，但不能把容器内部地址静默写入安装脚本。

---

## 6. 安装 URL 模型

管理平台启动后提供安装入口。

示例：

```text
https://10.0.0.25:8443/install.sh?token=bootstrap_xxx
```

或：

```bash
curl -fsSL "https://10.0.0.25:8443/install.sh?token=bootstrap_xxx" | sudo bash
```

安装脚本至少负责：

1. 下载并校验 agent 安装包
2. 写入 trust bundle
3. 写入 enrollment bootstrap 配置
4. 写入短期 enrollment token 或 token 文件
5. 安装并启动 `warp-insightd`

安装完成后的最小配置示例：

```toml
[control_plane]
mode = "managed"
enrollment_url = "https://10.0.0.25:8443/v1/enrollment"
gateway_url = "https://10.0.0.25:8443"
trust_bundle = "/etc/warpinsight/ca.pem"
enrollment_token_file = "/etc/warpinsight/enrollment.token"
```

约束：

- 安装 URL 不应长期有效
- token 应有 TTL、使用次数限制和撤销能力
- 安装包应有摘要或签名校验
- trust bundle 必须来自安装入口或预置可信来源
- 安装脚本不能让 agent 通过端口扫描推断管理平台地址

---

## 7. Bootstrap 到 Enrollment 流程

推荐流程：

```text
operator runs management platform container
  -> platform loads advertise_url
  -> platform generates bootstrap token
  -> operator copies install URL
  -> target machine runs install command
  -> installer writes enrollment_url / gateway_url / trust_bundle / token
  -> warp-insightd first start
  -> generate local keypair and CSR
  -> POST /v1/enrollment with token + CSR + node facts
  -> platform returns agent_id / instance_id / client certificate
  -> agent persists identity atomically
  -> agent connects gateway with mTLS
  -> heartbeat / capability report / long polling starts
```

这里有两个不同阶段：

- bootstrap 阶段：
  使用安装 URL、trust bundle 和短期 token。
- control 阶段：
  使用正式 agent identity 和 client certificate。

`enrollment_token` 只用于首次注册，不应作为正式控制面长期凭证。

---

## 8. Endpoint 字段边界

第一版建议区分以下字段。

### 8.1 `advertise_url`

管理平台自身对外声明地址。

由平台运行时配置。

示例：

```text
https://10.0.0.25:8443
```

### 8.2 `enrollment_url`

agent 首次注册接口。

由安装脚本写入 agent bootstrap 配置。

示例：

```text
https://10.0.0.25:8443/v1/enrollment
```

### 8.3 `gateway_url`

agent 注册成功后的控制面 Gateway 根地址。

示例：

```text
https://10.0.0.25:8443
```

### 8.4 `telemetry_url`

可选的数据面上报地址。

它可以与 `gateway_url` 同源，也可以指向单独的 WarpParse / telemetry receiver。

示例：

```text
tcp://10.0.0.25:9000
https://10.0.0.25:9443/v1/telemetry
```

约束：

- `gateway_url` 是控制面地址
- `telemetry_url` 是数据面地址
- 两者即使部署在同一容器，也必须保持协议语义分离

---

## 9. 部署形态

### 9.1 单机 all-in-one

适合第一版验证和小规模环境。

```text
operator machine
  └─ warp-insight-platform container
       ├─ control center
       ├─ enrollment / gateway
       ├─ embedded db
       └─ optional telemetry receiver
```

agent 连接：

```text
agent -> https://platform:8443/v1/enrollment
agent -> https://platform:8443/v1/agent/commands?wait=30s
agent -> telemetry receiver / warp-parse
```

### 9.2 Compose

适合本地或单机生产化试运行。

```text
platform-ui
control-center
agent-gateway
warp-parse
postgres
object-store
```

`advertise_url` 仍由外部入口决定，不能使用 compose 内部 service name 作为 agent 默认地址，除非 agent 与平台在同一 Docker 网络内。

### 9.3 Kubernetes

适合后续规模化部署。

推荐：

- Ingress 或 LoadBalancer 提供 `advertise_url`
- Secret 保存 CA、签名 key、token seed
- Stateful database 保存控制面状态
- Agent DaemonSet 通过安装 URL 或 Helm values 获取 bootstrap 配置

---

## 10. 安全要求

第一版必须满足：

- 管理平台 HTTPS server certificate 可被 agent trust bundle 校验
- bootstrap token 有 TTL
- bootstrap token 有使用次数限制
- bootstrap token 可撤销
- enrollment 请求必须携带 CSR
- 平台签发 agent client certificate
- 注册后控制面 API 默认使用 mTLS
- agent private key 不离开本机
- 安装包有摘要或签名校验
- 所有 enrollment 成功、拒绝、重复注册和证书签发事件写入审计

不建议：

- 把长期 token 写入安装命令
- 让同一个 bootstrap token 无限注册
- 让 agent 接受未校验的自签 server certificate
- 让安装脚本自动扫描网段寻找平台

---

## 11. 与 Agent 发现机制的关系

第一版 agent 发现控制平台的优先级建议为：

1. 安装脚本写入的显式 `enrollment_url`
2. 本地已有 identity 中缓存的上次成功 `gateway_url`
3. DNS SRV / well-known 域名发现
4. Kubernetes / cloud metadata 注入
5. 显式开启的实验室扫描模式

默认实现只要求第 1 和第 2 项成立。

DNS、Kubernetes、cloud metadata 可以作为后续增强。

扫描模式不进入生产默认路径。

---

## 12. 第一版最小验收

管理平台容器 bootstrap 链路第一版验收标准：

1. 平台容器通过 `WPI_ADVERTISE_URL` 启动。
2. 平台生成带短期 token 的安装 URL。
3. 目标机器执行安装命令后写入 agent bootstrap 配置。
4. agent 首次启动生成 keypair 和 CSR。
5. agent 使用 token 调用 `/v1/enrollment`。
6. 平台签发 `agent_id`、`instance_id` 和 client certificate。
7. agent 原子持久化 identity。
8. agent 使用 mTLS 调用 `/v1/agent/hello`。
9. agent 可以 heartbeat、capability report 和 long polling。
10. enrollment 与 token 使用记录可以审计查询。

如果这条链路未打通，不应把管理平台容器宣称为可纳管 agent 的控制中心。

---

## 13. 当前决定

当前固定：

- 第一版采用管理平台容器生成安装 URL 的 bootstrap 模型
- agent 默认不做端口扫描
- Docker image 不携带固定管理地址
- 管理平台运行时必须配置或确认 `advertise_url`
- 安装 URL 只承担 bootstrap，不承担长期控制面认证
- `enrollment_token` 只用于首次注册
- 注册成功后的正式控制面通信使用 mTLS
- all-in-one 容器是第一版推荐物理部署形态，但控制面和数据面逻辑必须分离
