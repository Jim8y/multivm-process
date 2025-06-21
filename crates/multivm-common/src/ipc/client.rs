use crate::{
    HealthStatus, IpcCommand, IpcMessage, IpcResponse, IpcTransport, MultivmError, MultivmResult,
    ProcessId,
};
use std::time::Duration;

/// IPC client for sending commands and receiving responses
pub struct IpcClient<T: IpcTransport> {
    transport: T,
    process_id: ProcessId,
}

impl<T: IpcTransport> IpcClient<T> {
    pub fn new(transport: T, process_id: ProcessId) -> Self {
        Self {
            transport,
            process_id,
        }
    }

    /// Send a command and wait for response
    pub async fn send_command(
        &self,
        destination: ProcessId,
        command: IpcCommand,
        timeout: Option<Duration>,
    ) -> MultivmResult<IpcResponse> {
        let mut message = IpcMessage::new(self.process_id, destination, command);
        if let Some(timeout) = timeout {
            message = message.with_timeout(timeout);
        }

        self.transport.send(message).await?;

        // Wait for response (simplified - in production you'd handle response matching)
        let response_msg = self.transport.receive().await?;
        Ok(response_msg.into_response())
    }

    /// Send a command without waiting for response
    pub async fn send_command_async(
        &self,
        destination: ProcessId,
        command: IpcCommand,
    ) -> MultivmResult<()> {
        let message = IpcMessage::new(self.process_id, destination, command);
        self.transport.send(message).await
    }

    /// Process a block with generic serialization
    pub async fn process_block<B>(
        &self,
        destination: ProcessId,
        block: &B,
        blockchain_type: crate::BlockchainType,
        timeout: Option<Duration>,
    ) -> MultivmResult<Vec<u8>>
    where
        B: serde::Serialize,
    {
        // Serialize the block to bytes
        let block_data_bytes = bincode::serialize(block)
            .map_err(|e| MultivmError::Serialization(e.to_string()))?;

        let command = IpcCommand::ProcessBlock {
            block_data_bytes,
            blockchain_type,
            expect_response: true,
        };

        match self
            .send_command(destination, command, timeout.or(Some(Duration::from_secs(30))))
            .await?
        {
            IpcResponse::BlockProcessed {
                result_bytes,
                success,
                ..
            } => {
                if success {
                    Ok(result_bytes)
                } else {
                    Err(MultivmError::BlockProcessing(
                        "Block processing failed".to_string(),
                    ))
                }
            }
            IpcResponse::Error { message, .. } => Err(MultivmError::Ipc(message)),
            _ => Err(MultivmError::Ipc("Unexpected response type".to_string())),
        }
    }

    /// Process a block asynchronously (fire-and-forget)
    pub async fn process_block_async<B>(
        &self,
        destination: ProcessId,
        block: &B,
        blockchain_type: crate::BlockchainType,
    ) -> MultivmResult<()>
    where
        B: serde::Serialize,
    {
        // Serialize the block to bytes
        let block_data_bytes = bincode::serialize(block)
            .map_err(|e| MultivmError::Serialization(e.to_string()))?;

        let command = IpcCommand::ProcessBlock {
            block_data_bytes,
            blockchain_type,
            expect_response: false,
        };

        self.send_command_async(destination, command).await
    }

    /// Get health status from a process
    pub async fn get_health(&self, destination: ProcessId) -> MultivmResult<HealthStatus> {
        match self
            .send_command(
                destination,
                IpcCommand::GetHealth,
                Some(Duration::from_secs(5)),
            )
            .await?
        {
            IpcResponse::Health { status } => Ok(status),
            IpcResponse::Error { message, .. } => Err(MultivmError::Ipc(message)),
            _ => Err(MultivmError::Ipc("Unexpected response type".to_string())),
        }
    }

    /// Request shutdown of a process
    pub async fn shutdown(
        &self,
        destination: ProcessId,
        graceful: bool,
        timeout: Option<Duration>,
    ) -> MultivmResult<()> {
        let command = IpcCommand::Shutdown { graceful, timeout };
        self.send_command_async(destination, command).await
    }
}

// Extension trait to convert IPC messages to responses
trait IntoResponse {
    fn into_response(self) -> IpcResponse;
}

impl IntoResponse for IpcMessage {
    fn into_response(self) -> IpcResponse {
        // This is a simplified implementation
        // In production, you'd need proper response message handling
        match self.command {
            IpcCommand::Ping => IpcResponse::Pong,
            _ => IpcResponse::Ack,
        }
    }
}
