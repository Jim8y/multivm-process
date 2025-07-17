//! HTTP proxy utilities for RPC forwarding

use crate::{
    error::{RpcError, RpcResult},
    types::JsonRpcRequest,
};
use reqwest::{Client, ClientBuilder};
use std::time::Duration;
use tracing::{debug, warn};

/// HTTP proxy for forwarding requests
pub struct RpcProxy {
    client: Client,
    timeout: Duration,
    max_retries: u32,
}

impl RpcProxy {
    /// Create new RPC proxy
    pub fn new(timeout: Duration, max_retries: u32) -> RpcResult<Self> {
        let client = ClientBuilder::new()
            .timeout(timeout)
            .pool_max_idle_per_host(50)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| RpcError::ProxyError {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self {
            client,
            timeout,
            max_retries,
        })
    }

    /// Forward request to target URL
    pub async fn forward_request(
        &self,
        url: &str,
        request: &JsonRpcRequest,
        headers: Option<&reqwest::header::HeaderMap>,
    ) -> RpcResult<serde_json::Value> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.send_request(url, request, headers).await {
                Ok(response) => {
                    debug!("Request forwarded successfully on attempt {}", attempt);
                    return Ok(response);
                }
                Err(e) => {
                    warn!("Attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    // Don't retry on client errors
                    if matches!(last_error, Some(RpcError::InvalidParams { .. })) {
                        break;
                    }

                    // Wait before retry (exponential backoff)
                    if attempt < self.max_retries {
                        let delay = Duration::from_millis(100 * 2_u64.pow(attempt - 1));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| RpcError::Internal {
            message: "All retry attempts failed".to_string(),
        }))
    }

    /// Send single HTTP request
    async fn send_request(
        &self,
        url: &str,
        request: &JsonRpcRequest,
        headers: Option<&reqwest::header::HeaderMap>,
    ) -> RpcResult<serde_json::Value> {
        let mut req_builder = self.client
            .post(url)
            .json(request)
            .header("Content-Type", "application/json");

        // Add custom headers if provided
        if let Some(headers) = headers {
            for (name, value) in headers {
                req_builder = req_builder.header(name, value);
            }
        }

        // Send request
        let response = req_builder
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    RpcError::TimeoutError {
                        operation: format!("HTTP request to {}", url),
                    }
                } else if e.is_connect() {
                    RpcError::NetworkError {
                        message: format!("Connection failed to {}: {}", url, e),
                    }
                } else {
                    RpcError::NetworkError {
                        message: format!("Request failed to {}: {}", url, e),
                    }
                }
            })?;

        // Check HTTP status
        if !response.status().is_success() {
            return Err(RpcError::RelayError {
                vm_type: "unknown".to_string(),
                message: format!("HTTP {} from {}", response.status(), url),
            });
        }

        // Parse JSON response
        let json_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| RpcError::SerializationError {
                message: format!("Failed to parse JSON response from {}: {}", url, e),
            })?;

        Ok(json_response)
    }

    /// Test connection to target URL
    pub async fn test_connection(&self, url: &str) -> bool {
        let test_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "web3_clientVersion".to_string(),
            params: None,
            id: Some(serde_json::Value::Number(1.into())),
        };

        self.send_request(url, &test_request, None).await.is_ok()
    }

    /// Get proxy statistics
    pub fn get_stats(&self) -> ProxyStats {
        ProxyStats {
            timeout: self.timeout,
            max_retries: self.max_retries,
        }
    }
}

/// Proxy statistics
#[derive(Debug, Clone)]
pub struct ProxyStats {
    pub timeout: Duration,
    pub max_retries: u32,
}

/// Proxy configuration builder
pub struct ProxyBuilder {
    timeout: Duration,
    max_retries: u32,
    user_agent: Option<String>,
    default_headers: reqwest::header::HeaderMap,
}

impl ProxyBuilder {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            user_agent: None,
            default_headers: reqwest::header::HeaderMap::new(),
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    pub fn default_header(mut self, name: &str, value: &str) -> Self {
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            self.default_headers.insert(name, value);
        }
        self
    }

    pub fn build(self) -> RpcResult<RpcProxy> {
        let mut builder = ClientBuilder::new()
            .timeout(self.timeout)
            .pool_max_idle_per_host(50)
            .pool_idle_timeout(Duration::from_secs(30))
            .default_headers(self.default_headers);

        if let Some(user_agent) = self.user_agent {
            builder = builder.user_agent(user_agent);
        }

        let client = builder.build().map_err(|e| RpcError::ProxyError {
            message: format!("Failed to create HTTP client: {}", e),
        })?;

        Ok(RpcProxy {
            client,
            timeout: self.timeout,
            max_retries: self.max_retries,
        })
    }
}

impl Default for ProxyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Proxy pool for load balancing
pub struct ProxyPool {
    proxies: Vec<RpcProxy>,
    current_index: std::sync::atomic::AtomicUsize,
}

impl ProxyPool {
    pub fn new(proxies: Vec<RpcProxy>) -> Self {
        Self {
            proxies,
            current_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get next proxy using round-robin selection
    pub fn get_proxy(&self) -> Option<&RpcProxy> {
        if self.proxies.is_empty() {
            return None;
        }

        let index = self.current_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % self.proxies.len();
        
        self.proxies.get(index)
    }

    /// Forward request using available proxy
    pub async fn forward_request(
        &self,
        url: &str,
        request: &JsonRpcRequest,
        headers: Option<&reqwest::header::HeaderMap>,
    ) -> RpcResult<serde_json::Value> {
        let proxy = self.get_proxy().ok_or_else(|| RpcError::ProxyError {
            message: "No proxies available".to_string(),
        })?;

        proxy.forward_request(url, request, headers).await
    }

    /// Get pool size
    pub fn size(&self) -> usize {
        self.proxies.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_proxy_builder() {
        let proxy = ProxyBuilder::new()
            .timeout(Duration::from_secs(10))
            .max_retries(5)
            .user_agent("test-agent".to_string())
            .default_header("x-custom", "test")
            .build();

        assert!(proxy.is_ok());
        let proxy = proxy.unwrap();
        assert_eq!(proxy.timeout, Duration::from_secs(10));
        assert_eq!(proxy.max_retries, 5);
    }

    #[test]
    fn test_proxy_pool() {
        let proxy1 = ProxyBuilder::new().build().unwrap();
        let proxy2 = ProxyBuilder::new().build().unwrap();
        
        let pool = ProxyPool::new(vec![proxy1, proxy2]);
        assert_eq!(pool.size(), 2);
        
        // Test round-robin selection
        let first = pool.get_proxy();
        let second = pool.get_proxy();
        let third = pool.get_proxy();
        
        assert!(first.is_some());
        assert!(second.is_some());
        assert!(third.is_some());
    }

    #[tokio::test]
    async fn test_proxy_creation() {
        let proxy = RpcProxy::new(Duration::from_secs(5), 3);
        assert!(proxy.is_ok());
        
        let stats = proxy.unwrap().get_stats();
        assert_eq!(stats.timeout, Duration::from_secs(5));
        assert_eq!(stats.max_retries, 3);
    }

    #[test]
    fn test_empty_proxy_pool() {
        let pool = ProxyPool::new(vec![]);
        assert_eq!(pool.size(), 0);
        assert!(pool.get_proxy().is_none());
    }
}