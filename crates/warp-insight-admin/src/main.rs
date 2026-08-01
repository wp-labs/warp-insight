// @moju generated
// @moju hash=46ea6f5515c8cd4b

use std::{error::Error, net::SocketAddr, sync::Arc};

use axum::{
    extract::{connect_info::ConnectInfo, Request, State},
    middleware::{from_fn_with_state, Next},
    response::Response,
    Router,
};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = warp_insight_admin::infra::AdminConfig::load_from_env()?;
    let addr = config.listen_addr.clone();
    let tls_config = warp_insight_admin::infra::load_admin_tls_config(&config)?;
    let listener = TcpListener::bind(&addr).await?;
    println!("warp-insight-admin listening on https://{addr}");
    serve_tls(
        listener,
        warp_insight_admin::api::router(config),
        tls_config,
    )
    .await?;
    Ok(())
}

async fn serve_tls(
    listener: TcpListener,
    app: Router,
    tls_config: rustls::ServerConfig,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let acceptor = TlsAcceptor::from(Arc::new(tls_config));
    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let service = app.clone();
        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(stream).await {
                Ok(stream) => stream,
                Err(err) => {
                    eprintln!("failed TLS handshake from {peer_addr}: {err}");
                    return;
                }
            };
            let io = TokioIo::new(tls_stream);
            // Inject the real peer address so rate limiting can bucket per client and
            // cannot be bypassed with spoofed x-real-ip / x-forwarded-for headers.
            let service = service.layer(from_fn_with_state(peer_addr, inject_connect_info));
            let service = TowerToHyperService::new(service);
            let builder = Builder::new(TokioExecutor::new());
            if let Err(err) = builder.serve_connection(io, service).await {
                eprintln!("failed to serve HTTPS connection from {peer_addr}: {err}");
            }
        });
    }
}

async fn inject_connect_info(
    State(peer): State<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Response {
    request
        .extensions_mut()
        .insert(ConnectInfo::<SocketAddr>(peer));
    next.run(request).await
}
