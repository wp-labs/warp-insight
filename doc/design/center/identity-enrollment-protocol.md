# warp-insight Agent Identity 与 Enrollment 协议设计

## 1. 文档目的

本文档记录 `warp-insightd` 接入控制平台的身份与自动注册设计决定。

重点回答：

- `warp-insightd` 首次启动时如何自动注册
- enrollment 阶段是否需要 mTLS
- 注册成功后的控制面通信如何认证
- `agent_id`、`instance_id`、`node_id`、`session_id` 如何区分
- 为什么第一版选择 `HTTPS long polling + mTLS`

相关文档：

- [`agent-gateway-protocol.md`](agent-gateway-protocol.md)
- [`control-plane.md`](control-plane.md)
- [`control-center-architecture.md`](control-center-architecture.md)
- [`management-platform-bootstrap.md`](management-platform-bootstrap.md)
- [`../foundation/security-model.md`](../foundation/security-model.md)

---

## 2. 设计决定

第一版固定以下决定：

- 首次 enrollment 使用 `HTTPS server TLS + enrollment_token`
- enrollment 阶段不要求 mTLS
- enrollment 成功后，控制平台签发 agent client certificate
- 注册后的控制面通信使用 `HTTPS long polling + mTLS`
- WebSocket over mTLS 和 gRPC bidirectional stream 只作为后续可选 transport
- 控制平台不反连 agent，agent 只建立出站连接
- agent 默认不通过端口扫描发现控制平台
- 第一版推荐由管理平台容器生成安装 URL，并通过安装脚本写入 `enrollment_url`、`gateway_url` 和 trust bundle

一句话说：

`enrollment_token` 只用于首次注册，mTLS 用于注册后的正式控制链路。

---

## 3. 为什么 enrollment 不使用 mTLS

首次启动时，`warp-insightd` 还没有正式 agent identity，也没有控制平台签发的 client certificate。

因此 enrollment 阶段只能依赖：

- agent 校验控制平台 server TLS 证书
- bootstrap bundle 中的 trust bundle
- 有有效期和使用次数限制的 `enrollment_token`
- agent 本地生成 keypair 和 CSR

enrollment 请求成功后，控制平台才签发正式 agent client certificate。

---

## 4. 为什么控制面使用 mTLS

注册后的控制面会承载：

- heartbeat
- capability report
- action plan dispatch
- action plan ack
- action result report
- policy refresh hint
- identity rotation hint
- upgrade plan dispatch

这些消息属于控制面，不是普通配置读取。

使用 mTLS 的目的不是单纯加密，而是把 agent 身份绑定到：

- `agent_id`
- `instance_id`
- client certificate
- agent 本地 private key

控制平台据此可以实现：

- instance 级身份追踪
- 证书续期
- 证书吊销
- clone / duplicate registration 风险识别
- 高风险 action 的可信审计链

---

## 5. 为什么第一版选择 HTTPS long polling

第一版默认 transport 是：

`HTTPS long polling + mTLS`

原因：

- HTTPS 在企业网络、防火墙、代理和 LB 中最容易部署
- agent 只需要出站连接，不暴露入站端口
- 控制面消息低频、小包、强状态，不需要高吞吐流式协议
- long polling 足以支持中心向 agent 下发计划
- JSON envelope 便于联调、抓包、回放和现场排障
- 不把逻辑协议过早绑定到 gRPC 或 WebSocket

后续如果需要更低延迟下发，可以增加 WebSocket transport。

后续如果需要强 IDL、跨语言 SDK 或服务化 stream 治理，可以增加 gRPC transport。

---

## 6. 自动注册流程

推荐流程：

```text
management platform install URL
  -> install/bootstrap
  -> write enrollment_url / gateway_url / trust_bundle / token
  -> warp-insightd first start
  -> generate local keypair and CSR
  -> POST /v1/enrollment
  -> receive agent identity and client certificate
  -> persist identity atomically
  -> POST /v1/agent/hello with mTLS
  -> start heartbeat and long polling
```

enrollment 请求至少包含：

- `enrollment_token`
- CSR
- node facts
- capability summary
- requested time

enrollment 响应至少包含：

- `agent_id`
- `instance_id`
- issued client certificate
- CA bundle
- gateway endpoint
- initial config snapshot
- initial policy snapshot

---

## 7. 管理平台容器 Bootstrap

第一版推荐的 agent 接入入口不是端口扫描，而是管理平台容器生成的安装 URL。

典型流程：

```text
operator runs warp-insight-platform container
  -> platform uses WPI_ADVERTISE_URL
  -> platform generates install URL with short-lived token
  -> target machine runs install command
  -> installer writes bootstrap config
  -> agent performs enrollment
```

管理平台容器启动示例：

```bash
docker run -d \
  --name warp-insight-platform \
  -p 8443:8443 \
  -v wpi-data:/var/lib/warp-insight \
  -e WPI_ADVERTISE_URL=https://10.0.0.25:8443 \
  -e WPI_BOOTSTRAP_TOKEN_TTL=30m \
  warp-insight-platform:latest
```

安装后 agent bootstrap 配置示例：

```toml
[control_plane]
mode = "managed"
enrollment_url = "https://10.0.0.25:8443/v1/enrollment"
gateway_url = "https://10.0.0.25:8443"
trust_bundle = "/etc/warpinsight/ca.pem"
enrollment_token_file = "/etc/warpinsight/enrollment.token"
```

约束：

- Docker image 不应写死管理平台地址
- 管理平台运行时必须配置或确认 `advertise_url`
- agent 不应使用容器内部 IP 作为默认控制面地址
- 安装 URL 只用于 bootstrap，不作为长期控制面凭证
- 端口扫描只允许作为显式开启的实验室模式，生产默认关闭

详细设计见：

- [`management-platform-bootstrap.md`](management-platform-bootstrap.md)

---

## 8. 身份对象

第一版必须区分以下对象：

- `agent_id`
  稳定逻辑 agent 身份。重启和升级不改变。

- `instance_id`
  当前安装实例或身份实例。重装、身份重置或证书重签时可以变化。

- `node_id`
  目标节点的稳定环境身份。可由 machine-id、云主机 instance ID、Kubernetes node UID 等归一化得到。

- `boot_id`
  每次 `warp-insightd` 进程启动生成，用于区分重启前后的会话。

- `session_id`
  Gateway 为当前控制会话生成，只表示本次在线会话。

禁止把 `session_id` 当作 agent 身份使用。

---

## 9. 控制面 API 语义

第一版建议的 HTTPS API 语义：

```text
POST /v1/enrollment
POST /v1/agent/hello
POST /v1/agent/heartbeat
POST /v1/agent/capabilities
GET  /v1/agent/commands?wait=30s
POST /v1/dispatch/{dispatch_id}/ack
POST /v1/executions/{execution_id}/result
POST /v1/agent/credential/renew
```

其中：

- `/v1/enrollment` 使用 `HTTPS server TLS + enrollment_token`
- 其他控制面 API 使用 `HTTPS + mTLS`
- `/v1/agent/commands?wait=30s` 是 long polling 下发通道

---

## 10. 重复注册处理

第一版建议保守处理：

- 本地已有 identity 时，不再执行 enrollment，直接使用 mTLS 连接 Gateway
- 证书快过期时，使用 mTLS 调用 renew API
- 本地 identity 丢失时，必须重新 enrollment
- 同一 `node_id` 重复注册时，新实例进入 `pending` 或触发 `supersede` 决策
- 同一 enrollment token 超过使用次数或过期后必须拒绝

所有重复注册、拒绝、吊销和替换都必须写入控制面审计。

---

## 11. 当前决定

当前固定：

- 第一版不选 gRPC 作为默认南向协议
- 第一版不选 WebSocket 作为默认南向协议
- 第一版控制面默认使用 `HTTPS long polling + mTLS`
- 首次 enrollment 不使用 mTLS，使用 `HTTPS server TLS + enrollment_token`
- 注册成功后所有正式控制面 API 默认要求 mTLS
- 第一版推荐通过管理平台容器安装 URL 写入 agent bootstrap 配置
- agent 默认不通过端口扫描寻找控制平台
