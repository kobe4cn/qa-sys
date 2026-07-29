use std::{sync::Arc, time::Duration};

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    middleware::{Next, from_fn, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pb::{VerifyTokenRequest, VerifyTokenResponse};
use tokio::time::timeout;
use tonic::{Code, Status};
use uuid::Uuid;

use crate::{
    config::app::AppState,
    handler::{
        Reply,
        qa::{self, GatewayError},
    },
    middleware::header::no_cache_header,
};

const MAX_TOKEN_LENGTH: usize = 512;
const MAX_USERNAME_LENGTH: usize = 64;
const TOKEN_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(3);
const PUBLIC_PATHS: [&str; 5] = [
    "/",
    "/api/home",
    "/api/hello",
    "/api/user/register",
    "/api/user/login",
];

pub fn api_router(state: Arc<AppState>) -> Result<Router> {
    let api_routers = Router::new()
        .route("/home", get(qa::root))
        .route("/hello", get(qa::hello))
        .route("user/register", post(qa::user_register))
        .route("user/login", post(qa::user_login))
        .route("user/logout", post(qa::user_logout))
        .route("question/add", post(qa::question_add))
        .route("question/detail", post(qa::question_detail))
        .route("question/update", post(qa::question_update))
        .route("question/delete", post(qa::question_delete))
        .route("question/find_latest", post(qa::question_find_latest))
        .route("answer/add", post(qa::answer_add))
        .route("answer/update", post(qa::answer_update))
        .route("answer/delete", post(qa::answer_delete))
        .route("answer/find_one", post(qa::answer_find_one))
        .route("answer/find_list", post(qa::answer_find_list))
        .route("user_vote/add", post(qa::answer_agree))
        .with_state(Arc::clone(&state))
        .fallback(api_not_found);
    let router = Router::new()
        .nest("/api", api_routers)
        .route("/", get(qa::root))
        .fallback(method_not_found)
        .layer(from_fn(no_cache_header))
        .layer(from_fn_with_state(state, verify_token));

    Ok(router)
}

async fn verify_token(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, GatewayError> {
    if is_public_path(request.uri().path()) {
        return Ok(next.run(request).await);
    }

    let token = authorization_token(request.headers())?;
    let mut grpc_client = state.grpc_client.clone();
    let verification = timeout(
        TOKEN_VERIFICATION_TIMEOUT,
        grpc_client.verify_token(VerifyTokenRequest {
            token,
            request_id: Uuid::new_v4().simple().to_string(),
        }),
    )
    .await
    .map_err(|_| GatewayError::InternalServerError)?
    .map_err(map_verification_error)?
    .into_inner();
    let username = authenticated_username(verification)?;

    request.extensions_mut().insert(username);
    Ok(next.run(request).await)
}

fn is_public_path(path: &str) -> bool {
    PUBLIC_PATHS.contains(&path)
}

fn authorization_token(headers: &HeaderMap) -> Result<String, GatewayError> {
    let authorization = headers
        .get(AUTHORIZATION)
        .ok_or(GatewayError::Unauthorized)?
        .to_str()
        .map_err(|_| GatewayError::Unauthorized)?;
    let (scheme, token) = authorization
        .split_once(' ')
        .ok_or(GatewayError::Unauthorized)?;

    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.len() > MAX_TOKEN_LENGTH
        || token.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(GatewayError::Unauthorized);
    }

    Ok(token.to_string())
}

fn authenticated_username(response: VerifyTokenResponse) -> Result<String, GatewayError> {
    if response.state != 1
        || response.username.is_empty()
        || response.username.len() > MAX_USERNAME_LENGTH
    {
        return Err(GatewayError::Unauthorized);
    }

    Ok(response.username)
}

fn map_verification_error(status: Status) -> GatewayError {
    match status.code() {
        Code::InvalidArgument | Code::Unauthenticated | Code::PermissionDenied => {
            GatewayError::Unauthorized
        }
        _ => GatewayError::GrpcError(status),
    }
}

async fn api_not_found() -> Result<impl IntoResponse, GatewayError> {
    Ok((
        StatusCode::NOT_FOUND,
        Json(Reply::<()> {
            code: 404,
            msg: "api not found".to_string(),
            data: None,
        }),
    ))
}

async fn method_not_found() -> Result<impl IntoResponse, GatewayError> {
    Ok((
        StatusCode::METHOD_NOT_ALLOWED,
        Json(Reply::<()> {
            code: 405,
            msg: "method not allowed".to_string(),
            data: None,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};
    use pb::VerifyTokenResponse;
    use tonic::{Code, Status};

    use super::{
        authenticated_username, authorization_token, is_public_path, map_verification_error,
    };

    #[test]
    fn test_should_identify_only_configured_public_paths() {
        for path in [
            "/",
            "/api/home",
            "/api/hello",
            "/api/user/register",
            "/api/user/login",
        ] {
            assert!(is_public_path(path), "{path} should be public");
        }

        for path in [
            "/api/user/logout",
            "/api/question/add",
            "/api/user/login/extra",
            "/unknown",
        ] {
            assert!(
                !is_public_path(path),
                "{path} should require authentication"
            );
        }
    }

    #[test]
    fn test_should_extract_bearer_token_from_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer encrypted-token"),
        );

        assert!(matches!(
            authorization_token(&headers),
            Ok(token) if token == "encrypted-token"
        ),);
    }

    #[test]
    fn test_should_reject_missing_or_malformed_authorization_header() {
        let headers = HeaderMap::new();
        assert!(authorization_token(&headers).is_err());

        for value in [
            "",
            "Basic credentials",
            "Bearer",
            "Bearer ",
            "Bearer token with spaces",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::AUTHORIZATION,
                HeaderValue::from_str(value).expect("test header must be valid"),
            );
            assert!(
                authorization_token(&headers).is_err(),
                "{value:?} should be rejected",
            );
        }
    }

    #[test]
    fn test_should_reject_oversized_bearer_token() {
        let value = format!("Bearer {}", "a".repeat(513));
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&value).expect("test header must be valid"),
        );

        assert!(authorization_token(&headers).is_err());
    }

    #[test]
    fn test_should_accept_verified_username() {
        let response = VerifyTokenResponse {
            state: 1,
            reason: String::new(),
            username: "alice".to_string(),
        };

        assert!(matches!(
            authenticated_username(response),
            Ok(username) if username == "alice"
        ),);
    }

    #[test]
    fn test_should_reject_unsuccessful_or_empty_verification() {
        for response in [
            VerifyTokenResponse {
                state: 0,
                reason: "login session not found".to_string(),
                username: String::new(),
            },
            VerifyTokenResponse {
                state: 1,
                reason: String::new(),
                username: String::new(),
            },
        ] {
            assert!(authenticated_username(response).is_err());
        }
    }

    #[test]
    fn test_should_reject_oversized_verified_username() {
        let response = VerifyTokenResponse {
            state: 1,
            reason: String::new(),
            username: "a".repeat(65),
        };

        assert!(authenticated_username(response).is_err());
    }

    #[test]
    fn test_should_map_authentication_grpc_errors_to_unauthorized() {
        for code in [
            Code::InvalidArgument,
            Code::Unauthenticated,
            Code::PermissionDenied,
        ] {
            let error = map_verification_error(Status::new(code, "rejected"));
            assert!(matches!(
                error,
                crate::handler::qa::GatewayError::Unauthorized
            ));
        }
    }

    #[test]
    fn test_should_preserve_internal_grpc_errors() {
        let error = map_verification_error(Status::internal("unavailable"));

        assert!(matches!(
            error,
            crate::handler::qa::GatewayError::GrpcError(status)
                if status.code() == Code::Internal
        ));
    }
}
