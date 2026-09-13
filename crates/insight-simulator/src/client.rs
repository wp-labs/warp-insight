// HTTP 客户端：镜像 wist-agentd 的 enrollment_http_client（超时 + TLS 处理）。

use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub fn build_client(insecure: bool) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT);
    if insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build().map_err(|err| format!("failed to build http client: {err}"))
}

/// 统一请求助手：Bearer 鉴权 + JSON；transport 错误 3 次退避重试（200ms * 2^attempt）。
/// `request` 闭包捕获已构建的 reqwest::Client。
pub async fn send_json(
    request: impl Fn() -> reqwest::RequestBuilder,
    token: &str,
) -> Result<reqwest::Response, String> {
    let mut attempt = 0;
    loop {
        let response = request()
            .bearer_auth(token)
            .header("content-type", "application/json")
            .send()
            .await;
        match response {
            Ok(response) => return Ok(response),
            Err(err) => {
                if attempt >= 2 {
                    return Err(format!("request failed after retries: {err}"));
                }
                let delay_ms = 200_u64 * (1_u64 << attempt);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                attempt += 1;
            }
        }
    }
}
