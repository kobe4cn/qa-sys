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
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8");
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
    let body = response_json(login).await?;
    body.get("token")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("login response must contain a token")
}

#[tokio::test]
async fn test_should_enforce_http_contract_and_authentication() -> Result<()> {
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
        let root = router
            .clone()
            .oneshot(Request::get("/api/home").body(Body::empty())?)
            .await?;
        ensure!(root.status() == StatusCode::OK);
        ensure!(
            root.headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok())
                == Some("no-store, no-cache, must-revalidate")
        );

        let unauthenticated = router
            .clone()
            .oneshot(json_request(
                "/api/question/add",
                json!({"title": "title", "content": "content", "username": "alice"}),
                None,
            )?)
            .await?;
        ensure!(unauthenticated.status() == StatusCode::UNAUTHORIZED);

        let token = register_and_login(&router, "alice").await?;
        let created = router
            .clone()
            .oneshot(json_request(
                "/api/question/add",
                json!({"title": "title", "content": "content", "username": "alice"}),
                Some(&token),
            )?)
            .await?;
        ensure!(created.status() == StatusCode::OK);
        let created_body = response_json(created).await?;
        ensure!(
            created_body
                .get("question_id")
                .and_then(Value::as_i64)
                .is_some_and(|id| id > 0)
        );

        let malformed = router
            .clone()
            .oneshot(
                Request::post("/api/user/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{not-json"))?,
            )
            .await?;
        ensure!(malformed.status() == StatusCode::BAD_REQUEST);

        let not_found = router
            .clone()
            .oneshot(json_request(
                "/api/does-not-exist",
                json!({}),
                Some(&token),
            )?)
            .await?;
        ensure!(not_found.status() == StatusCode::NOT_FOUND);

        let mismatched_logout = router
            .clone()
            .oneshot(json_request(
                "/api/user/logout",
                json!({"token": "different-token"}),
                Some(&token),
            )?)
            .await?;
        ensure!(mismatched_logout.status() == StatusCode::FORBIDDEN);

        let logout = router
            .clone()
            .oneshot(json_request(
                "/api/user/logout",
                json!({"token": token.clone()}),
                Some(&token),
            )?)
            .await?;
        ensure!(logout.status() == StatusCode::OK);

        let after_logout = router
            .clone()
            .oneshot(json_request(
                "/api/question/add",
                json!({"title": "title", "content": "content", "username": "alice"}),
                Some(&token),
            )?)
            .await?;
        ensure!(after_logout.status() == StatusCode::UNAUTHORIZED);
        Ok(())
    }
    .await;

    let _shutdown_result = shutdown_sender.send(());
    server.await.context("gRPC server task must not panic")??;
    clear_redis_database(&redis)?;
    database.cleanup().await?;
    result
}
