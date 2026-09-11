// WarpInsightCenter 状态存储：PostgreSQL 实现（开发期）。
// 连接 compose 提供的 postgres:16（127.0.0.1:55432，demo/demo，库 insight_demo），
// schema 由 docker/initdb/01_schema.sql 在首次启动时建表。

use insight_control::types::DateTime;
use insight_control::GatewayInstanceLifecycleState;
use orion_error::{conversion::ToStructError, prelude::*};
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool, Row};
use wist_error::{StoreError, StoreReason};

use crate::config::GatewayCredentialSeed;

use super::{
    sha256_hex, EnrollmentTokenIssue, GatewayCustomerBindingRecord, GatewayStatusUpdate,
    LifecycleEvent, ReleaseRecord, Store, StoredAgent, StoredEnrollmentToken, StoredGateway,
    StoredGatewayCredentialStatus, UpgradePlanRecord,
};

#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub async fn connect(database_url: &str) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .source_raw_err(StoreReason::Sql, "connect postgres")?;
        Ok(Self { pool })
    }
}

/// gateways 表行（与 docker/initdb/01_schema.sql 列一一对应）。
/// 时间列直接读为 chrono（sqlx postgres+chrono 特性映射 TIMESTAMPTZ）。
#[derive(Debug, FromRow)]
struct GatewayRow {
    gateway_id: String,
    instance_id: String,
    credential_token_hash: String,
    credential_status: String,
    credential_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    bootstrap_token_hash: String,
    version: Option<String>,
    status: Option<String>,
    health: Option<String>,
    memory_bytes: Option<i64>,
    cpu_percent: Option<f64>,
    lifecycle_state: Option<String>,
    initialized_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn parse_lifecycle(value: &str) -> GatewayInstanceLifecycleState {
    match value {
        "Initializing" => GatewayInstanceLifecycleState::Initializing,
        "Running" => GatewayInstanceLifecycleState::Running,
        "Failed" => GatewayInstanceLifecycleState::Failed,
        _ => GatewayInstanceLifecycleState::Provisioned,
    }
}

fn lifecycle_name(state: GatewayInstanceLifecycleState) -> &'static str {
    match state {
        GatewayInstanceLifecycleState::Provisioned => "Provisioned",
        GatewayInstanceLifecycleState::Initializing => "Initializing",
        GatewayInstanceLifecycleState::Running => "Running",
        GatewayInstanceLifecycleState::Failed => "Failed",
    }
}

/// gateway_lifecycle_events 表行。
#[derive(Debug, FromRow)]
struct LifecycleEventRow {
    gateway_id: String,
    from_state: Option<String>,
    to_state: String,
    at: chrono::DateTime<chrono::Utc>,
}

impl LifecycleEventRow {
    fn into_event(self) -> LifecycleEvent {
        LifecycleEvent {
            gateway_id: self.gateway_id,
            from_state: self.from_state.as_deref().map(parse_lifecycle),
            to_state: parse_lifecycle(&self.to_state),
            at: DateTime::from_rfc3339(&self.at.to_rfc3339()).unwrap_or_else(DateTime::now),
        }
    }
}

impl GatewayRow {
    fn into_stored(self) -> StoredGateway {
        StoredGateway {
            gateway_id: self.gateway_id,
            instance_id: self.instance_id,
            credential_token_hash: self.credential_token_hash,
            credential_status: parse_credential_status(&self.credential_status),
            credential_expires_at: self.credential_expires_at.map(|value| value.to_rfc3339()),
            bootstrap_token_hash: self.bootstrap_token_hash,
            version: self.version,
            status: self.status,
            health: self.health,
            memory_bytes: self.memory_bytes,
            cpu_percent: self.cpu_percent,
            lifecycle_state: self.lifecycle_state.as_deref().map(parse_lifecycle),
            initialized_at: self.initialized_at.map(|value| {
                DateTime::from_rfc3339(&value.to_rfc3339()).unwrap_or_else(DateTime::now)
            }),
            created_at: self.created_at.map(|value| {
                DateTime::from_rfc3339(&value.to_rfc3339()).unwrap_or_else(DateTime::now)
            }),
            last_seen_at: self.last_seen_at.map(|value| {
                DateTime::from_rfc3339(&value.to_rfc3339()).unwrap_or_else(DateTime::now)
            }),
        }
    }
}

/// agent_status 表行（与 docker/initdb/01_schema.sql 列对应）。
#[derive(Debug, FromRow)]
struct AgentRow {
    agent_id: String,
    gateway_id: String,
    instance_id: String,
    version: String,
    status: String,
    health: String,
    memory_bytes: Option<i64>,
    cpu_percent: Option<f64>,
    admin_latency_ms: Option<i64>,
    last_seen_at: chrono::DateTime<chrono::Utc>,
}

impl AgentRow {
    fn into_stored(self) -> StoredAgent {
        StoredAgent {
            agent_id: self.agent_id,
            gateway_id: self.gateway_id,
            instance_id: self.instance_id,
            version: self.version,
            status: self.status,
            health: self.health,
            memory_bytes: self.memory_bytes,
            cpu_percent: self.cpu_percent,
            admin_latency_ms: self.admin_latency_ms,
            last_seen_at: DateTime::from_rfc3339(&self.last_seen_at.to_rfc3339())
                .unwrap_or_else(DateTime::now),
        }
    }
}

/// DB 里 TEXT 存的枚举名 → 域枚举（未知值按 active 兜底，保持前向兼容）。
fn parse_credential_status(value: &str) -> StoredGatewayCredentialStatus {
    match value {
        "expired" => StoredGatewayCredentialStatus::Expired,
        "revoked" => StoredGatewayCredentialStatus::Revoked,
        _ => StoredGatewayCredentialStatus::Active,
    }
}

/// chrono TIMESTAMPTZ → 共享 DateTime。
fn to_shared_dt(value: chrono::DateTime<chrono::Utc>) -> DateTime {
    DateTime::from_rfc3339(&value.to_rfc3339()).unwrap_or_else(DateTime::now)
}

/// enrollment_tokens 行 → StoredEnrollmentToken（create/consume/revoke 共用）。
fn enrollment_token_from_row(row: &sqlx::postgres::PgRow) -> StoredEnrollmentToken {
    let issued_at: chrono::DateTime<chrono::Utc> = row.get("issued_at");
    let expires_at: Option<chrono::DateTime<chrono::Utc>> = row.get("expires_at");
    let revoked_at: Option<chrono::DateTime<chrono::Utc>> = row.get("revoked_at");
    StoredEnrollmentToken {
        token_id: row.get("token_id"),
        token_hash: row.get("token_hash"),
        gateway_id: row.get("gateway_id"),
        tenant_id: row.get("tenant_id"),
        environment_id: row.get("environment_id"),
        issued_by: row.get("issued_by"),
        control_center_trust_bundle: row.get("control_center_trust_bundle"),
        max_uses: row.get("max_uses"),
        used_count: row.get("used_count"),
        status: row.get("status"),
        issued_at: to_shared_dt(issued_at),
        expires_at: expires_at.map(to_shared_dt),
        revoked_at: revoked_at.map(to_shared_dt),
    }
}

fn parse_optional_timestamptz(value: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    value
        .as_deref()
        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .map(|value| value.with_timezone(&chrono::Utc))
}

const GATEWAY_COLUMNS: &str = "gateway_id, instance_id, credential_token_hash, \
                               credential_status, credential_expires_at, bootstrap_token_hash, \
                               version, status, health, memory_bytes, cpu_percent, \
                               lifecycle_state, initialized_at, created_at, last_seen_at";

#[async_trait::async_trait]
impl Store for PgStore {
    async fn seed(&self, seeds: &[GatewayCredentialSeed]) -> Result<bool, StoreError> {
        let mut added = false;
        for seed in seeds {
            let token_hash = sha256_hex(&seed.token);
            let expires_at = parse_optional_timestamptz(&seed.expires_at);
            let result = sqlx::query(
                "INSERT INTO gateways \
                    (gateway_id, instance_id, credential_token_hash, credential_expires_at, \
                     lifecycle_state, created_at) \
                 VALUES ($1, '', $2, $3, 'Provisioned', NOW()) \
                 ON CONFLICT (gateway_id) DO NOTHING",
            )
            .bind(&seed.gateway_id)
            .bind(token_hash)
            .bind(expires_at)
            .execute(&self.pool)
            .await
            .source_raw_err(StoreReason::Sql, "seed gateway")?;
            added |= result.rows_affected() > 0;
        }
        Ok(added)
    }

    async fn list_gateways(&self) -> Result<Vec<StoredGateway>, StoreError> {
        let rows: Vec<GatewayRow> = sqlx::query_as(&format!(
            "SELECT {GATEWAY_COLUMNS} FROM gateways ORDER BY gateway_id"
        ))
        .fetch_all(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "list gateways")?;
        Ok(rows.into_iter().map(GatewayRow::into_stored).collect())
    }

    async fn get_gateway(&self, gateway_id: &str) -> Result<Option<StoredGateway>, StoreError> {
        let row: Option<GatewayRow> = sqlx::query_as(&format!(
            "SELECT {GATEWAY_COLUMNS} FROM gateways WHERE gateway_id = $1"
        ))
        .bind(gateway_id)
        .fetch_optional(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "get gateway")?;
        Ok(row.map(GatewayRow::into_stored))
    }

    async fn upsert_gateway_status(&self, update: &GatewayStatusUpdate) -> Result<(), StoreError> {
        let current = self.get_gateway(&update.gateway_id).await?;
        let prev = current.and_then(|gateway| gateway.lifecycle_state);
        let last_seen_at = update.last_seen_at.to_chrono();
        sqlx::query(
            "UPDATE gateways \
             SET instance_id = $2, version = $3, status = $4, health = $5, \
                 memory_bytes = $6, cpu_percent = $7, \
                 lifecycle_state = 'Running', \
                 initialized_at = COALESCE(initialized_at, NOW()), \
                 last_seen_at = $8 \
             WHERE gateway_id = $1",
        )
        .bind(&update.gateway_id)
        .bind(&update.instance_id)
        .bind(&update.version)
        .bind(&update.status)
        .bind(&update.health)
        .bind(update.memory_bytes)
        .bind(update.cpu_percent)
        .bind(last_seen_at)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "upsert gateway status")?;
        // 首次上报 → Running（记录转变事件）。
        if prev != Some(GatewayInstanceLifecycleState::Running) {
            self.record_lifecycle_event(
                &update.gateway_id,
                prev,
                GatewayInstanceLifecycleState::Running,
            )
            .await?;
        }
        Ok(())
    }

    async fn create_gateway(
        &self,
        gateway_id: &str,
        token: &str,
    ) -> Result<StoredGateway, StoreError> {
        let bootstrap_hash = if token.is_empty() {
            String::new()
        } else {
            sha256_hex(token)
        };
        let result = sqlx::query(
            "INSERT INTO gateways (gateway_id, instance_id, bootstrap_token_hash, lifecycle_state, created_at) \
             VALUES ($1, '', $2, 'Provisioned', NOW()) \
             ON CONFLICT (gateway_id) DO NOTHING",
        )
        .bind(gateway_id)
        .bind(&bootstrap_hash)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "insert gateway")?;
        if result.rows_affected() == 0 {
            return Err(StoreReason::Conflict
                .to_err()
                .with_detail(gateway_id.to_string()));
        }
        self.record_lifecycle_event(gateway_id, None, GatewayInstanceLifecycleState::Provisioned)
            .await?;
        let mut stored =
            StoredGateway::provisioned(gateway_id.to_string(), String::new(), String::new(), None);
        stored.bootstrap_token_hash = bootstrap_hash;
        Ok(stored)
    }

    async fn consume_bootstrap_token(
        &self,
        gateway_id: &str,
        bootstrap_token: &str,
    ) -> Result<bool, StoreError> {
        let bootstrap_hash = sha256_hex(bootstrap_token);
        let result = sqlx::query(
            "UPDATE gateways SET bootstrap_token_hash = '' \
             WHERE gateway_id = $1 AND bootstrap_token_hash = $2 \
               AND bootstrap_token_hash <> '' AND lifecycle_state = 'Provisioned'",
        )
        .bind(gateway_id)
        .bind(bootstrap_hash)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "consume bootstrap token")?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_gateway_credential(
        &self,
        gateway_id: &str,
        token_hash: &str,
        expires_at: Option<String>,
    ) -> Result<bool, StoreError> {
        let expires_at = parse_optional_timestamptz(&expires_at);
        let result = sqlx::query(
            "UPDATE gateways SET credential_token_hash = $2, credential_status = 'active', \
                 credential_expires_at = $3 \
             WHERE gateway_id = $1",
        )
        .bind(gateway_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "update gateway credential")?;
        Ok(result.rows_affected() > 0)
    }

    async fn create_enrollment_token(
        &self,
        gateway_id: &str,
        issue: &EnrollmentTokenIssue,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        let token_hash = sha256_hex(&issue.token);
        let token_id = format!(
            "enroll-{gateway_id}-{}",
            DateTime::now().to_chrono().timestamp()
        );
        let row = sqlx::query(
            "INSERT INTO enrollment_tokens \
               (token_id, token_hash, gateway_id, tenant_id, environment_id, issued_by, \
                control_center_trust_bundle, max_uses, used_count, status, issued_at, expires_at) \
             VALUES ($1, $2, $3, 'tenant-default', 'env-default', $4, $5, 1, 0, 'Active', \
                     NOW(), NOW() + INTERVAL '30 days') \
             RETURNING token_id, token_hash, gateway_id, tenant_id, environment_id, issued_by, \
                       control_center_trust_bundle, max_uses, used_count, status, issued_at, \
                       expires_at, revoked_at",
        )
        .bind(&token_id)
        .bind(&token_hash)
        .bind(gateway_id)
        .bind(&issue.issued_by)
        .bind(&issue.control_center_trust_bundle)
        .fetch_one(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "insert enrollment token")?;
        Ok(enrollment_token_from_row(&row))
    }

    async fn consume_enrollment_token(
        &self,
        token: &str,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        let token_hash = sha256_hex(token);
        let now = chrono::Utc::now();
        let row = sqlx::query(
            "SELECT token_id, token_hash, gateway_id, tenant_id, environment_id, issued_by, \
                    control_center_trust_bundle, max_uses, used_count, status, issued_at, \
                    expires_at, revoked_at \
             FROM enrollment_tokens WHERE token_hash = $1 FOR UPDATE",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "select enrollment token")?
        .ok_or_else(|| {
            StoreReason::Enrollment
                .to_err()
                .with_detail("token not found")
        })?;
        let status: String = row.get("status");
        let used_count: i64 = row.get("used_count");
        let max_uses: i64 = row.get("max_uses");
        let expires_at: Option<chrono::DateTime<chrono::Utc>> = row.get("expires_at");
        if status != "Active" && status != "Used" {
            return Err(StoreReason::Enrollment
                .to_err()
                .with_detail(format!("token status {status}")));
        }
        if expires_at.as_ref().is_some_and(|exp| *exp < now) {
            return Err(StoreReason::Enrollment
                .to_err()
                .with_detail("token expired"));
        }
        if used_count >= max_uses {
            return Err(StoreReason::Enrollment
                .to_err()
                .with_detail("token exhausted"));
        }
        let new_used = used_count + 1;
        let new_status = if new_used >= max_uses {
            "Exhausted"
        } else {
            "Used"
        };
        sqlx::query(
            "UPDATE enrollment_tokens SET used_count = $1, status = $2 WHERE token_id = $3",
        )
        .bind(new_used)
        .bind(new_status)
        .bind(row.get::<String, _>("token_id"))
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "update enrollment token")?;
        let mut token = enrollment_token_from_row(&row);
        token.used_count = new_used;
        token.status = new_status.to_string();
        Ok(token)
    }

    async fn revoke_enrollment_token(
        &self,
        gateway_id: &str,
        token_id: &str,
    ) -> Result<StoredEnrollmentToken, StoreError> {
        let row = sqlx::query(
            "UPDATE enrollment_tokens SET status = 'Revoked', revoked_at = NOW() \
             WHERE token_id = $1 AND gateway_id = $2 \
             RETURNING token_id, token_hash, gateway_id, tenant_id, environment_id, issued_by, \
                       control_center_trust_bundle, max_uses, used_count, status, issued_at, \
                       expires_at, revoked_at",
        )
        .bind(token_id)
        .bind(gateway_id)
        .fetch_optional(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "revoke enrollment token")?
        .ok_or_else(|| {
            StoreReason::Enrollment
                .to_err()
                .with_detail("token not found")
        })?;
        Ok(enrollment_token_from_row(&row))
    }

    async fn get_enrollment_token_for_gateway(
        &self,
        gateway_id: &str,
    ) -> Result<Option<StoredEnrollmentToken>, StoreError> {
        let row = sqlx::query(
            "SELECT token_id, token_hash, gateway_id, tenant_id, environment_id, issued_by, \
                    control_center_trust_bundle, max_uses, used_count, status, issued_at, \
                    expires_at, revoked_at \
             FROM enrollment_tokens WHERE gateway_id = $1 \
             ORDER BY issued_at DESC LIMIT 1",
        )
        .bind(gateway_id)
        .fetch_optional(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "get enrollment token")?;
        Ok(row.map(|row| enrollment_token_from_row(&row)))
    }

    async fn upsert_agent_status(
        &self,
        gateway_id: &str,
        agents: &[StoredAgent],
    ) -> Result<(), StoreError> {
        for agent in agents {
            let last_seen_at = agent.last_seen_at.to_chrono();
            sqlx::query(
                "INSERT INTO agent_status \
                    (agent_id, gateway_id, instance_id, version, status, health, \
                     memory_bytes, cpu_percent, admin_latency_ms, last_seen_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
                 ON CONFLICT (agent_id) DO UPDATE SET \
                   gateway_id = EXCLUDED.gateway_id, instance_id = EXCLUDED.instance_id, \
                   version = EXCLUDED.version, status = EXCLUDED.status, \
                   health = EXCLUDED.health, memory_bytes = EXCLUDED.memory_bytes, \
                   cpu_percent = EXCLUDED.cpu_percent, \
                   admin_latency_ms = EXCLUDED.admin_latency_ms, \
                   last_seen_at = EXCLUDED.last_seen_at",
            )
            .bind(&agent.agent_id)
            .bind(gateway_id)
            .bind(&agent.instance_id)
            .bind(&agent.version)
            .bind(&agent.status)
            .bind(&agent.health)
            .bind(agent.memory_bytes)
            .bind(agent.cpu_percent)
            .bind(agent.admin_latency_ms)
            .bind(last_seen_at)
            .execute(&self.pool)
            .await
            .source_raw_err(StoreReason::Sql, "upsert agent status")?;
        }
        Ok(())
    }

    async fn list_agents_by_gateway(
        &self,
        gateway_id: &str,
    ) -> Result<Vec<StoredAgent>, StoreError> {
        let rows: Vec<AgentRow> = sqlx::query_as(
            "SELECT agent_id, gateway_id, instance_id, version, status, health, \
                    memory_bytes, cpu_percent, admin_latency_ms, last_seen_at \
             FROM agent_status WHERE gateway_id = $1 ORDER BY agent_id",
        )
        .bind(gateway_id)
        .fetch_all(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "list agents")?;
        Ok(rows.into_iter().map(AgentRow::into_stored).collect())
    }

    async fn mark_gateway_initializing(&self, gateway_id: &str) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE gateways SET lifecycle_state = 'Initializing' \
             WHERE gateway_id = $1 AND lifecycle_state = 'Provisioned'",
        )
        .bind(gateway_id)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "mark gateway initializing")?;
        if result.rows_affected() > 0 {
            self.record_lifecycle_event(
                gateway_id,
                Some(GatewayInstanceLifecycleState::Provisioned),
                GatewayInstanceLifecycleState::Initializing,
            )
            .await?;
        }
        Ok(())
    }

    async fn record_lifecycle_event(
        &self,
        gateway_id: &str,
        from: Option<GatewayInstanceLifecycleState>,
        to: GatewayInstanceLifecycleState,
    ) -> Result<(), StoreError> {
        let at = chrono::Utc::now();
        let from_state = from.map(lifecycle_name);
        let to_state = lifecycle_name(to);
        sqlx::query(
            "INSERT INTO gateway_lifecycle_events (gateway_id, from_state, to_state, at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(gateway_id)
        .bind(from_state)
        .bind(to_state)
        .bind(at)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "insert lifecycle event")?;
        Ok(())
    }

    async fn list_lifecycle_events(
        &self,
        gateway_id: &str,
    ) -> Result<Vec<LifecycleEvent>, StoreError> {
        let rows: Vec<LifecycleEventRow> = sqlx::query_as(
            "SELECT gateway_id, from_state, to_state, at FROM gateway_lifecycle_events \
             WHERE gateway_id = $1 ORDER BY at",
        )
        .bind(gateway_id)
        .fetch_all(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "list lifecycle events")?;
        Ok(rows
            .into_iter()
            .map(LifecycleEventRow::into_event)
            .collect())
    }

    async fn publish_release(
        &self,
        component: &str,
        version: &str,
        artifact_url: &str,
    ) -> Result<ReleaseRecord, StoreError> {
        let published_at = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO release_records (component, version, artifact_url, status, published_at) \
             VALUES ($1, $2, $3, 'published', $4)",
        )
        .bind(component)
        .bind(version)
        .bind(artifact_url)
        .bind(published_at)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "insert release")?;
        Ok(ReleaseRecord {
            version: version.to_string(),
            artifact_url: artifact_url.to_string(),
            status: "published".to_string(),
            published_at: DateTime::from_rfc3339(&published_at.to_rfc3339())
                .unwrap_or_else(DateTime::now),
        })
    }

    async fn list_releases(&self, component: &str) -> Result<Vec<ReleaseRecord>, StoreError> {
        let rows: Vec<ReleaseRow> = sqlx::query_as(
            "SELECT version, artifact_url, status, published_at FROM release_records \
             WHERE component = $1 ORDER BY published_at DESC",
        )
        .bind(component)
        .fetch_all(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "list releases")?;
        Ok(rows.into_iter().map(ReleaseRow::into_record).collect())
    }

    async fn create_upgrade_plan(
        &self,
        plan: &UpgradePlanRecord,
    ) -> Result<UpgradePlanRecord, StoreError> {
        let payload =
            serde_json::to_value(plan).source_err(StoreReason::Json, "serialize upgrade plan")?;
        sqlx::query(
            "INSERT INTO upgrade_plans (plan_id, payload, status) VALUES ($1, $2, 'pending')",
        )
        .bind(&plan.plan_id)
        .bind(payload)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "insert upgrade plan")?;
        Ok(plan.clone())
    }

    async fn list_upgrade_plans(&self) -> Result<Vec<UpgradePlanRecord>, StoreError> {
        let rows = sqlx::query("SELECT payload FROM upgrade_plans ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
            .source_raw_err(StoreReason::Sql, "list upgrade plans")?;
        let mut plans = Vec::new();
        for row in rows {
            let value: serde_json::Value = row
                .try_get("payload")
                .source_raw_err(StoreReason::Sql, "read upgrade plan payload")?;
            let plan = serde_json::from_value(value)
                .source_err(StoreReason::Json, "deserialize upgrade plan")?;
            plans.push(plan);
        }
        Ok(plans)
    }

    async fn approve_upgrade_plan(
        &self,
        plan_id: &str,
        approved_by: &str,
    ) -> Result<UpgradePlanRecord, StoreError> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT payload FROM upgrade_plans WHERE plan_id = $1")
                .bind(plan_id)
                .fetch_optional(&self.pool)
                .await
                .source_raw_err(StoreReason::Sql, "select upgrade plan")?;
        let Some((payload,)) = row else {
            return Err(StoreReason::Conflict
                .to_err()
                .with_detail(plan_id.to_string()));
        };
        let mut plan: UpgradePlanRecord = serde_json::from_value(payload)
            .source_err(StoreReason::Json, "deserialize upgrade plan")?;
        plan.status = "approved".to_string();
        plan.approved_by = Some(approved_by.to_string());
        plan.approved_at = Some(DateTime::now());
        let payload =
            serde_json::to_value(&plan).source_err(StoreReason::Json, "serialize upgrade plan")?;
        sqlx::query(
            "UPDATE upgrade_plans SET payload = $2, status = 'approved' WHERE plan_id = $1",
        )
        .bind(plan_id)
        .bind(payload)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "update upgrade plan")?;
        Ok(plan)
    }

    async fn bind_gateway_customer(
        &self,
        gateway_id: &str,
        customer_id: &str,
    ) -> Result<GatewayCustomerBindingRecord, StoreError> {
        let bound_at = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO gateway_customer_bindings (gateway_id, customer_id, status, bound_at) \
             VALUES ($1, $2, 'bound', $3) \
             ON CONFLICT (gateway_id) DO UPDATE SET customer_id = $2, status = 'bound', bound_at = $3",
        )
        .bind(gateway_id)
        .bind(customer_id)
        .bind(bound_at)
        .execute(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "bind gateway customer")?;
        Ok(GatewayCustomerBindingRecord {
            gateway_id: gateway_id.to_string(),
            customer_id: customer_id.to_string(),
            status: "bound".to_string(),
            bound_at: DateTime::from_rfc3339(&bound_at.to_rfc3339()).unwrap_or_else(DateTime::now),
        })
    }

    async fn list_customer_bindings(
        &self,
    ) -> Result<Vec<GatewayCustomerBindingRecord>, StoreError> {
        let rows: Vec<(String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT gateway_id, customer_id, status, bound_at FROM gateway_customer_bindings",
        )
        .fetch_all(&self.pool)
        .await
        .source_raw_err(StoreReason::Sql, "list customer bindings")?;
        Ok(rows
            .into_iter()
            .map(
                |(gateway_id, customer_id, status, bound_at)| GatewayCustomerBindingRecord {
                    gateway_id,
                    customer_id,
                    status,
                    bound_at: DateTime::from_rfc3339(&bound_at.to_rfc3339())
                        .unwrap_or_else(DateTime::now),
                },
            )
            .collect())
    }
}

/// release_records 表行。
#[derive(Debug, FromRow)]
struct ReleaseRow {
    version: String,
    artifact_url: String,
    status: String,
    published_at: chrono::DateTime<chrono::Utc>,
}

impl ReleaseRow {
    fn into_record(self) -> ReleaseRecord {
        ReleaseRecord {
            version: self.version,
            artifact_url: self.artifact_url,
            status: self.status,
            published_at: DateTime::from_rfc3339(&self.published_at.to_rfc3339())
                .unwrap_or_else(DateTime::now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_credential_status_with_unknown_fallback() {
        assert_eq!(
            parse_credential_status("active"),
            StoredGatewayCredentialStatus::Active
        );
        assert_eq!(
            parse_credential_status("expired"),
            StoredGatewayCredentialStatus::Expired
        );
        assert_eq!(
            parse_credential_status("revoked"),
            StoredGatewayCredentialStatus::Revoked
        );
        // 未知状态前向兼容：按 active 兜底。
        assert_eq!(
            parse_credential_status("suspended"),
            StoredGatewayCredentialStatus::Active
        );
    }

    #[test]
    fn parses_optional_timestamptz_from_rfc3339() {
        let value = Some("2026-08-08T12:00:00Z".to_string());
        let parsed = parse_optional_timestamptz(&value).expect("parse");
        assert_eq!(parsed.to_rfc3339(), "2026-08-08T12:00:00+00:00");
        assert_eq!(parse_optional_timestamptz(&None), None);
        assert_eq!(
            parse_optional_timestamptz(&Some("not-a-date".to_string())),
            None
        );
    }

    /// 集成测试：需本地 PostgreSQL（docker compose up -d postgres）。
    /// 用独立 gateway_id 避免污染，结束清理。默认连 compose 的 insight_demo 库，
    /// 可用 WARP_INSIGHT_CENTER_DATABASE_URL 覆盖。
    ///
    /// 运行：`cargo test -p wist-center -- --ignored pg_store_round_trip`
    #[tokio::test]
    #[ignore = "需要本地 PostgreSQL（docker compose up -d postgres）"]
    async fn pg_store_round_trip() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let database_url = std::env::var("WARP_INSIGHT_CENTER_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://demo:demo@127.0.0.1:55432/insight_demo".to_string());
        let store = PgStore::connect(&database_url).await.expect("connect pg");

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let gateway_id = format!("pg-it-{nanos}");

        // seed：缺失时新增，重复 seed 不再新增。
        let seeds = vec![GatewayCredentialSeed {
            gateway_id: gateway_id.clone(),
            token: "it-secret".to_string(),
            expires_at: None,
        }];
        assert!(store.seed(&seeds).await.expect("seed"));
        assert!(!store.seed(&seeds).await.expect("seed again"));

        let stored = store
            .get_gateway(&gateway_id)
            .await
            .expect("get")
            .expect("seeded gateway");
        assert_eq!(stored.credential_token_hash, sha256_hex("it-secret"));
        assert_eq!(
            stored.credential_status,
            StoredGatewayCredentialStatus::Active
        );

        // 上报状态落库后读回（TIMESTAMPTZ ↔ chrono ↔ 共享 DateTime）。
        let reported_at = DateTime::now();
        store
            .upsert_gateway_status(&GatewayStatusUpdate {
                gateway_id: gateway_id.clone(),
                instance_id: "it-instance".to_string(),
                version: "v9.9.9".to_string(),
                status: "online".to_string(),
                health: "healthy".to_string(),
                memory_bytes: Some(1_073_741_824),
                cpu_percent: Some(18.2),
                last_seen_at: reported_at.clone(),
            })
            .await
            .expect("upsert status");

        let updated = store
            .get_gateway(&gateway_id)
            .await
            .expect("get updated")
            .expect("still present");
        assert_eq!(updated.instance_id, "it-instance");
        assert_eq!(updated.version.as_deref(), Some("v9.9.9"));
        assert_eq!(updated.status.as_deref(), Some("online"));
        assert_eq!(updated.health.as_deref(), Some("healthy"));
        assert_eq!(
            updated.last_seen_at.expect("last_seen_at").to_chrono(),
            reported_at.to_chrono()
        );

        // 列表包含该网关。
        let all = store.list_gateways().await.expect("list");
        assert!(all.iter().any(|gateway| gateway.gateway_id == gateway_id));

        // 清理测试数据。
        sqlx::query("DELETE FROM gateways WHERE gateway_id = $1")
            .bind(&gateway_id)
            .execute(&store.pool)
            .await
            .expect("cleanup");
    }

    /// 集成测试：create_gateway 的 PG 往返 + 重复创建冲突。需本地 PostgreSQL。
    #[tokio::test]
    #[ignore = "需要本地 PostgreSQL（docker compose up -d postgres）"]
    async fn pg_store_create_gateway_and_conflict() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let database_url = std::env::var("WARP_INSIGHT_CENTER_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://demo:demo@127.0.0.1:55432/insight_demo".to_string());
        let store = PgStore::connect(&database_url).await.expect("connect pg");

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let gateway_id = format!("pg-create-{nanos}");

        let stored = store
            .create_gateway(&gateway_id, "it-token")
            .await
            .expect("create");
        assert_eq!(stored.gateway_id, gateway_id);
        assert_eq!(stored.credential_token_hash, sha256_hex("it-token"));
        assert_eq!(
            stored.credential_status,
            StoredGatewayCredentialStatus::Active
        );

        // 重复创建同一 gateway_id → Conflict。
        let err = store
            .create_gateway(&gateway_id, "other-token")
            .await
            .expect_err("conflict");
        assert_eq!(err.reason(), &StoreReason::Conflict);
        assert_eq!(err.detail().as_deref(), Some(gateway_id.as_str()));

        // 清理测试数据。
        sqlx::query("DELETE FROM gateways WHERE gateway_id = $1")
            .bind(&gateway_id)
            .execute(&store.pool)
            .await
            .expect("cleanup");
    }
}
