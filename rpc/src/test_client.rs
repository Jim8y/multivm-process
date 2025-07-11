//! Test client wrapper for RPC integration tests

use crate::proto::multivm::{
    vm_service_client::VmServiceClient,
    node_service_client::NodeServiceClient,
    CreateVmRequest, CreateVmResponse, VmIdRequest, StopVmRequest,
    ListVmsRequest, ListVmsResponse, MigrateVmRequest,
    NodeStatus, HeartbeatRequest,
};
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

/// Test client that wraps both VM and Node service clients
#[derive(Clone)]
pub struct TestClient {
    vm_client: VmServiceClient<Channel>,
    node_client: NodeServiceClient<Channel>,
}

impl TestClient {
    /// Connect to the RPC server
    pub async fn connect(addr: String) -> Result<Self, Box<dyn std::error::Error>> {
        let channel = Channel::from_shared(addr.clone())?.connect().await?;
        let vm_client = VmServiceClient::new(channel.clone());
        let node_client = NodeServiceClient::new(channel);
        
        Ok(Self {
            vm_client,
            node_client,
        })
    }
    
    /// Create a VM
    pub async fn create_vm(&mut self, request: CreateVmRequest) -> Result<Response<CreateVmResponse>, Status> {
        self.vm_client.create_vm(request).await
    }
    
    /// Start a VM
    pub async fn start_vm(&mut self, vm_id: String) -> Result<Response<()>, Status> {
        self.vm_client.start_vm(VmIdRequest { vm_id }).await
    }
    
    /// Stop a VM
    pub async fn stop_vm(&mut self, vm_id: String, force: bool) -> Result<Response<()>, Status> {
        self.vm_client.stop_vm(StopVmRequest { vm_id, force }).await
    }
    
    /// Delete a VM
    pub async fn delete_vm(&mut self, vm_id: String) -> Result<Response<()>, Status> {
        self.vm_client.delete_vm(VmIdRequest { vm_id }).await
    }
    
    /// Get VM info
    pub async fn get_vm(&mut self, vm_id: String) -> Result<Response<crate::proto::multivm::VmInfo>, Status> {
        self.vm_client.get_vm(VmIdRequest { vm_id }).await
    }
    
    /// List VMs
    pub async fn list_vms(&mut self, request: ListVmsRequest) -> Result<Response<ListVmsResponse>, Status> {
        self.vm_client.list_vms(request).await
    }
    
    /// Migrate VM
    pub async fn migrate_vm(&mut self, vm_id: String, target_node_id: String, live: bool) -> Result<Response<()>, Status> {
        self.vm_client.migrate_vm(MigrateVmRequest {
            vm_id,
            target_node_id,
            live,
        }).await
    }
    
    /// Get node status
    pub async fn get_node_status(&mut self) -> Result<Response<NodeStatus>, Status> {
        self.node_client.get_node_status(()).await
    }
    
    /// Send heartbeat
    pub async fn heartbeat(&mut self, request: HeartbeatRequest) -> Result<Response<()>, Status> {
        self.node_client.heartbeat(request).await
    }
}