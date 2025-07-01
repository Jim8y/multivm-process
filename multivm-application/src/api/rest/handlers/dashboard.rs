//! Dashboard handler for real-time metrics visualization

use crate::monitoring::dashboard::DashboardService;
use crate::ApplicationState;
use axum::extract::{State, WebSocketUpgrade};
use axum::response::{Html, Response};
use std::sync::Arc;

/// Get the dashboard HTML page
pub async fn get_dashboard() -> Html<&'static str> {
    Html(DashboardService::get_dashboard_html())
}

/// WebSocket handler for real-time metrics
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ApplicationState>>,
) -> Response {
    state.monitoring.dashboard.handle_websocket(ws).await
}
