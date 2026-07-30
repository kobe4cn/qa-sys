use axum::{
    Form, Json, RequestExt,
    extract::{
        FromRequest,
        rejection::{FormRejection, JsonRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};
use validator::Validate;

use crate::handler::Reply;

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonOrForm<T>(pub T);

#[derive(Debug)]
pub struct JsonOrFormRejection {
    status: StatusCode,
    validation_message: Option<String>,
}

impl JsonOrFormRejection {
    fn bad_request() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            validation_message: None,
        }
    }

    fn validation(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            validation_message: Some(message),
        }
    }

    fn unsupported_media_type() -> Self {
        Self {
            status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            validation_message: None,
        }
    }

    #[cfg(test)]
    fn status(&self) -> StatusCode {
        self.status
    }
}

impl IntoResponse for JsonOrFormRejection {
    fn into_response(self) -> Response {
        self.validation_message.map_or_else(
            || self.status.into_response(),
            |msg| {
                (
                    self.status,
                    Json(Reply::<()> {
                        code: 400,
                        msg,
                        data: None,
                    }),
                )
                    .into_response()
            },
        )
    }
}

impl<B, T> FromRequest<B> for JsonOrForm<T>
where
    B: Send + Sync,
    T: DeserializeOwned + Serialize + Validate + 'static,
    Json<T>: FromRequest<B, Rejection = JsonRejection>,
    Form<T>: FromRequest<B, Rejection = FormRejection>,
{
    type Rejection = JsonOrFormRejection;

    async fn from_request(req: axum::extract::Request, state: &B) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get("Content-Type")
            .and_then(|value| value.to_str().ok());
        match content_type {
            Some(value) if is_json_content_type(value) => {
                let Json(payload) = req
                    .extract_with_state(state)
                    .await
                    .map_err(|_| JsonOrFormRejection::bad_request())?;
                payload
                    .validate()
                    .map_err(|error| JsonOrFormRejection::validation(error.to_string()))?;
                Ok(Self(payload))
            }
            Some(value) if is_form_content_type(value) => {
                let Form(payload) = req
                    .extract_with_state(state)
                    .await
                    .map_err(|_| JsonOrFormRejection::bad_request())?;
                payload
                    .validate()
                    .map_err(|error| JsonOrFormRejection::validation(error.to_string()))?;
                Ok(Self(payload))
            }
            _ => Err(JsonOrFormRejection::unsupported_media_type()),
        }
    }
}

fn content_type_essence(content_type: &str) -> &str {
    content_type
        .split_once(';')
        .map_or(content_type, |(essence, _)| essence)
        .trim()
}

fn is_json_content_type(content_type: &str) -> bool {
    let essence = content_type_essence(content_type);
    let Some((media_type, subtype)) = essence.split_once('/') else {
        return false;
    };

    media_type.eq_ignore_ascii_case("application")
        && (subtype.eq_ignore_ascii_case("json")
            || subtype
                .rsplit_once('+')
                .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case("json")))
}

fn is_form_content_type(content_type: &str) -> bool {
    content_type_essence(content_type).eq_ignore_ascii_case("application/x-www-form-urlencoded")
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::FromRequest,
        http::{Request, StatusCode, header::CONTENT_TYPE},
    };
    use serde::{Deserialize, Serialize};
    use validator::Validate;

    use super::JsonOrForm;

    #[derive(Debug, Deserialize, Serialize, Validate)]
    struct TestPayload {
        #[validate(length(min = 1, max = 8))]
        name: String,
    }

    fn request(content_type: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .header(CONTENT_TYPE, content_type)
            .body(Body::from(body.to_string()))
            .expect("test request must be valid")
    }

    #[tokio::test]
    async fn test_should_extract_valid_json_and_form_payloads() {
        let json = JsonOrForm::<TestPayload>::from_request(
            request("application/json", r#"{"name":"alice"}"#),
            &(),
        )
        .await;
        let form = JsonOrForm::<TestPayload>::from_request(
            request("application/x-www-form-urlencoded", "name=alice"),
            &(),
        )
        .await;

        assert!(matches!(json, Ok(JsonOrForm(payload)) if payload.name == "alice"));
        assert!(matches!(form, Ok(JsonOrForm(payload)) if payload.name == "alice"));
    }

    #[tokio::test]
    async fn test_should_accept_json_content_type_with_charset() {
        let result = JsonOrForm::<TestPayload>::from_request(
            request("application/json; charset=utf-8", r#"{"name":"alice"}"#),
            &(),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_should_reject_malformed_payload_with_bad_request() {
        let result = JsonOrForm::<TestPayload>::from_request(
            request("application/json", r#"{"name":"#),
            &(),
        )
        .await;

        assert!(matches!(result, Err(rejection) if rejection.status() == StatusCode::BAD_REQUEST));
    }

    #[tokio::test]
    async fn test_should_reject_validation_failure_with_bad_request() {
        let result = JsonOrForm::<TestPayload>::from_request(
            request("application/json", r#"{"name":""}"#),
            &(),
        )
        .await;

        assert!(matches!(result, Err(rejection) if rejection.status() == StatusCode::BAD_REQUEST));
    }

    #[tokio::test]
    async fn test_should_reject_unsupported_content_type() {
        let result =
            JsonOrForm::<TestPayload>::from_request(request("text/plain", "name=alice"), &()).await;

        assert!(matches!(
            result,
            Err(rejection) if rejection.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE
        ));
    }
}
