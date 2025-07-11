//! RPC client implementation

use std::time::Duration;
use tonic::transport::{Channel, Endpoint};
use tonic::Status;

use crate::proto::multivm::*;

/// RPC client configuration
#[derive(Debug, Clone)]
pub struct RpcClientConfig {
    /// Server address
    pub server_addr: String,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Enable TLS
    pub enable_tls: bool,
    /// Node ID for authentication
    pub node_id: Option<String>,
}

impl Default for RpcClientConfig {
    fn default() -> Self {
        Self {
            server_addr: "http://127.0.0.1:50051".to_string(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            enable_tls: false,
            node_id: None,
        }
    }
}

/// RPC client
#[derive(Clone)]
pub struct RpcClient {
    config: RpcClientConfig,
    channel: Channel,
}

impl RpcClient {
    /// Create a new RPC client
    pub async fn new(config: RpcClientConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let endpoint = Endpoint::from_shared(config.server_addr.clone())?
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout);

        let channel = endpoint.connect().await?;

        Ok(Self { config, channel })
    }

    /// Get a node service client
    pub fn node_service(&self) -> node_service_client::NodeServiceClient<Channel> {
        node_service_client::NodeServiceClient::new(self.channel.clone())
    }

    /// Get a VM service client
    pub fn vm_service(&self) -> vm_service_client::VmServiceClient<Channel> {
        vm_service_client::VmServiceClient::new(self.channel.clone())
    }

    /// Helper: Join cluster
    pub async fn join_cluster(
        &self,
        node_id: String,
        address: String,
        capabilities: NodeCapabilities,
    ) -> Result<Vec<ClusterNode>, Status> {
        let request = JoinClusterRequest {
            node_id,
            address,
            capabilities: Some(capabilities),
        };

        let response = self.node_service().join_cluster(request).await?;
        Ok(response.into_inner().nodes)
    }

    /// Helper: Create VM
    pub async fn create_vm(
        &self,
        name: String,
        resources: multivm_core::ResourceRequirements,
        labels: std::collections::HashMap<String, String>,
    ) -> Result<String, Status> {
        let request = CreateVmRequest {
            name,
            resources: Some(resources.into()),
            labels,
        };

        let response = self.vm_service().create_vm(request).await?;
        Ok(response.into_inner().vm_id)
    }

    /// Helper: Start VM
    pub async fn start_vm(&self, vm_id: String) -> Result<(), Status> {
        let request = VmIdRequest { vm_id };
        self.vm_service().start_vm(request).await?;
        Ok(())
    }

    /// Helper: Stop VM
    pub async fn stop_vm(&self, vm_id: String, force: bool) -> Result<(), Status> {
        let request = StopVmRequest { vm_id, force };
        self.vm_service().stop_vm(request).await?;
        Ok(())
    }

    /// Helper: Get VM info
    pub async fn get_vm(&self, vm_id: String) -> Result<VmInfo, Status> {
        let request = VmIdRequest { vm_id };
        let response = self.vm_service().get_vm(request).await?;
        Ok(response.into_inner())
    }

    /// Helper: List VMs
    pub async fn list_vms(
        &self,
        label_selector: std::collections::HashMap<String, String>,
        node_id: Option<String>,
    ) -> Result<Vec<VmInfo>, Status> {
        let request = ListVmsRequest {
            label_selector,
            node_id: node_id.unwrap_or_default(),
        };

        let response = self.vm_service().list_vms(request).await?;
        Ok(response.into_inner().vms)
    }

    /// Helper: Send heartbeat
    pub async fn send_heartbeat(
        &self,
        node_id: String,
        resource_usage: multivm_core::ResourceUsage,
    ) -> Result<(), Status> {
        let request = HeartbeatRequest {
            node_id,
            resource_usage: Some(resource_usage.into()),
        };

        self.node_service().heartbeat(request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let config = RpcClientConfig::default();
        // This will fail to connect in tests, but we're just testing creation
        let _result = RpcClient::new(config).await;
    }
}