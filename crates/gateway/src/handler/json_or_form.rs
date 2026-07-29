use axum::extract::rejection::{FormRejection, JsonRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Form, RequestExt};
use axum::{Json, extract::FromRequest};
use serde::Serialize;
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::handler::Reply;

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonOrForm<T>(pub T);

impl<B, T> FromRequest<B> for JsonOrForm<T>
where
    B: Send + Sync,
    T: DeserializeOwned + Serialize + Validate + 'static,
    Json<T>: FromRequest<B, Rejection = JsonRejection>,
    Form<T>: FromRequest<B, Rejection = FormRejection>,
{
    type Rejection = Response;

    async fn from_request(req: axum::extract::Request, state: &B) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get("Content-Type")
            .map(|val| val.to_str().unwrap_or(""));
        match content_type {
            Some("application/json") => {
                let Json(payload) = req
                    .extract_with_state(state)
                    .await
                    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
                payload.validate().map_or_else(
                    |e| {
                        Err((
                            StatusCode::OK,
                            Json(Reply::<()> {
                                code: 400,
                                msg: e.to_string(),
                                data: None,
                            }),
                        )
                            .into_response())
                    },
                    |()| Ok(Self(payload)),
                )
            }
            Some("application/x-www-form-urlencoded") => {
                let Form(payload) = req
                    .extract_with_state(state)
                    .await
                    .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;
                payload.validate().map_or_else(
                    |e| {
                        Err((
                            StatusCode::OK,
                            Json(Reply::<()> {
                                code: 400,
                                msg: e.to_string(),
                                data: None,
                            }),
                        )
                            .into_response())
                    },
                    |()| Ok(Self(payload)),
                )
            }
            _ => Err(axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response()),
        }
    }
}
