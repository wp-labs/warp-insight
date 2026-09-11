// WarpInsightCenter 运行配置（env 驱动）。

use std::{env, path::PathBuf};

use orion_error::conversion::ToStructError;
use wist_error::{ConfigError, ConfigReason};

use crate::infra::sha256_hex;

const DEFAULT_LISTEN: &str = "127.0.0.1:3100";
const DEFAULT_STORE_PATH: &str = "state/warp-insight-center-store.json";
const ENV_LISTEN: &str = "WARP_INSIGHT_CENTER_LISTEN";
const ENV_STORE_PATH: &str = "WARP_INSIGHT_CENTER_STORE_PATH";
const ENV_GATEWAY_CREDENTIALS: &str = "WARP_INSIGHT_CENTER_GATEWAY_CREDENTIALS";
const ENV_ADMIN_TOKEN: &str = "WARP_INSIGHT_CENTER_ADMIN_TOKEN";
const ENV_DATABASE_URL: &str = "WARP_INSIGHT_CENTER_DATABASE_URL";
const ENV_VICTORIAMETRICS_URL: &str = "WARP_INSIGHT_CENTER_VICTORIAMETRICS_URL";
const ENV_PUBLIC_URL: &str = "WARP_INSIGHT_CENTER_PUBLIC_URL";
const ENV_GATEWAY_IMAGE: &str = "WARP_INSIGHT_CENTER_GATEWAY_IMAGE";
const ENV_ARTIFACT_DIR: &str = "WARP_INSIGHT_CENTER_ARTIFACT_DIR";
const ENV_OBJECT_STORAGE_ENDPOINT: &str = "WARP_INSIGHT_CENTER_OBJECT_STORAGE_ENDPOINT";
const ENV_OBJECT_STORAGE_BUCKET: &str = "WARP_INSIGHT_CENTER_OBJECT_STORAGE_BUCKET";
const ENV_OBJECT_STORAGE_ACCESS_KEY: &str = "WARP_INSIGHT_CENTER_OBJECT_STORAGE_ACCESS_KEY";
const ENV_OBJECT_STORAGE_SECRET_KEY: &str = "WARP_INSIGHT_CENTER_OBJECT_STORAGE_SECRET_KEY";
const ENV_CA_CERT_PATH: &str = "WARP_INSIGHT_CENTER_CA_CERT_PATH";
const ENV_PROTOCOL_VERSION: &str = "WARP_INSIGHT_CENTER_PROTOCOL_VERSION";
const ENV_HMAC_SECRET: &str = "WARP_INSIGHT_CENTER_HMAC_SECRET";
const ENV_CREDENTIAL_TTL_SECONDS: &str = "WARP_INSIGHT_CENTER_CREDENTIAL_TTL_SECONDS";

/// 默认对外地址使用域名（初始化 URL / 控制中心端点需要可被 Gateway 从外网访问）。
const DEFAULT_PUBLIC_URL: &str = "https://center.warpinsight.example";
const DEFAULT_GATEWAY_IMAGE: &str = "warp-gateway:latest";
const DEFAULT_ARTIFACT_DIR: &str = "artifacts";

#[derive(Debug, Clone)]
pub struct CenterConfig {
    pub listen_addr: String,
    pub store_path: PathBuf,
    /// seed 网关凭证：gateway_id:token 列表，启动时写入 store（缺失才写）。
    pub gateway_credentials: Vec<GatewayCredentialSeed>,
    /// 管理面 token hash（读取接口迭代用，本次未启用）。
    pub admin_token_hash: Option<String>,
    /// 开发期 PG 连接串：有值 → PgStore；未设置/为空 → FileStore。
    /// 推荐值即 compose 的 `postgres://demo:demo@127.0.0.1:55432/insight_demo`。
    pub database_url: Option<String>,
    /// 时序历史 VictoriaMetrics 基地址（如 compose 的 `http://127.0.0.1:8428`）。
    /// 有值 → 每次状态上报额外推送指标；未设置/为空 → 不启用（仅存快照）。
    pub victoriametrics_url: Option<String>,
    /// center 对外地址（生成网关初始化 URL），默认 `https://center.warpinsight.example`，
    /// 部署时通过 `WARP_INSIGHT_CENTER_PUBLIC_URL` 配置真实域名或 IP。
    /// 注意：该主机名/IP 必须落在控制中心服务器证书的 SAN 内（见
    /// `scripts/generate-control-center-cert.sh`），否则 server_tls_required=true 时
    /// 网关对 init_url 的 TLS 主机名校验会失败。
    pub public_url: String,
    /// gateway 镜像名（docker 安装命令 / 云镜像地址），默认 `warp-gateway:latest`。
    pub gateway_image: String,
    /// 本地制品目录（版本发布镜像制品落盘），默认 `artifacts/`。
    pub artifact_dir: PathBuf,
    /// 云对象存储（可选，S3 兼容 / MinIO）；配置了 endpoint 则发布制品存对象存储，否则本地。
    pub object_storage: Option<ObjectStorageConfig>,
    /// 控制中心 CA 证书内容（control-center.pem，信任根）——分发给 Gateway 作为
    /// control_center.trust_bundle。从 `WARP_INSIGHT_CENTER_CA_CERT_PATH` 读取，
    /// 默认 `~/.warpinsight-center/ca/control-center.pem`；文件不存在 → None。
    pub ca_cert: Option<String>,
    /// 网关↔中心 wire 协议版本（config.toml [protocol] version）。
    pub protocol_version: String,
    /// RegistToken 派生密钥（HMAC-SHA256）：由网关身份 Token 派生注册凭据。
    /// 派生后中心只存结果 hash、不重算，故轮换该密钥不影响既有凭据校验；
    /// 生产必须配置（`WARP_INSIGHT_CENTER_HMAC_SECRET`）。
    pub hmac_secret: String,
    /// 运行期凭据（RUNTIME_TOKEN）有效期秒数（镜像 warp-gateway 的 credential_ttl_seconds）。
    pub credential_ttl_seconds: i64,
}

/// S3 兼容对象存储配置（MinIO 等）。
#[derive(Debug, Clone)]
pub struct ObjectStorageConfig {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Debug, Clone)]
pub struct GatewayCredentialSeed {
    pub gateway_id: String,
    pub token: String,
    pub expires_at: Option<String>,
}

impl CenterConfig {
    pub fn load_from_env() -> Result<Self, ConfigError> {
        let listen_addr = env::var(ENV_LISTEN).unwrap_or_else(|_| DEFAULT_LISTEN.to_string());
        let store_path = PathBuf::from(
            env::var(ENV_STORE_PATH).unwrap_or_else(|_| DEFAULT_STORE_PATH.to_string()),
        );
        let gateway_credentials =
            parse_gateway_credentials(&env::var(ENV_GATEWAY_CREDENTIALS).unwrap_or_default())?;
        let admin_token = env::var(ENV_ADMIN_TOKEN)
            .ok()
            .filter(|value| !value.trim().is_empty());
        let admin_token_hash = admin_token.as_deref().map(sha256_hex);
        // 有值 → PgStore；未设置或空串 → 回退 FileStore（无 PG 开发 / cargo test 可用）。
        let database_url = parse_optional_env_url(ENV_DATABASE_URL);
        // 有值 → 状态上报推送 VictoriaMetrics；未设置/空 → 不启用时序推送。
        let victoriametrics_url = parse_optional_env_url(ENV_VICTORIAMETRICS_URL);
        // 有值 → 用配置的对外地址/镜像；空 → 默认值。
        let public_url =
            env::var(ENV_PUBLIC_URL).unwrap_or_else(|_| DEFAULT_PUBLIC_URL.to_string());
        let gateway_image =
            env::var(ENV_GATEWAY_IMAGE).unwrap_or_else(|_| DEFAULT_GATEWAY_IMAGE.to_string());
        let artifact_dir = PathBuf::from(
            env::var(ENV_ARTIFACT_DIR).unwrap_or_else(|_| DEFAULT_ARTIFACT_DIR.to_string()),
        );
        // 对象存储：endpoint/bucket/凭据齐全才启用（否则用本地文件）。
        let object_storage = match (
            env::var(ENV_OBJECT_STORAGE_ENDPOINT).ok(),
            env::var(ENV_OBJECT_STORAGE_BUCKET).ok(),
            env::var(ENV_OBJECT_STORAGE_ACCESS_KEY).ok(),
            env::var(ENV_OBJECT_STORAGE_SECRET_KEY).ok(),
        ) {
            (Some(endpoint), Some(bucket), Some(access_key), Some(secret_key)) => {
                Some(ObjectStorageConfig {
                    endpoint,
                    bucket,
                    access_key,
                    secret_key,
                })
            }
            _ => None,
        };
        // 信任根：从 WARP_INSIGHT_CENTER_CA_CERT_PATH 读取 control-center.pem；
        // 默认 ~/.warpinsight-center/ca/control-center.pem；文件不存在 → None。
        let ca_cert = {
            let default_path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".warpinsight-center")
                .join("ca")
                .join("control-center.pem");
            let path = env::var(ENV_CA_CERT_PATH)
                .map(PathBuf::from)
                .unwrap_or(default_path);
            std::fs::read_to_string(&path).ok()
        };
        let protocol_version = env::var(ENV_PROTOCOL_VERSION).unwrap_or_else(|_| "1.0".to_string());
        // RegistToken 派生密钥：生产必须显式配置；未配置 → dev 固定默认值。
        let hmac_secret = env::var(ENV_HMAC_SECRET)
            .unwrap_or_else(|_| "dev-center-hmac-secret-change-me".to_string());
        // 运行期凭据有效期：默认 30 天。
        let credential_ttl_seconds = env::var(ENV_CREDENTIAL_TTL_SECONDS)
            .ok()
            .and_then(|value| value.trim().parse::<i64>().ok())
            .unwrap_or(30 * 24 * 3600);
        if listen_addr.trim().is_empty() {
            return Err(config_validation("listen addr must not be empty"));
        }
        Ok(Self {
            listen_addr,
            store_path,
            gateway_credentials,
            admin_token_hash,
            database_url,
            victoriametrics_url,
            public_url,
            gateway_image,
            artifact_dir,
            object_storage,
            ca_cert,
            protocol_version,
            hmac_secret,
            credential_ttl_seconds,
        })
    }
}

/// 读取可选 env URL：未设置或空/纯空白 → None。
fn parse_optional_env_url(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 解析 `gateway_id:token,gateway_id:token,...`。
fn parse_gateway_credentials(raw: &str) -> Result<Vec<GatewayCredentialSeed>, ConfigError> {
    let mut seeds = Vec::new();
    for entry in raw
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let Some((gateway_id, token)) = entry.split_once(':') else {
            return Err(config_validation(format!(
                "invalid gateway credential entry {entry:?}: expected gateway_id:token"
            )));
        };
        let gateway_id = gateway_id.trim();
        let token = token.trim();
        if gateway_id.is_empty() || token.is_empty() {
            return Err(config_validation(format!(
                "invalid gateway credential entry {entry:?}: gateway_id and token must not be empty"
            )));
        }
        seeds.push(GatewayCredentialSeed {
            gateway_id: gateway_id.to_string(),
            token: token.to_string(),
            expires_at: None,
        });
    }
    Ok(seeds)
}

fn config_validation(message: impl Into<String>) -> ConfigError {
    ConfigReason::Validation.to_err().with_detail(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gateway_credential_seeds() {
        let seeds = parse_gateway_credentials("gw-001:tok-a,gw-002:tok-b").expect("seeds");
        assert_eq!(seeds.len(), 2);
        assert_eq!(seeds[0].gateway_id, "gw-001");
        assert_eq!(seeds[0].token, "tok-a");
        assert_eq!(seeds[1].gateway_id, "gw-002");
        assert_eq!(seeds[1].token, "tok-b");
    }

    #[test]
    fn rejects_malformed_seed_entry() {
        assert!(parse_gateway_credentials("gw-001").is_err());
        assert!(parse_gateway_credentials(":tok").is_err());
        assert!(parse_gateway_credentials("gw-001:").is_err());
    }

    #[test]
    fn empty_credentials_are_fine() {
        assert!(parse_gateway_credentials("").expect("empty").is_empty());
        assert!(parse_gateway_credentials(" , ").expect("blank").is_empty());
    }

    #[test]
    fn parses_optional_env_url_trimming_blank() {
        // 测试专用 env key，避免与其他并行测试互相污染。
        let key = "WARP_INSIGHT_CENTER_TEST_OPTIONAL_URL";
        std::env::set_var(key, "http://127.0.0.1:8428");
        assert_eq!(
            parse_optional_env_url(key).as_deref(),
            Some("http://127.0.0.1:8428")
        );
        std::env::set_var(key, "  ");
        assert_eq!(parse_optional_env_url(key), None);
        std::env::remove_var(key);
        assert_eq!(parse_optional_env_url(key), None);
    }
}
