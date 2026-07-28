# Agent 身份与通讯安全设计记录

本文记录 Control 域中 WpAgent 合法身份、注册 token、HTTPS 证书和后续通讯凭据的设计方向。核心语义已映射到 `.mju` 模型；具体 PKI 后端和证书轮换细节仍作为后续实现选择。

## 设计结论

- 私有网络不等于可信网络，Agent 连接管理端时不应关闭 TLS 证书校验。
- 注册阶段使用 HTTPS 加私有 CA 信任根，Agent 通过 `trust_bundle` 验证管理端证书。
- `enrollment_token` 只证明“允许注册”，不是 Agent 的长期身份。
- 注册成功后生成 `AgentIdentity`，并返回或签发 `AgentCredentialBundle` 作为后续通讯凭据。
- 后续状态上报、指令轮询、任务结果提交应校验 `AgentIdentity` 和通讯凭据，不再依赖 `enrollment_token`。
- IP 地址不是可信的 network zone 结论。Agent 可以上报本地网络事实，但 zone 归属应由管理端结合环境配置、资产登记、子网映射或人工规则判定。
- Agent 只能获得自身 scope 内的局部网格视图，不应默认获得全局资产图、全局服务依赖图或其它 Agent 的敏感信息。

## 安装、注册、控制攻击面

本文只记录防御视角的威胁建模，不记录可操作攻击步骤。

### 安装阶段

主要风险：

- 安装代码被替换，管理员复制到的 `install.sh | bash` 被页面、DNS、代理或中间人篡改。
- 安装 token 泄露，安装命令中的 token 进入 shell history、日志、工单或截图。
- 安装脚本下载的 Agent 二进制、配置模板或信任材料被替换。
- Agent 首次连接时信任了错误的管理端证书或错误的 CA。
- 旧安装命令被重放，用于注册非预期 Agent。

设计约束：

- 安装脚本和 Agent 二进制必须通过 HTTPS 获取，并校验 checksum 或签名。
- 安装代码短期有效，推荐一次性或限次使用。
- 安装代码只携带注册资格和 bootstrap 信息，不携带长期身份凭据。
- `trust_bundle` 必须随 bootstrap 信息下发或由可信渠道预置，Agent 不应依赖“首次连接到谁就信任谁”。
- 安装 token 泄露后只能造成注册资格风险，不能直接获得后续控制通道权限。

### 注册阶段

主要风险：

- 攻击者拿到 `enrollment_token` 后注册假 Agent。
- 同一 token 被重放，多次注册或挤占真实 Agent 身份。
- 攻击者伪造 `host_profile`，把自己注册成重要节点或错误环境中的节点。
- 注册返回的 `agent_id`、初始配置、策略绑定或管理端 endpoint 被篡改。
- 注册接口被批量调用，消耗 token、污染登记簿或压垮控制面。

设计约束：

- `AgentEnrollmentToken` 必须表达 `token_hash`、租户和环境绑定、选择器、过期时间、使用次数、撤销状态。
- 注册流程必须产生 `AgentEnrollmentTokenValidation`，把 token 校验结果作为独立事实记录。
- 注册成功后创建 `AgentIdentity` 和 `AgentCredentialBundle`，后续通讯转为正式身份认证。
- `host_profile` 只能作为注册辅助证据，不能单独决定 network zone、权限或全局身份归属。
- 重复注册先以事件和处理结果表达，不提前引入过重的策略结构。
- 注册接口需要限流、审计、幂等和防重放约束。

### 控制阶段

主要风险：

- 非法客户端伪装成 Agent 拉取控制指令。
- 攻击者伪造任务执行结果，让管理端误判指令已成功。
- 控制指令被重放，导致暂停、升级等动作重复执行。
- 控制指令被篡改，改变目标版本、参数或执行目标。
- 管理端用户权限过宽，普通用户发起高危控制动作。
- Agent 获得超过自身 scope 的全局网格、其它 Agent 信息或敏感配置。
- 被攻陷的 Agent 通过控制通道横向扩大影响范围。

设计约束：

- 指令轮询、状态提交、任务结果提交必须校验 `AgentIdentity` 和 `AgentCredentialBundle`。
- 控制消息需要稳定的 `message_id`、`sequence`、签发时间、过期时间和完整性保护。
- Agent 上报执行结果必须绑定原始 `command_id` 或 `dispatch_id`。
- 控制指令应有明确生命周期，例如 created、delivered、running、succeeded、failed、expired、cancelled。
- 暂停、远程升级、身份轮换等高危动作需要管理员权限和策略约束。
- 下发给 Agent 的网格信息应建模为局部 `AssignedMeshView`，不得直接暴露全局 mesh。
- Agent 本地发现的 IP、路由、端口和连接只能作为事实上报，由管理端完成归属、授权和可见范围计算。

## 注册阶段

安装代码或引导包提供：

- `control_endpoint`：管理端 HTTPS 地址。
- `trust_bundle`：内部 CA 根证书或等价信任材料。
- `tenant_id` / `environment_id`：注册目标环境。

注册资格由 `AgentEnrollmentToken` 单独表达。安装入口可以通过受控参数或管理端分配关系引用 token，但 `AgentBootstrapBundle` 本身不承担注册资格职责。

Agent 首次启动时提交：

- `enrollment_token`
- `credential_request`
- `host_profile`
- `capability_summary`

管理端校验：

- token hash 匹配。
- token 状态可用。
- token 未过期。
- `used_count < max_uses`。
- tenant/environment 匹配。
- `host_profile` 满足允许的宿主机选择器。
- 没有违反重复注册策略。

校验通过后创建：

- `AgentIdentity`
- `AgentInitialConfig`
- `AgentPolicyBinding`
- `AgentEnrollmentResult`

## Token 合法性

`AgentEnrollmentToken` 应按注册资格凭证处理：

- 服务端只保存 `token_hash`，明文 token 只在创建或安装代码中短暂出现。
- token 绑定 `tenant_id`、`environment_id` 和宿主机选择器。
- token 有 `issued_by`、`issued_at`、`expires_at`、`revoked_at`、`status`。
- token 受 `max_uses` 和 `used_count` 限制。
- 单机 token 注册成功后可置为 used。
- 批量 token 注册成功后递增 `used_count`，达到上限后置为 exhausted。

## HTTPS 与私有证书

管理端必须提供服务端证书。证书可以由私有 CA 签发，但必须满足：

- Agent 信任该私有 CA。
- 证书 SAN 包含 Agent 实际访问的内网 DNS 或 IP。
- Agent 不使用 `-k` 或跳过证书校验。

推荐关系：

```text
Internal CA
  -> signs WarpInsightAdmin server certificate
  -> optionally signs Agent client certificate after enrollment
```

注册阶段只要求 Agent 能校验管理端证书。mTLS 客户端证书不是注册前置条件，因为注册前 Agent 还没有已签发身份。

## 注册后通讯

注册成功后：

- `AgentIdentity` 是稳定身份锚点。
- `AgentCredentialBundle` 是后续通讯凭据。
- `AgentEnrollmentToken` 不再用于常规通讯认证。

后续通讯可以分阶段实现：

- 简化阶段：HTTPS 加 Agent credential token。
- 强安全阶段：HTTPS 加 mTLS 客户端证书。

## 暂不固化的内容

以下属于部署或后续实现选择，当前不进入核心模型：

- 具体 CA 后端：openssl、step-ca、Vault PKI、cert-manager、企业 CA。
- 证书轮换协议细节。
- 全量 mTLS 强制策略。
- 证书吊销列表或 OCSP 细节。

当确定生产 PKI 后端或要求所有 Agent API 强制 mTLS 时，再进一步细化证书签发、轮换、吊销和 mTLS 强制策略。
