// @jumo generated
// WarpInsightCenter 上级聚合控制中心服务（crate: wist-center）：接收 WarpGateWay 状态上报。

use std::{error::Error, sync::Arc};

use wist_center::infra::{FileStore, PgStore, Store};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config =
        wist_center::config::CenterConfig::load_from_env().map_err(|err| err.into_boxed_std())?;
    // 开发期配置 WARP_INSIGHT_CENTER_DATABASE_URL → PostgreSQL；未配置 → JSON 文件回退。
    let store: Arc<dyn Store> = match &config.database_url {
        Some(database_url) => Arc::new(
            PgStore::connect(database_url)
                .await
                .map_err(|err| err.into_boxed_std())?,
        ),
        None => Arc::new(FileStore::new(config.store_path.clone())),
    };
    store
        .seed(&config.gateway_credentials)
        .await
        .map_err(|err| err.into_boxed_std())?;
    let addr = config.listen_addr.clone();
    let app = wist_center::api::router(config, store);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("wist-center listening on http://{addr}");
    // 注入真实 peer 地址供限流按 IP 分桶（忽略可伪造的 x-real-ip / x-forwarded-for）。
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
