use crate::ipc::transport::IpcTransport;
use crate::{HealthStatus, IpcCommand, IpcResponse, MultivmError, MultivmResult, ProcessId};
use async_trait::async_trait;

/// IPC server for handling incoming commands
pub struct IpcServer<T: IpcTransport> {
    transport: T,
    #[allow(dead_code)]
    process_id: ProcessId,
    handlers: std::collections::HashMap<String, Box<dyn IpcCommandHandler>>,
}

#[async_trait]
pub trait IpcCommandHandler: Send + Sync {
    async fn handle(&self, command: IpcCommand) -> MultivmResult<IpcResponse>;
}

impl<T: IpcTransport> IpcServer<T> {
    pub fn new(transport: T, process_id: ProcessId) -> Self {
        Self {
            transport,
            process_id,
            handlers: std::collections::HashMap::new(),
        }
    }

    pub fn register_handler<H: IpcCommandHandler + 'static>(
        &mut self,
        command_type: &str,
        handler: H,
    ) {
        self.handlers
            .insert(command_type.to_string(), Box::new(handler));
    }

    /// Start the IPC server loop
    pub async fn run(&self) -> MultivmResult<()> {
        loop {
            let message = self.transport.receive().await?;

            // Skip expired messages
            if message.is_expired() {
                tracing::warn!("Skipping expired message: {:?}", message.id);
                continue;
            }

            // Process command
            let response = self.handle_command(message.command.clone()).await;

            // Send response
            match response {
                Ok(resp) => {
                    if let Err(e) = self.transport.send_response(message.id, resp).await {
                        tracing::error!("Failed to send response: {}", e);
                    }
                }
                Err(e) => {
                    let error_response = IpcResponse::Error {
                        code: -1,
                        message: e.to_string(),
                        details: None,
                    };
                    if let Err(e) = self
                        .transport
                        .send_response(message.id, error_response)
                        .await
                    {
                        tracing::error!("Failed to send error response: {}", e);
                    }
                }
            }
        }
    }

    async fn handle_command(&self, command: IpcCommand) -> MultivmResult<IpcResponse> {
        match &command {
            IpcCommand::Ping => Ok(IpcResponse::Pong),
            IpcCommand::GetHealth => {
                // Default health handler
                let status = HealthStatus::Healthy;
                Ok(IpcResponse::Health { status })
            }
            _ => {
                // Try to find a registered handler
                let command_type = std::mem::discriminant(&command);
                if let Some(handler) = self.handlers.get(&format!("{command_type:?}")) {
                    handler.handle(command).await
                } else {
                    Err(MultivmError::UnsupportedOperation {
                        operation: format!("No handler for command: {command:?}"),
                        alternatives: None,
                    })
                }
            }
        }
    }
}
