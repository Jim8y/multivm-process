//! # Account Handlers

use super::{calculate_response_time, start_request_timer, success_response};
use crate::ApplicationState;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;

/// List accounts
pub async fn list_accounts(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(Vec::<String>::new(), request_id, response_time).into_response()
}

/// Get account info
pub async fn get_account_info(
    State(_state): State<Arc<ApplicationState>>,
    Path(_address): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(serde_json::json!({}), request_id, response_time).into_response()
}

/// Get account history
pub async fn get_account_history(
    State(_state): State<Arc<ApplicationState>>,
    Path(_address): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(Vec::<String>::new(), request_id, response_time).into_response()
}

/// Search accounts
pub async fn search_accounts(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    Json(_request): Json<serde_json::Value>,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(Vec::<String>::new(), request_id, response_time).into_response()
}
