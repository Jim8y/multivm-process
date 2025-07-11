//! RPC server implementation

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, error};
use tower::ServiceBuilder;
use tower_http::timeout::TimeoutLayer;

use crate::proto::multivm::*;
use crate::interceptors::auth_interceptor;

/// RPC server configuration
#[derive(Debug, Clone)]
pub struct RpcServerConfig {
    /// Listen address
    pub listen_addr: SocketAddr,
    /// Enable TLS
    pub enable_tls: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS key path
    pub tls_key_path: Option<String>,
    /// Request timeout
    pub request_timeout: std::time::Duration,
    /// Max concurrent requests
    pub max_concurrent_requests: usize,
}

impl Default for RpcServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: ([127, 0, 0, 1], 50051).into(),
            enable_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            request_timeout: std::time::Duration::from_secs(30),
            max_concurrent_requests: 1000,
        }
    }
}

/// Node service implementation
pub struct NodeServiceImpl<S> {
    state: Arc<S>,
}

impl<S> NodeServiceImpl<S> {
    pub fn new(state: Arc<S>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl<S> node_service_server::NodeService for NodeServiceImpl<S>
where
    S: NodeState + Send + Sync + 'static,
{
    async fn join_cluster(
        &self,
        request: Request<JoinClusterRequest>,
    ) -> Result<Response<JoinClusterResponse>, Status> {
        let req = request.into_inner();
        info!("Node {} joining cluster", req.node_id);

        let nodes = self.state.join_cluster(req).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(JoinClusterResponse { nodes }))
    }

    async fn leave_cluster(
        &self,
        request: Request<()>,
    ) -> Result<Response<()>, Status> {
        let metadata = request.metadata();
        let node_id = metadata.get("node-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Status::unauthenticated("Missing node-id"))?;

        info!("Node {} leaving cluster", node_id);

        self.state.leave_cluster(node_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn get_node_status(
        &self,
        request: Request<()>,
    ) -> Result<Response<NodeStatus>, Status> {
        let metadata = request.metadata();
        let node_id = metadata.get("node-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Status::unauthenticated("Missing node-id"))?;

        let status = self.state.get_node_status(node_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(status))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        
        self.state.update_heartbeat(req).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }
}

/// VM service implementation
pub struct VmServiceImpl<S> {
    state: Arc<S>,
}

impl<S> VmServiceImpl<S> {
    pub fn new(state: Arc<S>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl<S> vm_service_server::VmService for VmServiceImpl<S>
where
    S: VmState + Send + Sync + 'static,
{
    async fn create_vm(
        &self,
        request: Request<CreateVmRequest>,
    ) -> Result<Response<CreateVmResponse>, Status> {
        let req = request.into_inner();
        info!("Creating VM: {}", req.name);

        let vm_id = self.state.create_vm(req).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateVmResponse { vm_id }))
    }

    async fn start_vm(
        &self,
        request: Request<VmIdRequest>,
    ) -> Result<Response<()>, Status> {
        let vm_id = request.into_inner().vm_id;
        info!("Starting VM: {}", vm_id);

        self.state.start_vm(&vm_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn stop_vm(
        &self,
        request: Request<StopVmRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Stopping VM: {} (force: {})", req.vm_id, req.force);

        self.state.stop_vm(&req.vm_id, req.force).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn delete_vm(
        &self,
        request: Request<VmIdRequest>,
    ) -> Result<Response<()>, Status> {
        let vm_id = request.into_inner().vm_id;
        info!("Deleting VM: {}", vm_id);

        self.state.delete_vm(&vm_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }

    async fn get_vm(
        &self,
        request: Request<VmIdRequest>,
    ) -> Result<Response<VmInfo>, Status> {
        let vm_id = request.into_inner().vm_id;
        
        let vm_info = self.state.get_vm(&vm_id).await
            .map_err(|e| match e.to_string().contains("not found") {
                true => Status::not_found(e.to_string()),
                false => Status::internal(e.to_string()),
            })?;

        Ok(Response::new(vm_info))
    }

    async fn list_vms(
        &self,
        request: Request<ListVmsRequest>,
    ) -> Result<Response<ListVmsResponse>, Status> {
        let req = request.into_inner();
        
        let vms = self.state.list_vms(req).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListVmsResponse { vms }))
    }

    async fn migrate_vm(
        &self,
        request: Request<MigrateVmRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Migrating VM {} to node {} (live: {})", 
              req.vm_id, req.target_node_id, req.live);

        self.state.migrate_vm(req).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(()))
    }
}

/// Trait for node state management
#[tonic::async_trait]
pub trait NodeState: Send + Sync {
    async fn join_cluster(&self, req: JoinClusterRequest) -> Result<Vec<ClusterNode>, multivm_core::Error>;
    async fn leave_cluster(&self, node_id: &str) -> Result<(), multivm_core::Error>;
    async fn get_node_status(&self, node_id: &str) -> Result<NodeStatus, multivm_core::Error>;
    async fn update_heartbeat(&self, req: HeartbeatRequest) -> Result<(), multivm_core::Error>;
}

/// Trait for VM state management
#[tonic::async_trait]
pub trait VmState: Send + Sync {
    async fn create_vm(&self, req: CreateVmRequest) -> Result<String, multivm_core::Error>;
    async fn start_vm(&self, vm_id: &str) -> Result<(), multivm_core::Error>;
    async fn stop_vm(&self, vm_id: &str, force: bool) -> Result<(), multivm_core::Error>;
    async fn delete_vm(&self, vm_id: &str) -> Result<(), multivm_core::Error>;
    async fn get_vm(&self, vm_id: &str) -> Result<VmInfo, multivm_core::Error>;
    async fn list_vms(&self, req: ListVmsRequest) -> Result<Vec<VmInfo>, multivm_core::Error>;
    async fn migrate_vm(&self, req: MigrateVmRequest) -> Result<(), multivm_core::Error>;
}

/// RPC server
#[derive(Clone)]
pub struct RpcServer {
    config: RpcServerConfig,
}

impl RpcServer {
    pub fn new(config: RpcServerConfig) -> Self {
        Self { config }
    }

    /// Run the RPC server
    pub async fn run<NS, VS>(
        self,
        node_state: Arc<NS>,
        vm_state: Arc<VS>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        NS: NodeState + 'static,
        VS: VmState + 'static,
    {
        let addr = self.config.listen_addr;
        info!("Starting RPC server on {}", addr);

        let node_service = NodeServiceImpl::new(node_state);
        let vm_service = VmServiceImpl::new(vm_state);

        let layer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(self.config.request_timeout))
            .into_inner();

        let server = Server::builder()
            .layer(layer)
            .add_service(node_service_server::NodeServiceServer::with_interceptor(
                node_service,
                auth_interceptor,
            ))
            .add_service(vm_service_server::VmServiceServer::with_interceptor(
                vm_service,
                auth_interceptor,
            ));

        if self.config.enable_tls {
            // TLS configuration would go here
            error!("TLS not implemented yet");
        }

        server.serve(addr).await?;

        Ok(())
    }
}