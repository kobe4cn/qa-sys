use std::{net::SocketAddr, sync::Arc, time::Duration};

use gateway::{
    config::app::{APP_CONFIG, AppState},
    router::api_router,
};
use pb::qa_service_client::QaServiceClient;
use qa_sys_core::{graceful_shutdown, prometheus_init};
use tokio::net::TcpListener;

#[tokio::main]

async fn main() -> anyhow::Result<()> {
    let metrics_server = prometheus_init(APP_CONFIG.metrics_port);
    let metrics_handler = tokio::spawn(metrics_server);

    let gateway_handler = tokio::spawn(async move {
        let grpc_client = QaServiceClient::connect(APP_CONFIG.grpc_address.as_str())
            .await
            .expect("failed to connect the grpc server");

        let app_state = Arc::new(AppState { grpc_client });
        let router = api_router(app_state).expect("failed to get api router");
        let address: SocketAddr = format!("0.0.0.0:{}", APP_CONFIG.app_port).parse().unwrap();
        let listener = TcpListener::bind(address)
            .await
            .expect("failed to bind address");
        axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(graceful_shutdown(Duration::from_secs(
                APP_CONFIG.graceful_wait_time,
            )))
            .await
            .expect("failed to start gateway service");
    });

    let _ = tokio::join!(metrics_handler, gateway_handler);

    Ok(())
}
