use std::{net::SocketAddr, time::Duration};

use anyhow::Result;
use pb::qa_service_server::QaServiceServer;
use qa_svc::{APP_CONFIG, AppState, application};
use qa_sys_core::{graceful_shutdown, prometheus_init};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{Layer as _, fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt};
pub(crate) const PROTO_FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../rpc_descriptor.bin");
#[tokio::main]
async fn main() -> Result<()> {
    let layer = Layer::new().pretty().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();
    let address: SocketAddr = format!("0.0.0.0:{}", APP_CONFIG.app_port).parse().unwrap();
    let pgsql_pool = qa_svc::config::pgsql::pool(&APP_CONFIG.pgsql_conf).await?;
    let xredis_pool = qa_svc::config::xredis::pool(&APP_CONFIG.redis_conf).await?;
    let xpulsar_client = qa_svc::config::xpulsar::client(&APP_CONFIG.pulsar_conf).await?;
    let state = AppState {
        pgsql_pool,
        pulsar_client: xpulsar_client,
        redis_pool: xredis_pool,
    };
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(PROTO_FILE_DESCRIPTOR_SET)
        .build_v1()
        .unwrap();
    let qa_service = application::app::QaServiceImpl::new(state);
    let svc = QaServiceServer::new(qa_service);

    info!("grpc server running at {}", address);
    let server = tonic::transport::Server::builder()
        .add_service(reflection_service)
        .add_service(svc)
        .serve_with_shutdown(
            address,
            graceful_shutdown(Duration::from_secs(APP_CONFIG.graceful_wait_time)),
        );
    let metrics_server = prometheus_init(APP_CONFIG.metrics_port);
    let grpc_handler = tokio::spawn(server);
    let http_handler = tokio::spawn(metrics_server);

    let _ = tokio::try_join!(grpc_handler, http_handler);
    Ok(())
}
