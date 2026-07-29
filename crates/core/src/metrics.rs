use std::{net::Ipv4Addr, time::Duration};

use autometrics::prometheus_exporter;
use axum::{Router, routing::get};
use tokio::net::TcpListener;

use crate::graceful_shutdown;
pub async fn prometheus_init(port: u16) {
    prometheus_exporter::init();
    let router = Router::new().route(
        "/metrics",
        get(|| async { prometheus_exporter::encode_http_response() }),
    );
    let listener = TcpListener::bind((Ipv4Addr::from([127, 0, 0, 1]), port))
        .await
        .unwrap();
    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(graceful_shutdown(Duration::from_secs(5)))
        .await
        .expect("Failed to serve metrics");
}
