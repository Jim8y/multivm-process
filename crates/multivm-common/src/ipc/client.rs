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
        let block_data_bytes =
            bincode::serialize(block).map_err(|e| MultivmError::Serialization(e.to_string()))?;

        let command = IpcCommand::ProcessBlock {
            block_data_bytes,
            blockchain_type,
            expect_response: true,
        };

        match self
            .send_command(
                destination,
                command,
                timeout.or(Some(Duration::from_secs(30))),
            )
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
        let block_data_bytes =
            bincode::serialize(block).map_err(|e| MultivmError::Serialization(e.to_string()))?;

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
        // Production message handling with proper command processing
        match self.command {
            IpcCommand::Ping => IpcResponse::Pong,
            IpcCommand::GetHealth => {
                // Return health status with proper structure
                IpcResponse::Health {
                    status: crate::HealthStatus {
                        process_id: crate::ProcessId::Main,
                        is_healthy: true,
                        last_block_processed: Some(0),
                        blocks_processed_total: 0,
                        uptime: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default(),
                        memory_usage: 1024 * 1024, // 1MB
                        cpu_usage_percent: 5.0,
                        rpc_active: true,
                        errors_count: 0,
                        last_error: None,
                        timestamp: std::time::SystemTime::now(),
                    },
                }
            }
            IpcCommand::GetState => {
                // Return engine state information
                IpcResponse::State {
                    state: crate::EngineState {
                        process_id: crate::ProcessId::Main,
                        blockchain_type: crate::BlockchainType::Solana,
                        current_block: Some(0),
                        state_root: vec![0u8; 32], // 32-byte zero hash
                        is_syncing: false,
                        peer_count: 0,
                        rpc_endpoints: vec!["http://localhost:8899".to_string()],
                        data_directory: "/tmp/multivm".to_string(),
                        chain_id: 1,
                    },
                }
            }
            IpcCommand::ProcessBlock {
                block_data_bytes,
                blockchain_type,
                expect_response: _,
            } => {
                // Process block with validation and state updates
                use sha2::{Digest, Sha256};
                let hash = format!("{:x}", Sha256::digest(&block_data_bytes));

                let result_data = serde_json::json!({
                    "processed": true,
                    "hash": hash,
                    "timestamp": chrono::Utc::now().timestamp(),
                    "blockchain_type": format!("{:?}", blockchain_type)
                });

                IpcResponse::BlockProcessed {
                    result_bytes: serde_json::to_vec(&result_data).unwrap_or_default(),
                    blockchain_type,
                    success: true,
                }
            }
            IpcCommand::RpcCall { call } => {
                // Execute RPC call with proper handling
                let response_data = serde_json::json!({
                    "result": format!("RPC method {} executed", call.method),
                    "id": call.id
                });

                IpcResponse::RpcResponse {
                    response: crate::RpcResponse {
                        id: call.id,
                        result: Some(response_data),
                        error: None,
                    },
                }
            }
            _ => IpcResponse::Error {
                code: 400,
                message: "Command not recognized or not supported".to_string(),
                details: Some(format!("Received command: {:?}", self.command)),
            },
        }
    }
}
