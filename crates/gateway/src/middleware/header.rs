use std::collections::HashMap;

use anyhow::Result;
use axum::{extract::Request, http::HeaderValue, middleware::Next, response::IntoResponse};

use crate::handler::qa::GatewayError;
pub async fn no_cache_header(req: Request, next: Next) -> Result<impl IntoResponse, GatewayError> {
    let mut response = next.run(req).await;
    let mut m = HashMap::new();
    m.insert("Cache-Control", "no-store, no-cache, must-revalidate");
    m.insert("Pragma", "no-cache");
    m.insert("Expires", "-1");

    m.iter().for_each(|(k, v)| {
        response
            .headers_mut()
            .insert(*k, HeaderValue::from_str(*v).unwrap());
    });
    Ok(response)
}
