//! Health check alerting and webhook notifications
//!
//! This module provides alerting capabilities for health check failures,
//! including webhook notifications, email alerts, and metric-based alerts.

use crate::error::ApplicationResult;
use crate::monitoring::health_checks::{ComponentHealth, FullHealthReport};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational alert
    Info,
    /// Warning alert - degraded performance
    Warning,
    /// Critical alert - service failure
    Critical,
    /// Emergency alert - complete system failure
    Emergency,
}

/// Alert configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AlertConfig {
    /// Enable alerting
    pub enabled: bool,

    /// Webhook endpoints for alerts
    pub webhooks: Vec<WebhookConfig>,

    /// Email configuration
    pub email: Option<EmailConfig>,

    /// Alert thresholds
    pub thresholds: AlertThresholds,

    /// Alert cooldown period (seconds)
    pub cooldown_seconds: u64,
}

/// Webhook configuration
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,

    /// Webhook secret for authentication
    pub secret: Option<String>,

    /// Minimum severity to trigger webhook
    pub min_severity: AlertSeverity,

    /// Request timeout (seconds)
    pub timeout_seconds: u64,
}

/// Email alert configuration
#[derive(Debug, Clone, Deserialize)]
pub struct EmailConfig {
    /// SMTP server
    pub smtp_server: String,

    /// SMTP port
    pub smtp_port: u16,

    /// Username
    pub username: String,

    /// Password (should be from environment)
    pub password: String,

    /// From address
    pub from: String,

    /// To addresses
    pub to: Vec<String>,

    /// Minimum severity for email alerts
    pub min_severity: AlertSeverity,
}

/// Alert thresholds
#[derive(Debug, Clone, Deserialize)]
pub struct AlertThresholds {
    /// Consecutive failures before alerting
    pub consecutive_failures: u32,

    /// Error rate threshold (0.0 - 1.0)
    pub error_rate: f64,

    /// Response time threshold (ms)
    pub response_time_ms: u64,

    /// CPU usage threshold (%)
    pub cpu_threshold_percent: f64,

    /// Memory usage threshold (%)
    pub memory_threshold_percent: f64,
}

/// Alert manager for health checks
pub struct AlertManager {
    config: AlertConfig,
    alert_history: Arc<RwLock<AlertHistory>>,
    http_client: reqwest::Client,
}

/// Alert history tracking
struct AlertHistory {
    /// Last alert time by component
    last_alerts: std::collections::HashMap<String, Instant>,

    /// Consecutive failure counts
    failure_counts: std::collections::HashMap<String, u32>,

    /// Recent alerts
    recent_alerts: Vec<Alert>,
}

/// Alert information
#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    /// Alert ID
    pub id: String,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Severity
    pub severity: AlertSeverity,

    /// Component name
    pub component: String,

    /// Alert title
    pub title: String,

    /// Alert description
    pub description: String,

    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            webhooks: vec![],
            email: None,
            thresholds: AlertThresholds {
                consecutive_failures: 3,
                error_rate: 0.1,
                response_time_ms: 5000,
                cpu_threshold_percent: 90.0,
                memory_threshold_percent: 90.0,
            },
            cooldown_seconds: 300, // 5 minutes
        }
    }
}

impl AlertManager {
    /// Create new alert manager
    pub fn new(config: AlertConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            config,
            alert_history: Arc::new(RwLock::new(AlertHistory {
                last_alerts: std::collections::HashMap::new(),
                failure_counts: std::collections::HashMap::new(),
                recent_alerts: Vec::new(),
            })),
            http_client,
        }
    }

    /// Process health report and generate alerts
    pub async fn process_health_report(&self, report: &FullHealthReport) -> ApplicationResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut alerts_to_send = Vec::new();

        // Check each component
        for component in &report.checks {
            if !component.healthy {
                let alert = self.check_component_alert(component).await?;
                if let Some(alert) = alert {
                    alerts_to_send.push(alert);
                }
            } else {
                // Reset failure count for healthy components
                self.reset_failure_count(&component.name).await;
            }
        }

        // Send alerts
        for alert in alerts_to_send {
            self.send_alert(alert).await?;
        }

        Ok(())
    }

    /// Check if component failure should trigger alert
    async fn check_component_alert(
        &self,
        component: &ComponentHealth,
    ) -> ApplicationResult<Option<Alert>> {
        let mut history = self.alert_history.write().await;

        // Increment failure count
        let failure_count = {
            let count = history
                .failure_counts
                .entry(component.name.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            *count
        };

        // Check if we should alert
        if failure_count < self.config.thresholds.consecutive_failures {
            return Ok(None);
        }

        // Check cooldown
        if let Some(last_alert) = history.last_alerts.get(&component.name) {
            if last_alert.elapsed() < Duration::from_secs(self.config.cooldown_seconds) {
                return Ok(None);
            }
        }

        // Create alert
        let severity = match component.name.as_str() {
            "consensus" | "process_manager" => AlertSeverity::Critical,
            "database" | "cache" => AlertSeverity::Warning,
            _ => AlertSeverity::Warning,
        };

        let alert = Alert {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            severity,
            component: component.name.clone(),
            title: format!("{} Health Check Failed", component.name),
            description: component.message.clone(),
            metadata: serde_json::json!({
                "consecutive_failures": failure_count,
                "healthy": component.healthy,
            }),
        };

        // Update last alert time
        history
            .last_alerts
            .insert(component.name.clone(), Instant::now());

        // Add to recent alerts
        history.recent_alerts.push(alert.clone());
        if history.recent_alerts.len() > 100 {
            history.recent_alerts.remove(0);
        }

        Ok(Some(alert))
    }

    /// Reset failure count for component
    async fn reset_failure_count(&self, component: &str) {
        let mut history = self.alert_history.write().await;
        history.failure_counts.remove(component);
    }

    /// Send alert through configured channels
    async fn send_alert(&self, alert: Alert) -> ApplicationResult<()> {
        info!("Sending alert: {} - {}", alert.component, alert.title);

        // Send webhooks
        for webhook in &self.config.webhooks {
            if alert.severity as u8 >= webhook.min_severity as u8 {
                if let Err(e) = self.send_webhook(webhook, &alert).await {
                    error!("Failed to send webhook alert: {}", e);
                }
            }
        }

        // Send email if configured
        if let Some(email_config) = &self.config.email {
            if alert.severity as u8 >= email_config.min_severity as u8 {
                if let Err(e) = self.send_email(email_config, &alert).await {
                    error!("Failed to send email alert: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Send webhook notification
    async fn send_webhook(&self, webhook: &WebhookConfig, alert: &Alert) -> ApplicationResult<()> {
        let payload = serde_json::json!({
            "alert": alert,
            "source": "multivm_health_check",
            "timestamp": chrono::Utc::now(),
        });

        let mut request = self
            .http_client
            .post(&webhook.url)
            .json(&payload)
            .timeout(Duration::from_secs(webhook.timeout_seconds));

        // Add authentication if configured
        if let Some(secret) = &webhook.secret {
            let signature = self.calculate_webhook_signature(&payload, secret)?;
            request = request.header("X-Webhook-Signature", signature);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            warn!("Webhook returned non-success status: {}", response.status());
        }

        Ok(())
    }

    /// Calculate webhook signature for authentication
    fn calculate_webhook_signature(
        &self,
        payload: &serde_json::Value,
        secret: &str,
    ) -> ApplicationResult<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let payload_str = serde_json::to_string(payload)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|e| {
            crate::ApplicationError::InternalError {
                component: "webhook_auth".to_string(),
                message: e.to_string(),
            }
        })?;

        mac.update(payload_str.as_bytes());
        let result = mac.finalize();

        Ok(hex::encode(result.into_bytes()))
    }

    /// Send email notification (placeholder)
    async fn send_email(&self, _config: &EmailConfig, alert: &Alert) -> ApplicationResult<()> {
        // In a real implementation, would use lettre or similar
        warn!(
            "Email alerts not yet implemented for alert: {}",
            alert.title
        );
        Ok(())
    }

    /// Get recent alerts
    pub async fn get_recent_alerts(&self, limit: usize) -> Vec<Alert> {
        let history = self.alert_history.read().await;
        let start = history.recent_alerts.len().saturating_sub(limit);
        history.recent_alerts[start..].to_vec()
    }

    /// Check if any critical alerts are active
    pub async fn has_critical_alerts(&self) -> bool {
        let history = self.alert_history.read().await;
        history
            .recent_alerts
            .iter()
            .any(|a| a.severity == AlertSeverity::Critical)
    }
}

/// Create alert for metric threshold breach
pub fn create_metric_alert(
    metric_name: &str,
    current_value: f64,
    threshold: f64,
    severity: AlertSeverity,
) -> Alert {
    Alert {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        severity,
        component: "metrics".to_string(),
        title: format!("{metric_name} Threshold Exceeded"),
        description: format!(
            "{metric_name} is {current_value:.2}, which exceeds threshold of {threshold:.2}"
        ),
        metadata: serde_json::json!({
            "metric": metric_name,
            "current_value": current_value,
            "threshold": threshold,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_alert_manager_creation() {
        let config = AlertConfig::default();
        let manager = AlertManager::new(config);
        assert_eq!(manager.get_recent_alerts(10).await.len(), 0);
    }

    #[test]
    fn test_metric_alert_creation() {
        let alert = create_metric_alert("cpu_usage", 95.5, 90.0, AlertSeverity::Warning);
        assert_eq!(alert.component, "metrics");
        assert!(alert.description.contains("95.5"));
        assert!(alert.description.contains("90.0"));
    }
}
