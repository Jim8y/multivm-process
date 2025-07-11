//! TLS configuration and utilities

use crate::{NetworkError, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use tokio::fs;
use tokio_rustls::rustls::{self, ClientConfig, ServerConfig};

/// TLS configuration
#[derive(Clone, Debug)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub ca_cert_path: Option<String>,
    pub verify_hostname: bool,
}

impl TlsConfig {
    /// Load server TLS configuration
    pub async fn load_server_config(&self) -> Result<Arc<ServerConfig>> {
        let certs = load_certs(&self.cert_path).await?;
        let key = load_private_key(&self.key_path).await?;
        
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| NetworkError::Tls(e.to_string()))?;
            
        Ok(Arc::new(config))
    }
    
    /// Load client TLS configuration
    pub async fn load_client_config(&self) -> Result<Arc<ClientConfig>> {
        let root_certs = if let Some(ca_path) = &self.ca_cert_path {
            load_root_certs(ca_path).await?
        } else {
            // Use system root certificates
            rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS
                    .iter()
                    .cloned()
                    .collect(),
            }
        };
        
        let config = ClientConfig::builder()
            .with_root_certificates(root_certs)
            .with_no_client_auth();
            
        Ok(Arc::new(config))
    }
}

/// Load certificates from file
async fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let cert_data = fs::read(path)
        .await
        .map_err(|e| NetworkError::Tls(format!("Failed to read cert file: {}", e)))?;
        
    let certs = rustls_pemfile::certs(&mut &cert_data[..])
        .map(|cert| cert.map(|c| c.to_owned()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| NetworkError::Tls(format!("Failed to parse certs: {}", e)))?;
        
    if certs.is_empty() {
        return Err(NetworkError::Tls("No certificates found".to_string()));
    }
    
    Ok(certs)
}

/// Load private key from file
async fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let key_data = fs::read(path)
        .await
        .map_err(|e| NetworkError::Tls(format!("Failed to read key file: {}", e)))?;
        
    let keys = rustls_pemfile::pkcs8_private_keys(&mut &key_data[..])
        .map(|key| key.map(|k| PrivateKeyDer::Pkcs8(k.secret_pkcs8_der().to_owned().into())))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| NetworkError::Tls(format!("Failed to parse private key: {}", e)))?;
        
    keys.into_iter()
        .next()
        .ok_or_else(|| NetworkError::Tls("No private key found".to_string()))
}

/// Load root certificates
async fn load_root_certs(path: &str) -> Result<rustls::RootCertStore> {
    let mut root_store = rustls::RootCertStore::empty();
    let ca_data = fs::read(path)
        .await
        .map_err(|e| NetworkError::Tls(format!("Failed to read CA file: {}", e)))?;
        
    let ca_certs = rustls_pemfile::certs(&mut &ca_data[..])
        .map(|cert| cert.map(|c| c.to_owned()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| NetworkError::Tls(format!("Failed to parse CA certs: {}", e)))?;
        
    for cert in ca_certs {
        root_store
            .add(cert)
            .map_err(|e| NetworkError::Tls(format!("Failed to add CA cert: {}", e)))?;
    }
    
    Ok(root_store)
}

/// Generate self-signed certificate for testing
#[cfg(test)]
pub fn generate_test_cert() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    use rcgen::{CertificateParams, DnType, KeyPair};
    
    let mut params = CertificateParams::default();
    params.distinguished_name.push(DnType::CommonName, "localhost");
    params.subject_alt_names = vec![
        rcgen::SanType::DnsName("localhost".to_string().try_into().unwrap()),
        rcgen::SanType::IpAddress("127.0.0.1".parse().unwrap()),
    ];
    
    let key_pair = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    
    (
        vec![cert.der().clone()],
        PrivateKeyDer::Pkcs8(key_pair.serialize_der().into()),
    )
}