//! RPC interceptors for authentication and logging

use tonic::{Request, Status};
use tracing::debug;

/// Simple authentication interceptor
pub fn auth_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    // In production, implement proper authentication
    debug!("Auth interceptor called");
    
    // Check for node-id in metadata for node operations
    let metadata = req.metadata();
    if metadata.contains_key("node-id") {
        debug!("Request authenticated with node-id");
    }

    Ok(req)
}

/// Logging interceptor
pub fn logging_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let metadata = req.metadata();
    debug!("Request metadata: {:?}", metadata);
    Ok(req)
}