use crate::{IpcMessage, IpcResponse, MessageId, MultivmResult};
use async_trait::async_trait;

/// IPC transport layer abstraction
#[async_trait]
pub trait IpcTransport: Send + Sync {
    async fn send(&self, message: IpcMessage) -> MultivmResult<()>;
    async fn receive(&self) -> MultivmResult<IpcMessage>;
    async fn send_response(
        &self,
        message_id: MessageId,
        response: IpcResponse,
    ) -> MultivmResult<()>;
    async fn close(&self) -> MultivmResult<()>;
}
