use std::sync::Arc;

use anyhow::Result;
use autometrics::{
    autometrics,
    objectives::{Objective, ObjectiveLatency, ObjectivePercentile},
};
use axum::{
    Json,
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::NaiveDateTime;
use pb::{
    AddAnswerRequest, AddQuestionRequest, AnswerDetailRequest, AnswerListRequest,
    DeleteAnswerRequest, DeleteQuestionRequest, LatestQuestionRequest, QuestionDetailRequest,
    UpdateAnswerRequest, UpdateQuestionRequest, UserLoginRequest, UserLogoutRequest,
    UserRegisterRequest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use validator::{Validate, ValidationError};

use crate::{
    config::app::AppState, handler::json_or_form::JsonOrForm, router::AuthenticatedPrincipal,
};

const API_SLO: Objective = Objective::new("gateway")
    // We expect 99.9% of all requests to succeed.
    .success_rate(ObjectivePercentile::P99_9)
    // We expect 99% of all latencies to be below 750ms.
    .latency(ObjectiveLatency::Ms1000, ObjectivePercentile::P99);

#[derive(Debug, Error)]
#[allow(unused)]
pub enum GatewayError {
    #[error("Internal server error")]
    InternalServerError,
    #[error("Bad request")]
    BadRequest,
    #[error("Not found")]
    NotFound,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Forbidden")]
    Forbidden,
    #[error("Validation error")]
    ValidationError(String),
    #[error("Grpc error: {0}")]
    GrpcError(#[from] tonic::Status),
}

impl From<anyhow::Error> for GatewayError {
    fn from(_: anyhow::Error) -> Self {
        Self::InternalServerError
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            Self::BadRequest => (StatusCode::BAD_REQUEST, "Bad request".to_string()),
            Self::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            Self::Forbidden => (StatusCode::FORBIDDEN, "Forbidden".to_string()),
            Self::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::GrpcError(status) => match status.code() {
                tonic::Code::InvalidArgument => {
                    (StatusCode::BAD_REQUEST, status.message().to_string())
                }
                tonic::Code::NotFound => (StatusCode::NOT_FOUND, status.message().to_string()),
                tonic::Code::Unauthenticated => {
                    (StatusCode::UNAUTHORIZED, status.message().to_string())
                }
                tonic::Code::PermissionDenied => {
                    (StatusCode::FORBIDDEN, status.message().to_string())
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    status.message().to_string(),
                ),
            },
        };
        (status, msg).into_response()
    }
}

#[autometrics(objective = API_SLO)]
pub async fn root() -> Result<impl IntoResponse, GatewayError> {
    Ok("API gateway root!".to_string())
}

#[autometrics(objective = API_SLO)]
pub async fn hello() -> Result<impl IntoResponse, GatewayError> {
    Ok("Qa-svc!".to_string())
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(max = 32, message = "username invalid"))]
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    pub username: String,
    #[validate(length(min = 6, max = 32, message = "password invalid"))]
    #[validate(custom(function = "validate_required", message = "password is empty"))]
    pub password: String,
    #[validate(email(message = "email invalid"))]
    #[validate(custom(function = "validate_required", message = "email is empty"))]
    pub email: String,
    #[validate(length(min = 8, max = 11, message = "phone invalid"))]
    #[validate(custom(function = "validate_required", message = "phone is empty"))]
    pub phone: String,
}
fn validate_required(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::new("username is empty"));
    }
    Ok(())
}

fn validate_vote_action(value: &str) -> Result<(), ValidationError> {
    if matches!(value, "up" | "down") {
        return Ok(());
    }

    Err(ValidationError::new("action invalid"))
}

fn authenticated_username(
    principal: &AuthenticatedPrincipal,
    claimed_username: &str,
) -> Result<String, GatewayError> {
    if principal.username != claimed_username {
        return Err(GatewayError::Forbidden);
    }

    Ok(principal.username.clone())
}

fn authenticated_token(
    principal: &AuthenticatedPrincipal,
    claimed_token: &str,
) -> Result<String, GatewayError> {
    if principal.token != claimed_token {
        return Err(GatewayError::Forbidden);
    }

    Ok(principal.token.clone())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterReply {
    pub state: i64,
}
#[autometrics(objective = API_SLO)]
pub async fn user_register(
    State(state): State<Arc<AppState>>,
    JsonOrForm(payload): JsonOrForm<RegisterRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let req = tonic::Request::new(UserRegisterRequest {
        username: payload.username,
        password: payload.password,
        email: payload.email,
        phone: payload.phone,
    });
    let resp = state.grpc_client.clone().user_register(req).await?;
    let reply = RegisterReply {
        state: resp.into_inner().state,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(max = 32, message = "username invalid"))]
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    pub username: String,
    #[validate(length(min = 6, max = 32, message = "password invalid"))]
    #[validate(custom(function = "validate_required", message = "password is empty"))]
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginReply {
    pub token: String,
}
#[autometrics(objective = API_SLO)]
pub async fn user_login(
    State(state): State<Arc<AppState>>,
    JsonOrForm(payload): JsonOrForm<LoginRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let req = tonic::Request::new(UserLoginRequest {
        username: payload.username,
        password: payload.password,
    });
    let resp = state.grpc_client.clone().user_login(req).await?;
    let reply = LoginReply {
        token: resp.into_inner().token,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct LogoutRequest {
    #[validate(length(max = 512, message = "token invalid"))]
    #[validate(custom(function = "validate_required", message = "token is empty"))]
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogoutReply {
    pub state: i64,
}
#[autometrics(objective = API_SLO)]
pub async fn user_logout(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<LogoutRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let token = authenticated_token(&principal, &payload.token)?;
    let req = tonic::Request::new(UserLogoutRequest { token });
    let resp = state.grpc_client.clone().user_logout(req).await?;
    let reply = LogoutReply {
        state: resp.into_inner().state,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuestionDetailResponse {
    pub question: Option<QuestionEntity>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct QuestionEntity {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub read_count: i64,
    pub reply_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct QuestionRequest {
    #[validate(range(min = 1, message = "question_id invalid"))]
    pub question_id: u64,
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    pub username: String,
}

#[autometrics(objective = API_SLO)]
pub async fn question_detail(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<QuestionRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(QuestionDetailRequest {
        id: payload.question_id,
        username,
    });
    let resp = state.grpc_client.clone().question_detail(req).await?;
    let reply = QuestionDetailResponse {
        question: resp.into_inner().question.map(|q| QuestionEntity {
            id: q.id as i64,
            title: q.title,
            content: q.content,
            created_by: q.create_by,
            read_count: q.read_count as i64,
            reply_count: q.reply_count as i64,
            ..Default::default()
        }),
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddQuestRequest {
    #[validate(custom(function = "validate_required", message = "title is empty"))]
    #[validate(length(min = 1, max = 512, message = "title invalid"))]
    pub title: String,
    #[validate(custom(function = "validate_required", message = "content is empty"))]
    #[validate(length(min = 1, max = 4096, message = "content invalid"))]
    pub content: String,
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddQuesReply {
    pub question_id: i64,
}

#[autometrics(objective = API_SLO)]
pub async fn question_add(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<AddQuestRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(AddQuestionRequest {
        title: payload.title,
        content: payload.content,
        create_by: username,
    });
    let resp = state.grpc_client.clone().add_question(req).await?;
    let reply = AddQuesReply {
        question_id: resp.into_inner().id as i64,
    };
    Ok(Json(reply))
}
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateQuestRequest {
    #[validate(range(min = 1, message = "id invalid"))]
    pub id: u64,
    pub question: QuestionEntity,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateQuestReply {
    pub state: i64,
}
#[autometrics(objective=API_SLO)]
pub async fn question_update(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<UpdateQuestRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.question.updated_by)?;
    let req = tonic::Request::new(UpdateQuestionRequest {
        id: payload.id,
        title: payload.question.title,
        content: payload.question.content,
        update_by: username,
    });
    let resp = state.grpc_client.clone().update_question(req).await?;
    let reply = UpdateQuestReply {
        state: resp.into_inner().state,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeleteQuestRequest {
    #[validate(range(min = 1, message = "id invalid"))]
    pub id: u64,
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    #[validate(length(min = 1, max = 32, message = "username invalid"))]
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteQuestReply {
    pub state: i64,
}

#[autometrics(objective=API_SLO)]
pub async fn question_delete(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<DeleteQuestRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(DeleteQuestionRequest {
        id: payload.id,
        username,
    });
    let resp = state.grpc_client.clone().delete_question(req).await?;
    let reply = DeleteQuestReply {
        state: resp.into_inner().state,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct LastestQuestRequest {
    #[validate(range(min = 0, message = "last_id invalid"))]
    pub last_id: u64,
    #[validate(range(min = 1, max = 100, message = "limit invalid"))]
    pub limit: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LastestQuestReply {
    pub questions: Vec<QuestionEntity>,
    pub last_id: u64,
    pub is_end: bool,
}

#[autometrics(objective=API_SLO)]
pub async fn question_find_latest(
    State(state): State<Arc<AppState>>,
    JsonOrForm(payload): JsonOrForm<LastestQuestRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let req = tonic::Request::new(LatestQuestionRequest {
        last_id: payload.last_id,
        limit: payload.limit,
    });
    let resp = state.grpc_client.clone().latest_question(req).await?;
    let respclone = resp.into_inner().clone();
    let reply = LastestQuestReply {
        questions: respclone
            .questions
            .iter()
            .map(|q| QuestionEntity {
                id: q.id as i64,
                title: q.title.clone(),
                content: q.content.clone(),
                created_by: q.create_by.clone(),
                read_count: q.read_count as i64,
                reply_count: q.reply_count as i64,
                ..Default::default()
            })
            .collect(),
        last_id: respclone.last_id,
        is_end: respclone.is_end,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AnswerObject {
    pub id: i64,
    pub question_id: i64,
    pub content: String,
    pub created_by: String,
    pub updated_by: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub agree_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AnswerAddRequest {
    #[validate(custom(function = "validate_required", message = "content is empty"))]
    #[validate(length(min = 1, max = 1024, message = "content invalid"))]
    pub content: String,
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    #[validate(length(min = 1, max = 32, message = "username invalid"))]
    pub username: String,
    #[validate(range(min = 1, message = "question_id invalid"))]
    pub question_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddAnswerReply {
    pub answer_id: i64,
}

#[autometrics(objective=API_SLO)]
pub async fn answer_add(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<AnswerAddRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(AddAnswerRequest {
        answer: Some(pb::AnswerEntity {
            question_id: payload.question_id,
            content: payload.content,
            create_by: username,
            ..Default::default()
        }),
    });
    let resp = state.grpc_client.clone().add_answer(req).await?;
    let reply = AddAnswerReply {
        answer_id: resp.into_inner().id as i64,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AnswerDeleteRequest {
    #[validate(range(min = 1, message = "id invalid"))]
    pub id: u64,
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    #[validate(length(min = 1, max = 32, message = "username invalid"))]
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerDeleteReply {
    pub state: i64,
}

#[autometrics(objective=API_SLO)]
pub async fn answer_delete(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<AnswerDeleteRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(DeleteAnswerRequest {
        id: payload.id,
        username,
    });
    let resp = state.grpc_client.clone().delete_answer(req).await?;
    let reply = AnswerDeleteReply {
        state: resp.into_inner().state,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AnswerUpdateRequest {
    #[validate(range(min = 1, message = "id invalid"))]
    pub id: u64,
    #[validate(custom(function = "validate_required", message = "content is empty"))]
    #[validate(length(min = 1, max = 1024, message = "content invalid"))]
    pub content: String,
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    #[validate(length(min = 1, max = 32, message = "username invalid"))]
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerUpdateReply {
    pub state: i64,
}

#[autometrics(objective=API_SLO)]
pub async fn answer_update(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<AnswerUpdateRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(UpdateAnswerRequest {
        id: payload.id,
        content: payload.content,
        username,
    });
    let resp = state.grpc_client.clone().update_answer(req).await?;
    let reply = AnswerUpdateReply {
        state: resp.into_inner().state,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct FindAnswerRequest {
    #[validate(range(min = 1, message = "id invalid"))]
    pub id: u64,
    #[validate(custom(function = "validate_required", message = "username is empty"))]
    #[validate(length(min = 1, max = 32, message = "username invalid"))]
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FindAnswerReply {
    pub answer: Option<AnswerObject>,
}

#[autometrics(objective=API_SLO)]
pub async fn answer_find_one(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<FindAnswerRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(AnswerDetailRequest {
        id: payload.id,
        username,
    });
    let resp = state.grpc_client.clone().answer_detail(req).await?;
    let reply = FindAnswerReply {
        answer: resp.into_inner().answer.map(|a| AnswerObject {
            id: a.id as i64,
            question_id: a.question_id as i64,
            content: a.content.clone(),
            created_by: a.create_by.clone(),
            agree_count: a.agree_count as i64,
            ..Default::default()
        }),
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AnswerLastestRequest {
    #[validate(range(min = 1, message = "question_id invalid"))]
    pub question_id: u64,
    #[validate(range(min = 1, message = "page invalid"))]
    pub page: u64,
    #[validate(range(min = 1, max = 100, message = "limit invalid"))]
    pub limit: u64,
    #[validate(custom(function = "validate_required", message = "sort is empty"))]
    #[validate(length(min = 1, max = 32, message = "sort invalid"))]
    pub username: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerLatestResponse {
    pub answers: Vec<AnswerObject>,
    pub total: i64,
    pub total_page: i64,
    pub page_size: i64,
    pub current_page: i64,
    pub is_end: bool,
}

#[autometrics(objective=API_SLO)]
pub async fn answer_find_list(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<AnswerLastestRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(AnswerListRequest {
        question_id: payload.question_id,
        page: payload.page,
        limit: payload.limit,
        username,
    });
    let resp = state.grpc_client.clone().answer_list(req).await?;
    let respclone = resp.into_inner();
    let reply = AnswerLatestResponse {
        answers: respclone
            .answers
            .iter()
            .map(|a| AnswerObject {
                id: a.id as i64,
                question_id: a.question_id as i64,
                content: a.content.clone(),
                created_by: a.create_by.clone(),
                agree_count: a.agree_count as i64,
                ..Default::default()
            })
            .collect(),
        total: respclone.total as i64,
        total_page: respclone.total_page as i64,
        page_size: respclone.page_size as i64,
        current_page: respclone.current_page as i64,
        is_end: respclone.is_end,
    };
    Ok(Json(reply))
}

#[derive(Debug, Serialize, Deserialize, Validate)]

pub struct AnswerAgreeRequest {
    #[validate(range(min = 1, message = "id invalid"))]
    pub id: u64,
    #[validate(custom(function = "validate_required", message = "create_by is empty"))]
    #[validate(length(min = 1, max = 32, message = "create_by invalid"))]
    pub username: String,
    #[validate(custom(function = "validate_required", message = "action is empty"))]
    #[validate(length(min = 1, max = 32, message = "action invalid"))]
    #[validate(custom(function = "validate_vote_action", message = "action invalid"))]
    pub action: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerAgreeReply {
    pub state: i64,
}

#[autometrics(objective = API_SLO)]
pub async fn answer_agree(
    State(state): State<Arc<AppState>>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    JsonOrForm(payload): JsonOrForm<AnswerAgreeRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let username = authenticated_username(&principal, &payload.username)?;
    let req = tonic::Request::new(pb::AgreeAnswerRequest {
        id: payload.id,
        create_by: username,
        action: payload.action,
    });
    let resp = state.grpc_client.clone().agree_answer(req).await?;
    let reply = AnswerAgreeReply {
        state: resp.into_inner().state,
    };
    Ok(Json(reply))
}

#[cfg(test)]
mod tests {
    use axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    };
    use tonic::{Code, Status};
    use validator::Validate;

    use super::{
        AnswerAgreeRequest, AnswerLastestRequest, GatewayError, LastestQuestRequest, LoginRequest,
        RegisterRequest,
    };

    fn response_status(error: GatewayError) -> StatusCode {
        let response: Response = error.into_response();
        response.status()
    }

    #[test]
    fn test_should_map_gateway_errors_to_http_statuses() {
        for (error, expected) in [
            (
                GatewayError::InternalServerError,
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (GatewayError::BadRequest, StatusCode::BAD_REQUEST),
            (GatewayError::NotFound, StatusCode::NOT_FOUND),
            (GatewayError::Unauthorized, StatusCode::UNAUTHORIZED),
            (GatewayError::Forbidden, StatusCode::FORBIDDEN),
            (
                GatewayError::ValidationError("invalid".to_string()),
                StatusCode::BAD_REQUEST,
            ),
        ] {
            assert_eq!(response_status(error), expected);
        }
    }

    #[test]
    fn test_should_map_grpc_errors_to_http_statuses() {
        for (code, expected) in [
            (Code::InvalidArgument, StatusCode::BAD_REQUEST),
            (Code::NotFound, StatusCode::NOT_FOUND),
            (Code::Unauthenticated, StatusCode::UNAUTHORIZED),
            (Code::PermissionDenied, StatusCode::FORBIDDEN),
            (Code::Unavailable, StatusCode::INTERNAL_SERVER_ERROR),
        ] {
            let error = GatewayError::GrpcError(Status::new(code, "rejected"));
            assert_eq!(response_status(error), expected);
        }
    }

    #[test]
    fn test_should_accept_valid_request_boundaries() {
        let register = RegisterRequest {
            username: "a".repeat(32),
            password: "p".repeat(32),
            email: "alice@example.com".to_string(),
            phone: "1".repeat(11),
        };
        let latest_question = LastestQuestRequest {
            last_id: 0,
            limit: 100,
        };

        assert!(register.validate().is_ok());
        assert!(latest_question.validate().is_ok());
    }

    #[test]
    fn test_should_reject_invalid_registration_and_login_fields() {
        let register = RegisterRequest {
            username: String::new(),
            password: "short".to_string(),
            email: "invalid".to_string(),
            phone: "123".to_string(),
        };
        let login = LoginRequest {
            username: "a".repeat(33),
            password: "p".repeat(33),
        };

        assert!(register.validate().is_err());
        assert!(login.validate().is_err());
    }

    #[test]
    fn test_should_reject_zero_answer_page() {
        let request = AnswerLastestRequest {
            question_id: 1,
            page: 0,
            limit: 10,
            username: "alice".to_string(),
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_should_reject_unsupported_vote_action() {
        let request = AnswerAgreeRequest {
            id: 1,
            username: "alice".to_string(),
            action: "sideways".to_string(),
        };

        assert!(request.validate().is_err());
    }
}
