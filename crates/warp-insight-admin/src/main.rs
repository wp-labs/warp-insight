// @moju generated
// @moju hash=46ea6f5515c8cd4b

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr =
        std::env::var("WARP_INSIGHT_ADMIN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("warp-insight-admin listening on http://{addr}");
    axum::serve(listener, warp_insight_admin::api::router()).await?;
    Ok(())
}
