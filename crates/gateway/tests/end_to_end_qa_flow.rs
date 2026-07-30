mod common;

use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use common::{TestDatabase, clear_redis_database, pulsar_client, redis_pool};
use gateway::{config::app::AppState as GatewayState, router::api_router};
use http_body_util::BodyExt;
use pb::{qa_service_client::QaServiceClient, qa_service_server::QaServiceServer};
use qa_svc::{AppState as ServiceState, QaServiceImpl};
use serde_json::{Value, json};
use tonic::transport::{Server, server::TcpIncoming};
use tower::ServiceExt;

fn json_request(path: &str, body: Value, token: Option<&str>) -> Result<Request<Body>> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    Ok(builder.body(Body::from(serde_json::to_vec(&body)?))?)
}

async fn response_json(response: axum::response::Response) -> Result<Value> {
    let bytes = response.into_body().collect().await?.to_bytes();
    serde_json::from_slice(&bytes).context("decode Gateway JSON response")
}

async fn register_and_login(router: &Router, username: &str) -> Result<String> {
    let register = router
        .clone()
        .oneshot(json_request(
            "/api/user/register",
            json!({
                "username": username,
                "password": "secret-password",
                "email": format!("{username}@example.com"),
                "phone": "12345678"
            }),
            None,
        )?)
        .await?;
    ensure!(register.status() == StatusCode::OK);
    let login = router
        .clone()
        .oneshot(json_request(
            "/api/user/login",
            json!({"username": username, "password": "secret-password"}),
            None,
        )?)
        .await?;
    ensure!(login.status() == StatusCode::OK);
    response_json(login)
        .await?
        .get("token")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("login response must contain a token")
}

#[tokio::test]
async fn test_should_reject_body_username_spoofing() -> Result<()> {
    let database = TestDatabase::create().await?;
    let redis = redis_pool()?;
    clear_redis_database(&redis)?;
    let incoming = TcpIncoming::bind("127.0.0.1:0".parse()?)?;
    let grpc_address = incoming.local_addr()?;
    let service = QaServiceImpl::new(ServiceState {
        pgsql_pool: database.pool.clone(),
        pulsar_client: pulsar_client().await?,
        redis_pool: redis.clone(),
    });
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(QaServiceServer::new(service))
            .serve_with_incoming_shutdown(incoming, async {
                let _result = shutdown_receiver.await;
            })
            .await
    });
    let grpc_client = QaServiceClient::connect(format!("http://{grpc_address}")).await?;
    let router = api_router(Arc::new(GatewayState { grpc_client }))?;

    let result = async {
        let alice_token = register_and_login(&router, "alice").await?;
        let bob_token = register_and_login(&router, "bob").await?;
        let created = router
            .clone()
            .oneshot(json_request(
                "/api/question/add",
                json!({
                    "title": "Bob's question",
                    "content": "Only Bob may delete this.",
                    "username": "bob"
                }),
                Some(&bob_token),
            )?)
            .await?;
        ensure!(created.status() == StatusCode::OK);
        let question_id = response_json(created)
            .await?
            .get("question_id")
            .and_then(Value::as_i64)
            .context("question response must contain an ID")?;

        let spoofed_delete = router
            .clone()
            .oneshot(json_request(
                "/api/question/delete",
                json!({"id": question_id, "username": "bob"}),
                Some(&alice_token),
            )?)
            .await?;
        ensure!(spoofed_delete.status() == StatusCode::FORBIDDEN);

        let detail = router
            .clone()
            .oneshot(json_request(
                "/api/question/detail",
                json!({"question_id": question_id, "username": "bob"}),
                Some(&bob_token),
            )?)
            .await?;
        ensure!(detail.status() == StatusCode::OK);
        Ok(())
    }
    .await;

    let _shutdown_result = shutdown_sender.send(());
    server.await.context("gRPC server task must not panic")??;
    clear_redis_database(&redis)?;
    database.cleanup().await?;
    result
}
