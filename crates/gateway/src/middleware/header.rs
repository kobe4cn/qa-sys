use anyhow::Result;
use axum::{
    extract::Request,
    http::{
        HeaderValue,
        header::{CACHE_CONTROL, EXPIRES, PRAGMA},
    },
    middleware::Next,
    response::IntoResponse,
};

use crate::handler::qa::GatewayError;
pub async fn no_cache_header(req: Request, next: Next) -> Result<impl IntoResponse, GatewayError> {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
    response
        .headers_mut()
        .insert(EXPIRES, HeaderValue::from_static("-1"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware::from_fn,
        routing::get,
    };
    use tower::ServiceExt;

    use super::no_cache_header;

    #[tokio::test]
    async fn test_should_add_no_cache_headers_to_success_and_error_responses() {
        for (path, expected_status) in [
            ("/success", StatusCode::OK),
            ("/error", StatusCode::BAD_REQUEST),
        ] {
            let app = Router::new()
                .route("/success", get(|| async { StatusCode::OK }))
                .route("/error", get(|| async { StatusCode::BAD_REQUEST }))
                .layer(from_fn(no_cache_header));
            let response = app
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(Body::empty())
                        .expect("test request must be valid"),
                )
                .await
                .expect("router response must succeed");

            assert_eq!(response.status(), expected_status);
            assert_eq!(
                response.headers().get("cache-control"),
                Some(
                    &"no-store, no-cache, must-revalidate"
                        .parse()
                        .expect("static header must be valid")
                )
            );
            assert_eq!(
                response.headers().get("pragma"),
                Some(&"no-cache".parse().expect("static header must be valid"))
            );
            assert_eq!(
                response.headers().get("expires"),
                Some(&"-1".parse().expect("static header must be valid"))
            );
        }
    }
}
