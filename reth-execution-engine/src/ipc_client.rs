use multivm_common::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpStream, UnixStream};

#[allow(dead_code)]
pub enum IpcStream {
    #[cfg(unix)]
    Unix {
        reader: BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
    },
    Tcp {
        reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    },
}

#[allow(dead_code)]
pub struct RethIpcClient {
    stream: IpcStream,
}

#[allow(dead_code)]
impl RethIpcClient {
    pub async fn new(address: &str) -> Result<Self, Box<dyn std::error::Error>> {
        tracing::info!("Connecting to IPC address: {}", address);

        if address.starts_with("/") || address.starts_with("./") {
            // Unix socket
            Self::connect_unix(address).await
        } else if address.contains(":") {
            // TCP socket
            Self::connect_tcp(address).await
        } else {
            // Assume it's a port number
            let tcp_address = format!("127.0.0.1:{}", address);
            Self::connect_tcp(&tcp_address).await
        }
    }

    async fn connect_unix(socket_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            let stream = UnixStream::connect(socket_path).await?;
            let (reader, writer) = stream.into_split();

            Ok(Self {
                stream: IpcStream::Unix {
                    reader: BufReader::new(reader),
                    writer: BufWriter::new(writer),
                },
            })
        }

        #[cfg(not(unix))]
        {
            Err("Unix sockets not supported on this platform".into())
        }
    }

    async fn connect_tcp(address: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(address).await?;
        let (reader, writer) = stream.into_split();

        Ok(Self {
            stream: IpcStream::Tcp {
                reader: BufReader::new(reader),
                writer: BufWriter::new(writer),
            },
        })
    }

    pub async fn receive_command(&mut self) -> Result<IpcCommand, MultivmError> {
        // Read message length
        let mut len_bytes = [0u8; 4];

        match &mut self.stream {
            #[cfg(unix)]
            IpcStream::Unix { reader, .. } => {
                reader.read_exact(&mut len_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to read message length: {}", e))
                })?;
            }
            IpcStream::Tcp { reader, .. } => {
                reader.read_exact(&mut len_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to read message length: {}", e))
                })?;
            }
        }

        let message_len = u32::from_be_bytes(len_bytes) as usize;

        // Read message data
        let mut message_bytes = vec![0u8; message_len];

        match &mut self.stream {
            #[cfg(unix)]
            IpcStream::Unix { reader, .. } => {
                reader.read_exact(&mut message_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to read message data: {}", e))
                })?;
            }
            IpcStream::Tcp { reader, .. } => {
                reader.read_exact(&mut message_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to read message data: {}", e))
                })?;
            }
        }

        // Deserialize message
        let message: IpcMessage = bincode::deserialize(&message_bytes)
            .map_err(|e| MultivmError::Ipc(format!("Failed to deserialize message: {}", e)))?;

        tracing::trace!("Received IPC command: {:?}", message.command);
        Ok(message.command)
    }

    pub async fn send_response(&mut self, response: IpcResponse) -> Result<(), MultivmError> {
        tracing::trace!("Sending IPC response: {:?}", response);

        // Serialize response
        let response_bytes = bincode::serialize(&response)
            .map_err(|e| MultivmError::Ipc(format!("Failed to serialize response: {}", e)))?;

        // Send message length
        let len_bytes = (response_bytes.len() as u32).to_be_bytes();

        match &mut self.stream {
            #[cfg(unix)]
            IpcStream::Unix { writer, .. } => {
                writer.write_all(&len_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to write response length: {}", e))
                })?;

                writer.write_all(&response_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to write response data: {}", e))
                })?;

                writer
                    .flush()
                    .await
                    .map_err(|e| MultivmError::Ipc(format!("Failed to flush response: {}", e)))?;
            }
            IpcStream::Tcp { writer, .. } => {
                writer.write_all(&len_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to write response length: {}", e))
                })?;

                writer.write_all(&response_bytes).await.map_err(|e| {
                    MultivmError::Ipc(format!("Failed to write response data: {}", e))
                })?;

                writer
                    .flush()
                    .await
                    .map_err(|e| MultivmError::Ipc(format!("Failed to flush response: {}", e)))?;
            }
        }

        Ok(())
    }

    pub async fn send_heartbeat(
        &mut self,
        health_response: IpcResponse,
    ) -> Result<(), MultivmError> {
        // For now, heartbeat is just a regular response
        // In a more complex implementation, you might have a separate heartbeat protocol
        self.send_response(health_response).await
    }
}
