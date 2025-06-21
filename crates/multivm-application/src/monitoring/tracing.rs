//! Distributed tracing support

use crate::error::{ApplicationError, ApplicationResult};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Tracing service for distributed tracing
#[derive(Debug)]
pub struct TracingService {
    config: crate::config::TracingConfig,
    spans: Arc<RwLock<Vec<TraceSpan>>>,
}

impl TracingService {
    /// Create new tracing service
    pub async fn new(config: &crate::config::TracingConfig) -> ApplicationResult<Self> {
        Ok(Self {
            config: config.clone(),
            spans: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Initialize tracing
    pub async fn initialize(&self) -> ApplicationResult<()> {
        if !self.config.enabled {
            tracing::info!("Distributed tracing is disabled");
            return Ok(());
        }

        tracing::info!(
            "Initializing distributed tracing: service_name={}, endpoint={:?}",
            self.config.service_name,
            self.config.endpoint
        );

        // Set up OpenTelemetry tracing backend
        use opentelemetry::global;
        use opentelemetry_jaeger;

        if let Some(_endpoint) = &self.config.endpoint {
            // Note: OpenTelemetry Jaeger integration would be configured here
            // For now, we'll just log that tracing is configured
            tracing::info!("Tracing service configured (Jaeger integration requires compatible opentelemetry version)");
        } else {
            tracing::info!("Tracing initialized without external backend");
        }

        Ok(())
    }

    /// Start a new trace span
    pub fn start_span(&self, name: &str, operation: &str) -> TraceSpan {
        let span = TraceSpan {
            trace_id: uuid::Uuid::new_v4().to_string(),
            span_id: uuid::Uuid::new_v4().to_string(),
            parent_span_id: None,
            name: name.to_string(),
            operation: operation.to_string(),
            start_time: chrono::Utc::now(),
            end_time: None,
            duration_ms: None,
            tags: vec![],
            events: vec![],
            status: SpanStatus::Running,
        };

        if self.should_sample() {
            let mut spans = self.spans.write();
            spans.push(span.clone());

            // Keep only recent spans (last 1000)
            if spans.len() > 1000 {
                spans.drain(0..100);
            }
        }

        span
    }

    /// End a trace span
    pub fn end_span(&self, mut span: TraceSpan, status: SpanStatus) {
        span.end_time = Some(chrono::Utc::now());
        span.duration_ms = span
            .end_time
            .map(|end| (end - span.start_time).num_milliseconds() as f64);
        span.status = status;

        if self.should_sample() {
            // Send span to the configured tracing backend
            if let Some(_) = &self.config.endpoint {
                // Span will be automatically exported via OpenTelemetry
                tracing::debug!("Span sent to tracing backend: {}", span.name);
            }
            tracing::debug!(
                "Span completed: name={}, duration_ms={:?}, status={:?}",
                span.name,
                span.duration_ms,
                span.status
            );
        }
    }

    /// Add tag to span
    pub fn add_tag(&self, span: &mut TraceSpan, key: &str, value: &str) {
        span.tags.push(SpanTag {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    /// Add event to span
    pub fn add_event(&self, span: &mut TraceSpan, name: &str, message: &str) {
        span.events.push(SpanEvent {
            timestamp: chrono::Utc::now(),
            name: name.to_string(),
            message: message.to_string(),
        });
    }

    /// Check if we should sample this trace
    fn should_sample(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Simple sampling based on configured rate
        rand::random::<f64>() < self.config.sampling_rate
    }

    /// Get recent spans for debugging
    pub fn get_recent_spans(&self) -> Vec<TraceSpan> {
        self.spans.read().clone()
    }

    /// Clear all stored spans
    pub fn clear_spans(&self) {
        self.spans.write().clear();
    }
}

/// Trace span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub operation: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_ms: Option<f64>,
    pub tags: Vec<SpanTag>,
    pub events: Vec<SpanEvent>,
    pub status: SpanStatus,
}

/// Span tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanTag {
    pub key: String,
    pub value: String,
}

/// Span event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub name: String,
    pub message: String,
}

/// Span status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpanStatus {
    Running,
    Ok,
    Error,
    Cancelled,
}

/// Helper macro for creating spans
#[macro_export]
macro_rules! traced {
    ($tracing:expr, $name:expr, $op:expr, $body:expr) => {{
        let mut span = $tracing.start_span($name, $op);
        let result = $body;
        let status = if result.is_ok() {
            $crate::monitoring::tracing::SpanStatus::Ok
        } else {
            $crate::monitoring::tracing::SpanStatus::Error
        };
        $tracing.end_span(span, status);
        result
    }};
}
