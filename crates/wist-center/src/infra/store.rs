// WarpInsightCenter 状态存储：Store trait + 双实现。
// - FileStore：JSON 文件快照（镜像 warp-gateway AdminStore），测试/无 PG 回退路径。
// - PgStore：PostgreSQL（开发期，见 pg_store.rs）。
// 持有每网关的凭证（sha256 hash）与最新上报状态（version/status/health/last_seen_at），
// 后续 GatewayRuntimeStatus / GatewayListView 从快照聚合读取。

use std::{
    collections::HashMap,
    fs,
    fs::OpenOptions,
    io,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock, Weak},
};

use insight_control::types::DateTime;
use insight_control::{GatewayInstanceLifecycleState, UpgradeStep, UpgradeTarget};
use orion_error::{conversion::ToStructError, prelude::*};
use serde::{Deserialize, Serialize};

use super::sha256_hex;
use crate::config::GatewayCredentialSeed;

pub use wist_error::{StoreError, StoreReason};

#[derive(Debug, Clone)]
pub struct FileStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CenterStoreSnapshot {
    /// key = gateway_id。
    pub gateways: HashMap<String, StoredGateway>,
    /// key = agent_id（Gateway 上报的其下 Agent 状态快照）。
    pub agents: HashMap<String, StoredAgent>,
    /// 网关生命周期转变历史（key = gateway_id，有界保留最近 ~100 条）。
    pub lifecycle_events: HashMap<String, Vec<LifecycleEvent>>,
    /// 版本发布记录（key = component，如 wist-agentd / warp-gateway）。
    pub releases: HashMap<String, Vec<ReleaseRecord>>,
    /// 升级计划（按创建顺序，新→旧）。
    pub upgrade_plans: Vec<UpgradePlanRecord>,
    /// 网关-客户绑定记录。
    pub customer_bindings: Vec<GatewayCustomerBindingRecord>,
    /// 网关注册 Token（key = token_id；映射模型 GatewayEnrollmentToken）。
    #[serde(default)]
    pub enrollment_tokens: HashMap<String, StoredEnrollmentToken>,
}

/// 签发注册 Token 的入参（映射模型 GatewayEnrollmentToken 的签发侧可变字段）。
#[derive(Debug, Clone)]
pub struct EnrollmentTokenIssue {
    /// 明文 token（仅用于计算 hash 落库，不持久化明文）。
    pub token: String,
    /// 签发人（管理面 requested_by）。
    pub issued_by: String,
    /// 控制中心信任根（control-center.pem 内容），随注册 token 下发；
    /// 未配置 CA → None。
    pub control_center_trust_bundle: Option<String>,
}

/// 网关注册 Token 记录（映射模型 GatewayEnrollmentToken）：服务端只存 hash，
/// 用 max_uses/used_count/status/expiry 控制"限量、防重放、可吊销"。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEnrollmentToken {
    pub token_id: String,
    pub token_hash: String,
    pub gateway_id: String,
    /// 环境绑定（模型字段；默认 "tenant-default"）。
    #[serde(default)]
    pub tenant_id: String,
    /// 环境绑定（模型字段；默认 "env-default"）。
    #[serde(default)]
    pub environment_id: String,
    /// 签发人（模型字段，admin 面的 requested_by）。
    #[serde(default)]
    pub issued_by: String,
    /// 随注册 token 下发的控制中心信任根（control-center.pem 内容）。
    #[serde(default)]
    pub control_center_trust_bundle: Option<String>,
    pub max_uses: i64,
    pub used_count: i64,
    pub status: String, // Active / Used / Exhausted / Revoked / Expired
    pub issued_at: DateTime,
    #[serde(default)]
    pub expires_at: Option<DateTime>,
    /// 吊销时间（status = Revoked 时记录）。
    #[serde(default)]
    pub revoked_at: Option<DateTime>,
}

/// 网关-客户绑定记录（映射模型 GatewayCustomerBinding）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayCustomerBindingRecord {
    pub gateway_id: String,
    pub customer_id: String,
    pub status: String,
    pub bound_at: DateTime,
}

/// 一次升级计划记录（映射模型 UpgradePlan）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradePlanRecord {
    pub plan_id: String,
    pub targets: Vec<UpgradeTarget>,
    pub target_count: i64,
    pub status: String,
    pub created_at: DateTime,
    pub steps: Vec<UpgradeStep>,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime>,
}

/// 一次版本发布记录（映射模型 WistAgentdRelease / WarpGateWayRelease）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRecord {
    pub version: String,
    pub artifact_url: String,
    pub status: String,
    pub published_at: DateTime,
}

/// 网关生命周期一次状态转变记录（过程历史，append-only）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub gateway_id: String,
    pub from_state: Option<GatewayInstanceLifecycleState>,
    pub to_state: GatewayInstanceLifecycleState,
    pub at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredGateway {
    pub gateway_id: String,
    pub instance_id: String,
    pub credential_token_hash: String,
    pub credential_status: StoredGatewayCredentialStatus,
    #[serde(default)]
    pub credential_expires_at: Option<String>,
    /// 一次性置备引导 Token 的 hash（BOOTSTRAP_TOKEN）。create 时签发，
    /// init-url 置备成功后清空（消费）；初始化后不可再生成。空串 = 无/已消费。
    #[serde(default)]
    pub bootstrap_token_hash: String,
    /// 最新上报状态（喂 GatewayRuntimeStatus，last_seen_at = reported_at）。
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub health: Option<String>,
    #[serde(default)]
    pub memory_bytes: Option<i64>,
    #[serde(default)]
    pub cpu_percent: Option<f64>,
    #[serde(default)]
    pub lifecycle_state: Option<GatewayInstanceLifecycleState>,
    #[serde(default)]
    pub initialized_at: Option<DateTime>,
    #[serde(default)]
    pub created_at: Option<DateTime>,
    #[serde(default)]
    pub last_seen_at: Option<DateTime>,
}

impl StoredGateway {
    pub fn provisioned(
        gateway_id: String,
        instance_id: String,
        credential_token_hash: String,
        expires_at: Option<String>,
    ) -> Self {
        Self {
            gateway_id,
            instance_id,
            credential_token_hash,
            credential_status: StoredGatewayCredentialStatus::Active,
            credential_expires_at: expires_at,
            bootstrap_token_hash: String::new(),
            version: None,
            status: None,
            health: None,
            memory_bytes: None,
            cpu_percent: None,
            lifecycle_state: None,
            initialized_at: None,
            created_at: None,
            last_seen_at: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredGatewayCredentialStatus {
    Active,
    Expired,
    Revoked,
}

/// Gateway 上报的其下 Agent 状态快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAgent {
    pub agent_id: String,
    pub gateway_id: String,
    pub instance_id: String,
    pub version: String,
    pub status: String,
    pub health: String,
    pub memory_bytes: Option<i64>,
    pub cpu_percent: Option<f64>,
    pub admin_latency_ms: Option<i64>,
    pub last_seen_at: DateTime,
}

/// 单次状态上报落库参数（ReceiveGatewayStatusReport → store）。
#[derive(Debug, Clone)]
pub struct GatewayStatusUpdate {
    pub gateway_id: String,
    pub instance_id: String,
    pub version: String,
    pub status: String,
    pub health: String,
    pub memory_bytes: Option<i64>,
    pub cpu_percent: Option<f64>,
    pub last_seen_at: DateTime,
}

/// 网关凭证与状态的持久化抽象：FileStore（JSON 文件，测试/无 PG 回退）与
/// PgStore（PostgreSQL，开发期）双实现。
/// 原生 async fn in trait 在当前工具链下不可 dyn（E0038），故用 `#[async_trait]`
/// （内部转为 `Pin<Box<dyn Future + Send>>`，trait 保持 `Send + Sync` 可 `Arc<dyn Store>`）。
#[async_trait::async_trait]
pub trait Store: Send + Sync + std::fmt::Debug {
    /// 用 seed 凭证补齐缺失的网关（已有条目不覆盖，保留已轮换凭证）。返回是否有新增。
    async fn seed(&self, seeds: &[GatewayCredentialSeed]) -> Result<bool, StoreError>;
    /// 全部网关（含已接入未上报的），按 gateway_id 排序。
    async fn list_gateways(&self) -> Result<Vec<StoredGateway>, StoreError>;
    async fn get_gateway(&self, gateway_id: &str) -> Result<Option<StoredGateway>, StoreError>;
    /// 落库最新上报状态（仅更新已接入网关，与 FileStore 的 update 语义一致）。
    async fn upsert_gateway_status(&self, update: &GatewayStatusUpdate) -> Result<(), StoreError>;
    /// 创建网关实例：gateway_id 已存在 → `Err(Conflict)`；否则以空 instance_id 接入。
    /// token 非空则存 `sha256(token)`，空 token → 空 hash（无凭证网关无法上报）。
    async fn create_gateway(
        &self,
        gateway_id: &str,
        token: &str,
    ) -> Result<StoredGateway, StoreError>;
    /// 为网关签发注册 Token（映射模型 GatewayEnrollmentToken）：持久化 hash、
    /// 限量（默认 max_uses=1）、状态 Active、有效期。注册/初始化时消费。
    async fn create_enrollment_token(
        &self,
        gateway_id: &str,
        issue: &EnrollmentTokenIssue,
    ) -> Result<StoredEnrollmentToken, StoreError>;
    /// 校验并消费一个注册 Token：按 hash 查找；状态/有效期/用量不合法 → Err；
    /// 合法则递增 used_count，达 max_uses → Exhausted；返回 token（含 gateway_id）。
    async fn consume_enrollment_token(
        &self,
        token: &str,
    ) -> Result<StoredEnrollmentToken, StoreError>;
    /// 校验并消费一次性置备引导 Token（BOOTSTRAP_TOKEN）：比对 hash + 未初始化，
    /// 成功则清空 bootstrap_token_hash。返回是否消费成功。
    async fn consume_bootstrap_token(
        &self,
        gateway_id: &str,
        bootstrap_token: &str,
    ) -> Result<bool, StoreError>;
    /// 落库运行期凭据（RUNTIME_TOKEN）：更新 credential_token_hash + Active + 过期时间。
    async fn update_gateway_credential(
        &self,
        gateway_id: &str,
        token_hash: &str,
        expires_at: Option<String>,
    ) -> Result<bool, StoreError>;
    /// 吊销注册 Token（映射模型 GatewayEnrollmentTokenStatus.Revoked）：
    /// 校验归属 gateway_id 后置 status=Revoked + revoked_at；未知 token / 归属不符 → Err。
    async fn revoke_enrollment_token(
        &self,
        gateway_id: &str,
        token_id: &str,
    ) -> Result<StoredEnrollmentToken, StoreError>;
    /// 查询某网关最近签发的一个注册 Token（初始配置的 enrollment_token_id 引用用）。
    async fn get_enrollment_token_for_gateway(
        &self,
        gateway_id: &str,
    ) -> Result<Option<StoredEnrollmentToken>, StoreError>;
    /// 落库 Gateway 上报的其下 Agent 状态（按 agent_id 幂等 upsert，记录归属 gateway_id）。
    async fn upsert_agent_status(
        &self,
        gateway_id: &str,
        agents: &[StoredAgent],
    ) -> Result<(), StoreError>;
    /// 生命周期：Provisioned → Initializing（Gateway 首次拉取初始配置时）。
    async fn mark_gateway_initializing(&self, gateway_id: &str) -> Result<(), StoreError>;
    /// 记录一次生命周期转变事件（append-only 过程历史）。
    async fn record_lifecycle_event(
        &self,
        gateway_id: &str,
        from: Option<GatewayInstanceLifecycleState>,
        to: GatewayInstanceLifecycleState,
    ) -> Result<(), StoreError>;
    /// 查询某网关的生命周期转变历史（按时间升序）。
    async fn list_lifecycle_events(
        &self,
        gateway_id: &str,
    ) -> Result<Vec<LifecycleEvent>, StoreError>;
    /// 记录一次版本发布（component = wist-agentd / warp-gateway），返回记录。
    async fn publish_release(
        &self,
        component: &str,
        version: &str,
        artifact_url: &str,
    ) -> Result<ReleaseRecord, StoreError>;
    /// 查询某组件的历史发布记录（新→旧）。
    async fn list_releases(&self, component: &str) -> Result<Vec<ReleaseRecord>, StoreError>;
    /// 创建升级计划（多目标 + 网关范围 + 多步执行），status=pending。
    async fn create_upgrade_plan(
        &self,
        plan: &UpgradePlanRecord,
    ) -> Result<UpgradePlanRecord, StoreError>;
    /// 查询全部升级计划（新→旧）。
    async fn list_upgrade_plans(&self) -> Result<Vec<UpgradePlanRecord>, StoreError>;
    /// 批准升级计划：status → approved，记录批准人/时间。
    async fn approve_upgrade_plan(
        &self,
        plan_id: &str,
        approved_by: &str,
    ) -> Result<UpgradePlanRecord, StoreError>;
    /// 绑定网关到客户（幂等：同网关重复绑定更新客户）。
    async fn bind_gateway_customer(
        &self,
        gateway_id: &str,
        customer_id: &str,
    ) -> Result<GatewayCustomerBindingRecord, StoreError>;
    /// 查询全部网关-客户绑定。
    async fn list_customer_bindings(&self)
        -> Result<Vec<GatewayCustomerBindingRecord>, StoreError>;
    /// 查询某 gateway 下的全部 Agent 状态（按 agent_id 排序）。
    async fn list_agents_by_gateway(
        &self,
        gateway_id: &str,
    ) -> Result<Vec<StoredAgent>, StoreError>;
}

impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let lock = shared_process_lock(&path);
        Self { path, lock }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<CenterStoreSnapshot, StoreError> {
        let _guard = self.lock()?;
        self.load_snapshot()
    }

    pub fn save(&self, snapshot: &CenterStoreSnapshot) -> Result<(), StoreError> {
        let _guard = self.lock()?;
        self.save_snapshot(snapshot)
    }

    pub fn update<T>(
        &self,
        mutate: impl FnOnce(&mut CenterStoreSnapshot) -> T,
    ) -> Result<T, StoreError> {
        let _guard = self.lock()?;
        let mut snapshot = self.load_snapshot()?;
        let output = mutate(&mut snapshot);
        self.save_snapshot(&snapshot)?;
        Ok(output)
    }

    /// 用 seed 凭证补齐缺失的网关（已有条目不覆盖，保留已轮换凭证）。返回是否有新增。
    pub fn seed(&self, seeds: &[GatewayCredentialSeed]) -> Result<bool, StoreError> {
        let seeds = seeds.to_vec();
        self.update(|snapshot| {
            let mut added = false;
            for seed in &seeds {
                if snapshot.gateways.contains_key(&seed.gateway_id) {
                    continue;
                }
                snapshot.gateways.insert(
                    seed.gateway_id.clone(),
                    StoredGateway::provisioned(
                        seed.gateway_id.clone(),
                        String::new(),
                        sha256_hex(&seed.token),
                        seed.expires_at.clone(),
                    ),
                );
                added = true;
            }
            added
        })
    }

    /// 创建网关实例（同步，供测试与 trait 委托）：gateway_id 已存在 → Conflict。
    /// `token` 为一次性置备引导 Token（BOOTSTRAP_TOKEN），只存 sha256；
    /// 运行期凭据（credential_token_hash）留空，注册后签发 RUNTIME_TOKEN 时再落库。
    pub fn create_gateway(
        &self,
        gateway_id: &str,
        token: &str,
    ) -> Result<StoredGateway, StoreError> {
        let stored = self.update(|snapshot| {
            if snapshot.gateways.contains_key(gateway_id) {
                return Err(StoreReason::Conflict
                    .to_err()
                    .with_detail(gateway_id.to_string()));
            }
            let bootstrap_hash = if token.is_empty() {
                String::new()
            } else {
                sha256_hex(token)
            };
            let mut stored = StoredGateway::provisioned(
                gateway_id.to_string(),
                String::new(),
                String::new(),
                None,
            );
            stored.bootstrap_token_hash = bootstrap_hash;
            stored.lifecycle_state = Some(GatewayInstanceLifecycleState::Provisioned);
            stored.created_at = Some(DateTime::now());
            snapshot
                .gateways
                .insert(gateway_id.to_string(), stored.clone());
            Ok(stored)
        })??;
        FileStore::record_lifecycle_event(
            self,
            gateway_id,
            None,
            GatewayInstanceLifecycleState::Provisioned,
        )?;
        Ok(stored)
    }

    /// 签发注册 Token（同步，供测试与 trait 委托）：默认 max_uses=1、30 天有效、
    /// 环境绑定 tenant-default/env-default，携带控制中心信任根。
    pub fn create_enrollment_token(
        &self,
        gateway_id: &str,
        issue: &EnrollmentTokenIssue,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        let token_hash = if issue.token.is_empty() {
            String::new()
        } else {
            sha256_hex(&issue.token)
        };
        self.update(|snapshot| {
            let now = DateTime::now();
            let serial = snapshot
                .enrollment_tokens
                .values()
                .filter(|t| t.gateway_id == gateway_id)
                .count() as i64
                + 1;
            let token = StoredEnrollmentToken {
                token_id: format!("enroll-{gateway_id}-{serial}"),
                token_hash,
                gateway_id: gateway_id.to_string(),
                tenant_id: "tenant-default".to_string(),
                environment_id: "env-default".to_string(),
                issued_by: issue.issued_by.clone(),
                control_center_trust_bundle: issue.control_center_trust_bundle.clone(),
                max_uses: 1,
                used_count: 0,
                status: "Active".to_string(),
                issued_at: now.clone(),
                expires_at: Some(DateTime::in_days(30)),
                revoked_at: None,
            };
            snapshot
                .enrollment_tokens
                .insert(token.token_id.clone(), token.clone());
            Ok(token)
        })?
    }

    /// 吊销注册 Token（同步，供测试与 trait 委托）：校验归属 gateway_id 后
    /// status → Revoked + revoked_at。已吊销再吊销视为幂等成功；未知 token / 归属不符 → Err。
    pub fn revoke_enrollment_token(
        &self,
        gateway_id: &str,
        token_id: &str,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        self.update(|snapshot| {
            let Some(token) = snapshot.enrollment_tokens.get_mut(token_id) else {
                return Err(StoreReason::Enrollment
                    .to_err()
                    .with_detail("token not found"));
            };
            if token.gateway_id != gateway_id {
                return Err(StoreReason::Enrollment
                    .to_err()
                    .with_detail("token gateway mismatch"));
            }
            token.status = "Revoked".to_string();
            token.revoked_at = Some(DateTime::now());
            Ok(token.clone())
        })?
    }

    /// 查询某网关最近签发的一个注册 Token（同步，供测试与 trait 委托）：
    /// 按 issued_at 取最新，供初始配置的 enrollment_token_id 引用。
    pub fn get_enrollment_token_for_gateway(
        &self,
        gateway_id: &str,
    ) -> Result<Option<StoredEnrollmentToken>, StoreError> {
        let snapshot = self.load()?;
        Ok(snapshot
            .enrollment_tokens
            .values()
            .filter(|token| token.gateway_id == gateway_id)
            .max_by(|left, right| left.issued_at.to_chrono().cmp(&right.issued_at.to_chrono()))
            .cloned())
    }

    /// 校验并消费注册 Token（同步）：按 hash 查找，校验状态/有效期/用量，
    /// 递增 used_count，达 max_uses → Exhausted；返回 token（含 gateway_id）。
    pub fn consume_enrollment_token(
        &self,
        token: &str,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        let token_hash = sha256_hex(token);
        self.update(|snapshot| {
            let found = snapshot
                .enrollment_tokens
                .values_mut()
                .find(|t| !t.token_hash.is_empty() && t.token_hash == token_hash)
                .ok_or_else(|| {
                    StoreReason::Enrollment
                        .to_err()
                        .with_detail("token not found")
                })?;
            if found.status != "Active" && found.status != "Used" {
                return Err(StoreReason::Enrollment
                    .to_err()
                    .with_detail(format!("token status {}", found.status)));
            }
            if found
                .expires_at
                .as_ref()
                .is_some_and(|exp| exp.to_chrono() < chrono::Utc::now())
            {
                found.status = "Expired".to_string();
                return Err(StoreReason::Enrollment
                    .to_err()
                    .with_detail("token expired"));
            }
            if found.used_count >= found.max_uses {
                found.status = "Exhausted".to_string();
                return Err(StoreReason::Enrollment
                    .to_err()
                    .with_detail("token exhausted"));
            }
            found.used_count += 1;
            found.status = if found.used_count >= found.max_uses {
                "Exhausted".to_string()
            } else {
                "Used".to_string()
            };
            Ok(found.clone())
        })?
    }

    /// 校验并消费一次性置备引导 Token（同步）：比对 `bootstrap_token_hash`，网关必须
    /// 处于 Provisioned（未初始化）且 token 未消费；成功则清空 bootstrap_token_hash（消费）。
    /// 返回是否消费成功（不匹配/已消费/已初始化 → false，由 handler 映射 401）。
    pub fn consume_bootstrap_token(
        &self,
        gateway_id: &str,
        bootstrap_token: &str,
    ) -> Result<bool, StoreError> {
        let bootstrap_hash = sha256_hex(bootstrap_token);
        self.update(|snapshot| {
            let Some(gateway) = snapshot.gateways.get_mut(gateway_id) else {
                return false;
            };
            if gateway.bootstrap_token_hash.is_empty()
                || gateway.bootstrap_token_hash != bootstrap_hash
                || gateway.lifecycle_state != Some(GatewayInstanceLifecycleState::Provisioned)
            {
                return false;
            }
            gateway.bootstrap_token_hash.clear();
            true
        })
    }

    /// 落库运行期凭据（RUNTIME_TOKEN，同步）：更新 credential_token_hash + Active + 过期时间。
    /// 网关注册/续期时调用；旧凭据 hash 被覆盖即失效。返回是否更新成功。
    pub fn update_gateway_credential(
        &self,
        gateway_id: &str,
        token_hash: &str,
        expires_at: Option<String>,
    ) -> Result<bool, StoreError> {
        self.update(|snapshot| {
            let Some(gateway) = snapshot.gateways.get_mut(gateway_id) else {
                return false;
            };
            gateway.credential_token_hash = token_hash.to_string();
            gateway.credential_status = StoredGatewayCredentialStatus::Active;
            gateway.credential_expires_at = expires_at;
            true
        })
    }

    /// 落库 Agent 状态（同步，供测试与 trait 委托）：按 agent_id upsert，归属以 gateway_id 参数为准。
    pub fn upsert_agent_status(
        &self,
        gateway_id: &str,
        agents: &[StoredAgent],
    ) -> Result<(), StoreError> {
        self.update(|snapshot| {
            for mut agent in agents.to_vec() {
                agent.gateway_id = gateway_id.to_string();
                snapshot.agents.insert(agent.agent_id.clone(), agent);
            }
        })
    }

    /// 生命周期：Provisioned → Initializing（幂等，Running 后不降级），并记录转变事件。
    pub fn mark_gateway_initializing(&self, gateway_id: &str) -> Result<(), StoreError> {
        let changed: bool = self.update(|snapshot| {
            if let Some(stored) = snapshot.gateways.get_mut(gateway_id) {
                if stored.lifecycle_state == Some(GatewayInstanceLifecycleState::Provisioned) {
                    stored.lifecycle_state = Some(GatewayInstanceLifecycleState::Initializing);
                    return true;
                }
            }
            false
        })?;
        if changed {
            FileStore::record_lifecycle_event(
                self,
                gateway_id,
                Some(GatewayInstanceLifecycleState::Provisioned),
                GatewayInstanceLifecycleState::Initializing,
            )?;
        }
        Ok(())
    }

    /// 记录一次生命周期转变事件（有界保留最近 ~100 条）。
    pub fn record_lifecycle_event(
        &self,
        gateway_id: &str,
        from: Option<GatewayInstanceLifecycleState>,
        to: GatewayInstanceLifecycleState,
    ) -> Result<(), StoreError> {
        self.update(|snapshot| {
            let events = snapshot
                .lifecycle_events
                .entry(gateway_id.to_string())
                .or_default();
            events.push(LifecycleEvent {
                gateway_id: gateway_id.to_string(),
                from_state: from,
                to_state: to,
                at: DateTime::now(),
            });
            const MAX_EVENTS: usize = 100;
            if events.len() > MAX_EVENTS {
                let excess = events.len() - MAX_EVENTS;
                events.drain(..excess);
            }
        })
    }

    pub fn list_lifecycle_events(
        &self,
        gateway_id: &str,
    ) -> Result<Vec<LifecycleEvent>, StoreError> {
        let snapshot = self.load()?;
        Ok(snapshot
            .lifecycle_events
            .get(gateway_id)
            .cloned()
            .unwrap_or_default())
    }

    /// 记录一次版本发布（同步，供测试与 trait 委托）。
    pub fn publish_release(
        &self,
        component: &str,
        version: &str,
        artifact_url: &str,
    ) -> Result<ReleaseRecord, StoreError> {
        let record = ReleaseRecord {
            version: version.to_string(),
            artifact_url: artifact_url.to_string(),
            status: "published".to_string(),
            published_at: DateTime::now(),
        };
        self.update(|snapshot| {
            snapshot
                .releases
                .entry(component.to_string())
                .or_default()
                .push(record.clone());
        })?;
        Ok(record)
    }

    pub fn list_releases(&self, component: &str) -> Result<Vec<ReleaseRecord>, StoreError> {
        let snapshot = self.load()?;
        let mut records = snapshot
            .releases
            .get(component)
            .cloned()
            .unwrap_or_default();
        records.sort_by(|left, right| {
            right
                .published_at
                .to_chrono()
                .cmp(&left.published_at.to_chrono())
        });
        Ok(records)
    }

    /// 创建升级计划（同步，供测试与 trait 委托）：status=pending，插入最前（新→旧）。
    pub fn create_upgrade_plan(
        &self,
        plan: &UpgradePlanRecord,
    ) -> Result<UpgradePlanRecord, StoreError> {
        self.update(|snapshot| {
            snapshot.upgrade_plans.insert(0, plan.clone());
        })?;
        Ok(plan.clone())
    }

    pub fn list_upgrade_plans(&self) -> Result<Vec<UpgradePlanRecord>, StoreError> {
        let snapshot = self.load()?;
        Ok(snapshot.upgrade_plans)
    }

    pub fn approve_upgrade_plan(
        &self,
        plan_id: &str,
        approved_by: &str,
    ) -> Result<UpgradePlanRecord, StoreError> {
        let plan = self.update(|snapshot| {
            let Some(plan) = snapshot
                .upgrade_plans
                .iter_mut()
                .find(|plan| plan.plan_id == plan_id)
            else {
                return Err(StoreReason::Conflict
                    .to_err()
                    .with_detail(plan_id.to_string()));
            };
            plan.status = "approved".to_string();
            plan.approved_by = Some(approved_by.to_string());
            plan.approved_at = Some(DateTime::now());
            Ok(plan.clone())
        })??;
        Ok(plan)
    }

    /// 绑定网关到客户（同步，供测试与 trait 委托）：同网关重复绑定更新客户。
    pub fn bind_gateway_customer(
        &self,
        gateway_id: &str,
        customer_id: &str,
    ) -> Result<GatewayCustomerBindingRecord, StoreError> {
        let binding =
            self.update::<Result<GatewayCustomerBindingRecord, StoreError>>(|snapshot| {
                if let Some(existing) = snapshot
                    .customer_bindings
                    .iter_mut()
                    .find(|binding| binding.gateway_id == gateway_id)
                {
                    existing.customer_id = customer_id.to_string();
                    existing.status = "bound".to_string();
                    existing.bound_at = DateTime::now();
                    return Ok(existing.clone());
                }
                let binding = GatewayCustomerBindingRecord {
                    gateway_id: gateway_id.to_string(),
                    customer_id: customer_id.to_string(),
                    status: "bound".to_string(),
                    bound_at: DateTime::now(),
                };
                snapshot.customer_bindings.push(binding.clone());
                Ok(binding)
            })??;
        Ok(binding)
    }

    pub fn list_customer_bindings(&self) -> Result<Vec<GatewayCustomerBindingRecord>, StoreError> {
        let snapshot = self.load()?;
        Ok(snapshot.customer_bindings)
    }

    pub fn list_agents_by_gateway(&self, gateway_id: &str) -> Result<Vec<StoredAgent>, StoreError> {
        let snapshot = self.load()?;
        let mut agents: Vec<StoredAgent> = snapshot
            .agents
            .into_values()
            .filter(|agent| agent.gateway_id == gateway_id)
            .collect();
        agents.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
        Ok(agents)
    }

    fn lock(&self) -> Result<StoreLockGuard<'_>, StoreError> {
        let process_guard = self.lock.lock().map_err(|_| {
            StoreReason::Io
                .to_err()
                .with_detail("center store lock poisoned")
        })?;
        let file_guard = FileLockGuard::lock(&lock_file_path(&self.path))?;
        Ok(StoreLockGuard {
            _process_guard: process_guard,
            _file_guard: file_guard,
        })
    }

    fn load_snapshot(&self) -> Result<CenterStoreSnapshot, StoreError> {
        if !self.path.exists() {
            return Ok(CenterStoreSnapshot::default());
        }
        let content = fs::read_to_string(&self.path).source_err(StoreReason::Io, "read store")?;
        if content.trim().is_empty() {
            return Ok(CenterStoreSnapshot::default());
        }
        Ok(serde_json::from_str(&content).source_err(StoreReason::Json, "parse store")?)
    }

    fn save_snapshot(&self, snapshot: &CenterStoreSnapshot) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).source_err(StoreReason::Io, "create store dir")?;
        }
        let content = serde_json::to_string_pretty(snapshot)
            .source_err(StoreReason::Json, "serialize store")?;
        let temp_path = temp_store_path(&self.path);
        let write_result = (|| -> Result<(), StoreError> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temp_path)
                .source_err(StoreReason::Io, "open temp store file")?;
            file.write_all(content.as_bytes())
                .source_err(StoreReason::Io, "write store")?;
            file.write_all(b"\n")
                .source_err(StoreReason::Io, "write store")?;
            file.sync_all().source_err(StoreReason::Io, "sync store")?;
            fs::rename(&temp_path, &self.path).source_err(StoreReason::Io, "rename store")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
                    .source_err(StoreReason::Io, "set store permissions")?;
            }
            if let Some(parent) = self.path.parent() {
                sync_directory(parent).source_err(StoreReason::Io, "sync store dir")?;
            }
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Store for FileStore {
    async fn seed(&self, seeds: &[GatewayCredentialSeed]) -> Result<bool, StoreError> {
        // 显式调用同步固有方法，避免与 trait 方法同名递归。
        FileStore::seed(self, seeds)
    }

    async fn list_gateways(&self) -> Result<Vec<StoredGateway>, StoreError> {
        let snapshot = self.load()?;
        let mut gateways: Vec<StoredGateway> = snapshot.gateways.into_values().collect();
        gateways.sort_by(|left, right| left.gateway_id.cmp(&right.gateway_id));
        Ok(gateways)
    }

    async fn get_gateway(&self, gateway_id: &str) -> Result<Option<StoredGateway>, StoreError> {
        let snapshot = self.load()?;
        Ok(snapshot.gateways.get(gateway_id).cloned())
    }

    async fn upsert_gateway_status(&self, update: &GatewayStatusUpdate) -> Result<(), StoreError> {
        let (changed, from): (bool, Option<GatewayInstanceLifecycleState>) =
            self.update(|snapshot| {
                if let Some(stored) = snapshot.gateways.get_mut(&update.gateway_id) {
                    stored.instance_id = update.instance_id.clone();
                    stored.version = Some(update.version.clone());
                    stored.status = Some(update.status.clone());
                    stored.health = Some(update.health.clone());
                    stored.memory_bytes = update.memory_bytes;
                    stored.cpu_percent = update.cpu_percent;
                    stored.last_seen_at = Some(update.last_seen_at.clone());
                    // 首次上报 → Running（初始化完成，记录 initialized_at + 转变事件）。
                    if stored.lifecycle_state != Some(GatewayInstanceLifecycleState::Running) {
                        let prev = stored.lifecycle_state;
                        stored.lifecycle_state = Some(GatewayInstanceLifecycleState::Running);
                        stored.initialized_at = Some(DateTime::now());
                        return (true, prev);
                    }
                }
                (false, None)
            })?;
        if changed {
            FileStore::record_lifecycle_event(
                self,
                &update.gateway_id,
                from,
                GatewayInstanceLifecycleState::Running,
            )?;
        }
        Ok(())
    }

    async fn create_gateway(
        &self,
        gateway_id: &str,
        token: &str,
    ) -> Result<StoredGateway, StoreError> {
        // 显式调用同步固有方法，避免与 trait 方法同名递归。
        FileStore::create_gateway(self, gateway_id, token)
    }

    async fn create_enrollment_token(
        &self,
        gateway_id: &str,
        issue: &EnrollmentTokenIssue,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        FileStore::create_enrollment_token(self, gateway_id, issue)
    }

    async fn consume_enrollment_token(
        &self,
        token: &str,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        FileStore::consume_enrollment_token(self, token)
    }

    async fn consume_bootstrap_token(
        &self,
        gateway_id: &str,
        bootstrap_token: &str,
    ) -> Result<bool, StoreError> {
        FileStore::consume_bootstrap_token(self, gateway_id, bootstrap_token)
    }

    async fn update_gateway_credential(
        &self,
        gateway_id: &str,
        token_hash: &str,
        expires_at: Option<String>,
    ) -> Result<bool, StoreError> {
        FileStore::update_gateway_credential(self, gateway_id, token_hash, expires_at)
    }

    async fn revoke_enrollment_token(
        &self,
        gateway_id: &str,
        token_id: &str,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        FileStore::revoke_enrollment_token(self, gateway_id, token_id)
    }

    async fn get_enrollment_token_for_gateway(
        &self,
        gateway_id: &str,
    ) -> Result<Option<StoredEnrollmentToken>, StoreError> {
        FileStore::get_enrollment_token_for_gateway(self, gateway_id)
    }

    async fn upsert_agent_status(
        &self,
        gateway_id: &str,
        agents: &[StoredAgent],
    ) -> Result<(), StoreError> {
        FileStore::upsert_agent_status(self, gateway_id, agents)
    }

    async fn list_agents_by_gateway(
        &self,
        gateway_id: &str,
    ) -> Result<Vec<StoredAgent>, StoreError> {
        FileStore::list_agents_by_gateway(self, gateway_id)
    }

    async fn mark_gateway_initializing(&self, gateway_id: &str) -> Result<(), StoreError> {
        FileStore::mark_gateway_initializing(self, gateway_id)
    }

    async fn record_lifecycle_event(
        &self,
        gateway_id: &str,
        from: Option<GatewayInstanceLifecycleState>,
        to: GatewayInstanceLifecycleState,
    ) -> Result<(), StoreError> {
        FileStore::record_lifecycle_event(self, gateway_id, from, to)
    }

    async fn list_lifecycle_events(
        &self,
        gateway_id: &str,
    ) -> Result<Vec<LifecycleEvent>, StoreError> {
        FileStore::list_lifecycle_events(self, gateway_id)
    }

    async fn publish_release(
        &self,
        component: &str,
        version: &str,
        artifact_url: &str,
    ) -> Result<ReleaseRecord, StoreError> {
        FileStore::publish_release(self, component, version, artifact_url)
    }

    async fn list_releases(&self, component: &str) -> Result<Vec<ReleaseRecord>, StoreError> {
        FileStore::list_releases(self, component)
    }

    async fn create_upgrade_plan(
        &self,
        plan: &UpgradePlanRecord,
    ) -> Result<UpgradePlanRecord, StoreError> {
        FileStore::create_upgrade_plan(self, plan)
    }

    async fn list_upgrade_plans(&self) -> Result<Vec<UpgradePlanRecord>, StoreError> {
        FileStore::list_upgrade_plans(self)
    }

    async fn approve_upgrade_plan(
        &self,
        plan_id: &str,
        approved_by: &str,
    ) -> Result<UpgradePlanRecord, StoreError> {
        FileStore::approve_upgrade_plan(self, plan_id, approved_by)
    }

    async fn bind_gateway_customer(
        &self,
        gateway_id: &str,
        customer_id: &str,
    ) -> Result<GatewayCustomerBindingRecord, StoreError> {
        FileStore::bind_gateway_customer(self, gateway_id, customer_id)
    }

    async fn list_customer_bindings(
        &self,
    ) -> Result<Vec<GatewayCustomerBindingRecord>, StoreError> {
        FileStore::list_customer_bindings(self)
    }
}

struct StoreLockGuard<'a> {
    _process_guard: MutexGuard<'a, ()>,
    _file_guard: FileLockGuard,
}

static PROCESS_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();

fn shared_process_lock(path: &Path) -> Arc<Mutex<()>> {
    let key = normalized_store_path(path);
    let locks = PROCESS_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks
        .lock()
        .expect("center store process lock map poisoned");
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    lock
}

fn normalized_store_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn lock_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("center-store.json");
    path.with_file_name(format!(".{file_name}.lock"))
}

fn temp_store_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("center-store.json");
    let nanos = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_micros() * 1_000);
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nanos))
}

#[cfg(unix)]
struct FileLockGuard {
    file: fs::File,
}

#[cfg(unix)]
impl FileLockGuard {
    fn lock(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).source_err(StoreReason::Io, "create store dir")?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .source_err(StoreReason::Io, "open lock file")?;
        lock_file_exclusive(&file).source_err(StoreReason::Io, "lock store file")?;
        Ok(Self { file })
    }
}

#[cfg(unix)]
impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let _ = unlock_file(&self.file);
    }
}

#[cfg(not(unix))]
struct FileLockGuard;

#[cfg(not(unix))]
impl FileLockGuard {
    fn lock(_path: &Path) -> Result<Self, StoreError> {
        Ok(Self)
    }
}

#[cfg(unix)]
fn lock_file_exclusive(file: &fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    const LOCK_EX: std::os::raw::c_int = 2;
    if unsafe { flock(file.as_raw_fd(), LOCK_EX) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn unlock_file(file: &fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    const LOCK_UN: std::os::raw::c_int = 8;
    if unsafe { flock(file.as_raw_fd(), LOCK_UN) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        OpenOptions::new().read(true).open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn seed_provisions_missing_gateways_only() {
        let path = test_store_path();
        let store = FileStore::new(&path);
        let seeds = vec![
            GatewayCredentialSeed {
                gateway_id: "gw-001".to_string(),
                token: "tok-a".to_string(),
                expires_at: None,
            },
            GatewayCredentialSeed {
                gateway_id: "gw-002".to_string(),
                token: "tok-b".to_string(),
                expires_at: None,
            },
        ];
        assert!(store.seed(&seeds).expect("seed"));
        assert!(store.seed(&seeds).expect("seed again") == false);

        let snapshot = store.load().expect("load");
        assert_eq!(snapshot.gateways.len(), 2);
        let first = &snapshot.gateways["gw-001"];
        assert_eq!(first.credential_token_hash, sha256_hex("tok-a"));
        assert_eq!(
            first.credential_status,
            StoredGatewayCredentialStatus::Active
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn update_upserts_latest_status() {
        let path = test_store_path();
        let store = FileStore::new(&path);
        let seeds = vec![GatewayCredentialSeed {
            gateway_id: "gw-001".to_string(),
            token: "tok-a".to_string(),
            expires_at: None,
        }];
        store.seed(&seeds).expect("seed");

        store
            .update(|snapshot| {
                if let Some(stored) = snapshot.gateways.get_mut("gw-001") {
                    stored.version = Some("v2.4.1".to_string());
                    stored.status = Some("online".to_string());
                    stored.health = Some("healthy".to_string());
                    stored.last_seen_at = Some(DateTime::now());
                }
            })
            .expect("update");

        let snapshot = store.load().expect("load");
        let stored = &snapshot.gateways["gw-001"];
        assert_eq!(stored.version.as_deref(), Some("v2.4.1"));
        assert_eq!(stored.status.as_deref(), Some("online"));
        assert!(stored.last_seen_at.is_some());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn create_gateway_provisions_and_conflicts_on_duplicate() {
        let path = test_store_path();
        let store = FileStore::new(&path);

        let stored = store.create_gateway("gw-100", "tok-x").expect("create");
        assert_eq!(stored.gateway_id, "gw-100");
        // create 的 token 是置备引导 Token（BOOTSTRAP_TOKEN）：存 bootstrap hash，
        // 运行期凭据（credential_token_hash）留空，注册后签发 RUNTIME_TOKEN 时再落库。
        assert_eq!(stored.bootstrap_token_hash, sha256_hex("tok-x"));
        assert_eq!(stored.credential_token_hash, "");
        assert_eq!(
            stored.credential_status,
            StoredGatewayCredentialStatus::Active
        );
        assert_eq!(stored.instance_id, "");

        // 重复创建同一 gateway_id → Conflict。
        let err = store
            .create_gateway("gw-100", "tok-y")
            .expect_err("conflict");
        assert_eq!(err.reason(), &StoreReason::Conflict);
        assert_eq!(err.detail().as_deref(), Some("gw-100"));

        // 空 token → bootstrap hash 也空（无引导凭据网关无法置备）。
        let stored = store
            .create_gateway("gw-nocred", "")
            .expect("create no credential");
        assert_eq!(stored.bootstrap_token_hash, "");
        assert_eq!(stored.credential_token_hash, "");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn bootstrap_token_consume_and_rotate() {
        // 消费：匹配 bootstrap + Provisioned → true，hash 清空。
        let path = test_store_path();
        let store = FileStore::new(&path);
        store.create_gateway("gw-b", "boot-x").expect("create");
        assert!(store
            .consume_bootstrap_token("gw-b", "boot-x")
            .expect("consume"));
        assert!(store.load().expect("load").gateways["gw-b"]
            .bootstrap_token_hash
            .is_empty());
        // 已消费 → false（防重放）。
        assert!(!store
            .consume_bootstrap_token("gw-b", "boot-x")
            .expect("re-consume"));
        // 错 token → false。
        let path2 = test_store_path();
        let store2 = FileStore::new(&path2);
        store2.create_gateway("gw-b", "boot-x").expect("create");
        assert!(!store2
            .consume_bootstrap_token("gw-b", "wrong")
            .expect("wrong"));
        // 已初始化 → false。
        let path3 = test_store_path();
        let store3 = FileStore::new(&path3);
        store3.create_gateway("gw-b", "boot-x").expect("create");
        store3.mark_gateway_initializing("gw-b").expect("init");
        assert!(!store3
            .consume_bootstrap_token("gw-b", "boot-x")
            .expect("initialized"));

        for path in [path, path2, path3] {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn update_gateway_credential_replaces_runtime_credential() {
        let path = test_store_path();
        let store = FileStore::new(&path);
        store.create_gateway("gw-c", "boot-x").expect("create");
        let expires = Some("2026-09-01T00:00:00Z".to_string());
        assert!(store
            .update_gateway_credential("gw-c", "sha256:runtime", expires.clone())
            .expect("update"));
        let snapshot = store.load().expect("load");
        let stored = &snapshot.gateways["gw-c"];
        assert_eq!(stored.credential_token_hash, "sha256:runtime");
        assert_eq!(
            stored.credential_status,
            StoredGatewayCredentialStatus::Active
        );
        assert_eq!(stored.credential_expires_at, expires);
        // 未知网关 → false。
        assert!(!store
            .update_gateway_credential("gw-nope", "h", None)
            .expect("unknown"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn enrollment_token_single_use_anti_replay_and_revoke() {
        let path = test_store_path();
        let store = FileStore::new(&path);

        // 签发：携带环境绑定 + 签发人 + 信任根。
        let issue = EnrollmentTokenIssue {
            token: "enroll-tok-1".to_string(),
            issued_by: "admin-test".to_string(),
            control_center_trust_bundle: Some(
                "-----BEGIN CERTIFICATE-----\nca\n-----END CERTIFICATE-----".to_string(),
            ),
        };
        let token = store
            .create_enrollment_token("gw-001", &issue)
            .expect("create");
        assert_eq!(token.status, "Active");
        assert_eq!(token.max_uses, 1);
        assert_eq!(token.used_count, 0);
        assert_eq!(token.tenant_id, "tenant-default");
        assert_eq!(token.environment_id, "env-default");
        assert_eq!(token.issued_by, "admin-test");
        assert!(token.control_center_trust_bundle.is_some());
        assert!(token.expires_at.is_some());
        assert!(token.revoked_at.is_none());

        // 首次消费成功 → Exhausted（max_uses=1）。
        let consumed = store
            .consume_enrollment_token("enroll-tok-1")
            .expect("consume");
        assert_eq!(consumed.status, "Exhausted");
        assert_eq!(consumed.used_count, 1);

        // 防重放：第二次消费被拒（Exhausted 状态在状态检查处被拒收）。
        let err = store
            .consume_enrollment_token("enroll-tok-1")
            .expect_err("anti-replay");
        assert_eq!(err.reason(), &StoreReason::Enrollment);

        // 吊销：Revoked 后消费被拒；归属不符拒绝吊销。
        let token2 = store
            .create_enrollment_token(
                "gw-001",
                &EnrollmentTokenIssue {
                    token: "enroll-tok-2".to_string(),
                    issued_by: "admin-test".to_string(),
                    control_center_trust_bundle: None,
                },
            )
            .expect("create2");
        let revoked = store
            .revoke_enrollment_token("gw-001", &token2.token_id)
            .expect("revoke");
        assert_eq!(revoked.status, "Revoked");
        assert!(revoked.revoked_at.is_some());
        let err = store
            .consume_enrollment_token("enroll-tok-2")
            .expect_err("revoked reject");
        assert_eq!(err.reason(), &StoreReason::Enrollment);
        assert!(err
            .detail()
            .as_deref()
            .is_some_and(|reason| reason.starts_with("token status")));
        let err = store
            .revoke_enrollment_token("gw-999", &token2.token_id)
            .expect_err("gateway mismatch");
        assert_eq!(err.reason(), &StoreReason::Enrollment);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn get_enrollment_token_for_gateway_returns_latest_for_gateway() {
        let path = test_store_path();
        let store = FileStore::new(&path);
        let issue = |token: &str| EnrollmentTokenIssue {
            token: token.to_string(),
            issued_by: "test".to_string(),
            control_center_trust_bundle: None,
        };
        store
            .create_enrollment_token("gw-001", &issue("tok-1"))
            .expect("t1");
        store
            .create_enrollment_token("gw-001", &issue("tok-2"))
            .expect("t2");
        store
            .create_enrollment_token("gw-002", &issue("tok-3"))
            .expect("t3");

        // 同网关取最新签发；跨网关互不可见。
        let latest = store
            .get_enrollment_token_for_gateway("gw-001")
            .expect("lookup")
            .expect("some");
        assert_eq!(latest.token_id, "enroll-gw-001-2");
        assert!(store
            .get_enrollment_token_for_gateway("gw-999")
            .expect("lookup")
            .is_none());

        let _ = fs::remove_file(path);
    }

    fn test_store_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("wist-center-store-test-{nanos}.json"))
    }
}
