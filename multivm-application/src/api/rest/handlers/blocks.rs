//! # Block Handlers

use super::{calculate_response_time, start_request_timer, success_response};
use crate::ApplicationState;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;

/// List blocks
pub async fn list_blocks(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(Vec::<String>::new(), request_id, response_time).into_response()
}

/// Get block details
pub async fn get_block_details(
    State(_state): State<Arc<ApplicationState>>,
    Path(_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(serde_json::json!({}), request_id, response_time).into_response()
}

/// Search blocks
pub async fn search_blocks(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    Json(_request): Json<serde_json::Value>,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(Vec::<String>::new(), request_id, response_time).into_response()
}

/// Get block stats
pub async fn get_block_stats(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(serde_json::json!({}), request_id, response_time).into_response()
}
