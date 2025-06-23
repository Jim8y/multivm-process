# Security Guide

Comprehensive security guide for deploying and operating MultiVM Process in production environments with enterprise-grade security.

## Table of Contents

- [Security Overview](#security-overview)
- [Architecture Security](#architecture-security)
- [Authentication & Authorization](#authentication--authorization)
- [Encryption & TLS](#encryption--tls)
- [Process Isolation](#process-isolation)
- [Network Security](#network-security)
- [Cryptographic Security](#cryptographic-security)
- [Security Monitoring](#security-monitoring)
- [Incident Response](#incident-response)
- [Security Best Practices](#security-best-practices)
- [Compliance & Auditing](#compliance--auditing)

## Security Overview

MultiVM Process implements defense-in-depth security with multiple layers of protection:

```
┌─────────────────────────────────────────────┐
│            Network Security Layer           │  ← Firewalls, TLS, Rate Limiting
├─────────────────────────────────────────────┤
│        Authentication & Authorization       │  ← JWT, RBAC, Token Management
├─────────────────────────────────────────────┤
│           Application Security              │  ← Input validation, Secure APIs
├─────────────────────────────────────────────┤
│         Cryptographic Security             │  ← Ed25519, ECDSA, Signature Verification
├─────────────────────────────────────────────┤
│            Process Isolation               │  ← OS-level separation, Sandboxing
├─────────────────────────────────────────────┤
│            Infrastructure Security          │  ← Secure deployment, Monitoring
└─────────────────────────────────────────────┘
```

### Security Principles

1. **Zero Trust Architecture**: Never trust, always verify
2. **Defense in Depth**: Multiple security layers
3. **Principle of Least Privilege**: Minimal required permissions
4. **Fail Secure**: Secure defaults and graceful failures
5. **Cryptographic Verification**: All operations cryptographically verified

## Architecture Security

### Process Isolation

MultiVM Process uses OS-level process isolation for security:

```rust
// Each VM runs in a separate process
struct ProcessManager {
    solana_process: ChildProcess,    // Isolated Solana validator
    ethereum_process: ChildProcess,  // Isolated Reth node
    consensus_process: ChildProcess, // Isolated consensus engine
}

// Secure IPC between processes
impl SecureIPC {
    pub async fn authenticate_process(&self, process_id: &str) -> SecurityResult<AuthToken> {
        // Process authentication logic
    }
    
    pub async fn encrypt_message(&self, message: &[u8]) -> SecurityResult<Vec<u8>> {
        // Message encryption logic
    }
}
```

### Component Isolation

Each component has restricted access:

| Component | Network Access | File System Access | Memory Isolation |
|-----------|---------------|-------------------|------------------|
| **Consensus Engine** | Consensus network only | Config files only | ✅ Isolated |
| **SVM Engine** | Local IPC only | VM data only | ✅ Isolated |
| **EVM Engine** | Local IPC only | VM data only | ✅ Isolated |
| **Account Mapper** | Local IPC only | Mapping data only | ✅ Isolated |
| **Coordinator** | Management APIs | All components | ✅ Isolated |

### Secure Communication

All inter-process communication is secured:

```toml
[security.ipc]
enable_authentication = true        # JWT-based authentication
enable_encryption = true           # AES-256-GCM encryption
message_integrity = true           # HMAC verification
replay_protection = true           # Nonce-based replay protection
```

## Authentication & Authorization

### JWT-Based Authentication

MultiVM Process uses JSON Web Tokens for authentication:

```rust
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,        // Subject (process ID)
    exp: usize,         // Expiration time
    iat: usize,         // Issued at
    aud: String,        // Audience (component)
    permissions: Vec<String>, // Granted permissions
}

impl AuthManager {
    pub fn create_token(&self, process_id: &str, permissions: Vec<String>) -> Result<String> {
        let claims = Claims {
            sub: process_id.to_string(),
            exp: (Utc::now() + Duration::hours(24)).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            aud: "multivm-process".to_string(),
            permissions,
        };
        
        encode(&Header::new(Algorithm::HS256), &claims, &self.secret)
    }
}
```

### Role-Based Access Control (RBAC)

Define granular permissions for different roles:

```toml
[security.rbac]
# System administrator role
[security.rbac.roles.admin]
permissions = [
    "system:read",
    "system:write",
    "config:read",
    "config:write",
    "monitoring:read",
    "security:read"
]

# Operator role
[security.rbac.roles.operator]
permissions = [
    "system:read",
    "monitoring:read",
    "health:read"
]

# Read-only monitoring role
[security.rbac.roles.monitor]
permissions = [
    "monitoring:read",
    "health:read",
    "metrics:read"
]

# Service account for consensus
[security.rbac.roles.consensus_service]
permissions = [
    "consensus:read",
    "consensus:write",
    "blocks:read",
    "blocks:write"
]
```

### Token Management

Secure token lifecycle management:

```rust
impl TokenManager {
    pub async fn rotate_tokens(&self) -> SecurityResult<()> {
        // Automatic token rotation
        for process in self.active_processes() {
            if process.token_expires_soon() {
                let new_token = self.create_fresh_token(&process.id).await?;
                process.update_token(new_token).await?;
                self.revoke_old_token(&process.old_token).await?;
            }
        }
        Ok(())
    }
    
    pub async fn revoke_compromised_token(&self, token_id: &str) -> SecurityResult<()> {
        // Immediate token revocation
        self.token_blacklist.add(token_id).await?;
        self.notify_all_processes_of_revocation(token_id).await?;
        Ok(())
    }
}
```

## Encryption & TLS

### TLS Configuration

Production TLS setup with strong security:

```toml
[security.tls]
# TLS versions
min_version = "1.3"                 # TLS 1.3 minimum
max_version = "1.3"                 # TLS 1.3 only

# Strong cipher suites
cipher_suites = [
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
    "TLS_AES_128_GCM_SHA256"
]

# Certificate configuration
cert_file = "/etc/multivm/certs/server.crt"
key_file = "/etc/multivm/certs/server.key"
ca_file = "/etc/multivm/certs/ca.crt"

# Client certificate verification
verify_client_cert = true
client_ca_file = "/etc/multivm/certs/client-ca.crt"

# OCSP stapling
enable_ocsp_stapling = true
ocsp_responder = "http://ocsp.example.com"

# Security headers
enable_hsts = true
hsts_max_age = 31536000            # 1 year
hsts_include_subdomains = true
```

### Certificate Management

Automated certificate management:

```bash
#!/bin/bash
# Certificate generation script

# Generate CA
openssl genrsa -out ca-key.pem 4096
openssl req -new -x509 -key ca-key.pem -out ca.pem -days 3650 \
    -subj "/C=US/ST=CA/L=San Francisco/O=MultiVM/CN=MultiVM CA"

# Generate server certificate with SAN
cat > server.conf << EOF
[req]
distinguished_name = req_distinguished_name
req_extensions = v3_req

[req_distinguished_name]
CN = multivm-server

[v3_req]
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = multivm-server
DNS.3 = *.multivm.local
IP.1 = 127.0.0.1
IP.2 = ::1
EOF

openssl genrsa -out server-key.pem 4096
openssl req -new -key server-key.pem -out server.csr -config server.conf \
    -subj "/C=US/ST=CA/L=San Francisco/O=MultiVM/CN=multivm-server"
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca-key.pem \
    -out server.pem -days 365 -extensions v3_req -extfile server.conf

# Set secure permissions
chmod 600 *-key.pem
chmod 644 *.pem *.crt
```

### Message Encryption

End-to-end message encryption:

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

pub struct MessageEncryption {
    cipher: Aes256Gcm,
    key_rotation_interval: Duration,
    current_key_id: u32,
}

impl MessageEncryption {
    pub fn encrypt_message(&self, plaintext: &[u8]) -> SecurityResult<EncryptedMessage> {
        let nonce = self.generate_nonce();
        let ciphertext = self.cipher.encrypt(&nonce, plaintext)
            .map_err(|e| SecurityError::EncryptionFailed(e.to_string()))?;
            
        Ok(EncryptedMessage {
            key_id: self.current_key_id,
            nonce: nonce.to_vec(),
            ciphertext,
            hmac: self.compute_hmac(&ciphertext)?,
        })
    }
    
    pub fn decrypt_message(&self, encrypted: &EncryptedMessage) -> SecurityResult<Vec<u8>> {
        // Verify HMAC first
        self.verify_hmac(&encrypted.ciphertext, &encrypted.hmac)?;
        
        // Get key for decryption
        let key = self.get_key(encrypted.key_id)?;
        let cipher = Aes256Gcm::new(&key);
        
        // Decrypt
        let nonce = Nonce::from_slice(&encrypted.nonce);
        cipher.decrypt(nonce, encrypted.ciphertext.as_slice())
            .map_err(|e| SecurityError::DecryptionFailed(e.to_string()))
    }
}
```

## Process Isolation

### Container Security

Docker security hardening:

```dockerfile
# Use minimal base image
FROM debian:12-slim

# Create non-root user
RUN groupadd -r multivm && useradd -r -g multivm multivm

# Set security labels
LABEL security.no-new-privileges=true
LABEL security.read-only-rootfs=true
LABEL security.capabilities.drop=ALL

# Install security updates only
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Set secure file permissions
COPY --chown=multivm:multivm target/release/multivm-process /usr/local/bin/
RUN chmod 755 /usr/local/bin/multivm-process

# Use non-root user
USER multivm

# Security configurations
WORKDIR /home/multivm
EXPOSE 8080 9090 26657

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

CMD ["multivm-process"]
```

### systemd Security

Secure systemd service configuration:

```ini
# /etc/systemd/system/multivm-process.service
[Unit]
Description=MultiVM Process - Unified Blockchain Execution
After=network.target
Requires=network.target

[Service]
Type=notify
User=multivm
Group=multivm

# Security settings
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictSUIDSGID=true
LockPersonality=true
MemoryDenyWriteExecute=true

# Capabilities
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
AmbientCapabilities=CAP_NET_BIND_SERVICE

# Namespaces
PrivateNetwork=false  # Needs network access
PrivateUsers=true
PrivateIPC=true

# File system
ReadWritePaths=/var/lib/multivm
ReadOnlyPaths=/etc/multivm
ProtectKernelLogs=true
ProtectClock=true

# System calls
SystemCallFilter=@system-service
SystemCallFilter=~@debug @mount @cpu-emulation @obsolete @privileged @reboot @swap

# Resource limits
LimitNOFILE=65536
LimitMEMLOCK=64K

# Runtime directory
RuntimeDirectory=multivm
RuntimeDirectoryMode=0750

# Configuration
Environment=RUST_LOG=info
EnvironmentFile=-/etc/multivm/environment
ExecStart=/usr/local/bin/multivm-process --config /etc/multivm/multivm.toml
ExecReload=/bin/kill -HUP $MAINPID
KillMode=mixed
KillSignal=SIGTERM
TimeoutStopSec=30

[Install]
WantedBy=multi-user.target
```

## Network Security

### Firewall Configuration

iptables rules for MultiVM Process:

```bash
#!/bin/bash
# Firewall rules for MultiVM Process

# Clear existing rules
iptables -F
iptables -X
iptables -t nat -F
iptables -t nat -X

# Default policies
iptables -P INPUT DROP
iptables -P FORWARD DROP
iptables -P OUTPUT ACCEPT

# Allow loopback
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT

# Allow established connections
iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT

# SSH access (restrict to management network)
iptables -A INPUT -p tcp --dport 22 -s 10.0.1.0/24 -j ACCEPT

# MultiVM services
iptables -A INPUT -p tcp --dport 8080 -s 10.0.0.0/8 -j ACCEPT  # Health check
iptables -A INPUT -p tcp --dport 9090 -s 10.0.1.0/24 -j ACCEPT # Metrics (mgmt only)
iptables -A INPUT -p tcp --dport 26657 -s 10.0.2.0/24 -j ACCEPT # Consensus (validators only)

# Rate limiting for public endpoints
iptables -A INPUT -p tcp --dport 8080 -m limit --limit 25/min --limit-burst 100 -j ACCEPT

# Log dropped packets
iptables -A INPUT -j LOG --log-prefix "DROPPED: "
iptables -A INPUT -j DROP

# Save rules
iptables-save > /etc/iptables/rules.v4
```

### Network Segmentation

Recommended network architecture:

```
┌─────────────────────────────────────────────┐
│              Internet / Public              │
│                   Traffic                   │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│            Load Balancer / WAF             │  ← 10.0.0.0/24
│           (Health Checks Only)             │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│           Management Network               │  ← 10.0.1.0/24
│        (Monitoring, Metrics, SSH)         │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│          Consensus Network                 │  ← 10.0.2.0/24
│      (Validator Communication)             │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│           Application Network              │  ← 10.0.3.0/24
│       (MultiVM Process Instances)          │
└─────────────────────────────────────────────┘
```

### DDoS Protection

Application-level DDoS protection:

```rust
use governor::{Quota, RateLimiter, state::{InMemoryState, NotKeyed}};

pub struct DDoSProtection {
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, quanta::Clock>,
    connection_tracker: ConnectionTracker,
    suspicious_ips: Arc<RwLock<HashSet<IpAddr>>>,
}

impl DDoSProtection {
    pub fn new() -> Self {
        let quota = Quota::per_second(nonzero!(100u32)); // 100 requests per second
        Self {
            rate_limiter: RateLimiter::direct(quota),
            connection_tracker: ConnectionTracker::new(),
            suspicious_ips: Arc::new(RwLock::new(HashSet::new())),
        }
    }
    
    pub async fn check_request(&self, client_ip: IpAddr) -> SecurityResult<()> {
        // Check if IP is blacklisted
        if self.suspicious_ips.read().await.contains(&client_ip) {
            return Err(SecurityError::IpBlacklisted(client_ip));
        }
        
        // Rate limiting
        if self.rate_limiter.check().is_err() {
            self.mark_suspicious_ip(client_ip).await;
            return Err(SecurityError::RateLimitExceeded);
        }
        
        // Connection tracking
        self.connection_tracker.track_connection(client_ip).await?;
        
        Ok(())
    }
}
```

## Cryptographic Security

### Signature Verification

Robust cryptographic verification for both VM types:

```rust
use ed25519_dalek::{Verifier, PublicKey as Ed25519PublicKey, Signature as Ed25519Signature};
use secp256k1::{Secp256k1, PublicKey as Secp256k1PublicKey, Signature as Secp256k1Signature};

pub struct CryptographicVerifier {
    secp_context: Secp256k1<secp256k1::All>,
    ed25519_cache: LruCache<[u8; 32], Ed25519PublicKey>,
    secp256k1_cache: LruCache<[u8; 33], Secp256k1PublicKey>,
}

impl CryptographicVerifier {
    pub fn verify_solana_signature(
        &self,
        message: &[u8],
        signature: &[u8],
        public_key: &[u8]
    ) -> SecurityResult<bool> {
        // Ed25519 verification for Solana
        let public_key = Ed25519PublicKey::from_bytes(public_key)
            .map_err(|e| SecurityError::InvalidPublicKey(e.to_string()))?;
            
        let signature = Ed25519Signature::from_bytes(signature)
            .map_err(|e| SecurityError::InvalidSignature(e.to_string()))?;
            
        public_key.verify(message, &signature)
            .map(|_| true)
            .map_err(|e| SecurityError::SignatureVerificationFailed(e.to_string()))
    }
    
    pub fn verify_ethereum_signature(
        &self,
        message_hash: &[u8],
        signature: &[u8],
        public_key: &[u8]
    ) -> SecurityResult<bool> {
        // ECDSA verification for Ethereum
        let public_key = Secp256k1PublicKey::from_slice(public_key)
            .map_err(|e| SecurityError::InvalidPublicKey(e.to_string()))?;
            
        let signature = Secp256k1Signature::from_compact(signature)
            .map_err(|e| SecurityError::InvalidSignature(e.to_string()))?;
            
        let message = secp256k1::Message::from_slice(message_hash)
            .map_err(|e| SecurityError::InvalidMessage(e.to_string()))?;
            
        self.secp_context.verify(&message, &signature, &public_key)
            .map(|_| true)
            .map_err(|e| SecurityError::SignatureVerificationFailed(e.to_string()))
    }
}
```

### Key Management

Secure key management system:

```rust
use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};

pub struct KeyManager {
    master_key: LessSafeKey,
    key_derivation_salt: [u8; 32],
    key_rotation_schedule: KeyRotationSchedule,
}

impl KeyManager {
    pub fn derive_process_key(&self, process_id: &str) -> SecurityResult<[u8; 32]> {
        use ring::pbkdf2;
        
        let mut derived_key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(100_000).unwrap(), // iterations
            &self.key_derivation_salt,
            process_id.as_bytes(),
            &mut derived_key,
        );
        
        Ok(derived_key)
    }
    
    pub async fn rotate_keys(&self) -> SecurityResult<()> {
        // Implement key rotation logic
        for process in self.get_all_processes().await? {
            let new_key = self.generate_new_key().await?;
            self.distribute_key_to_process(&process.id, &new_key).await?;
            self.schedule_old_key_cleanup(&process.id).await?;
        }
        Ok(())
    }
}
```

## Security Monitoring

### Security Event Logging

Comprehensive security event logging:

```rust
use tracing::{event, Level, Span};
use serde_json::json;

pub struct SecurityLogger {
    audit_sink: AuditSink,
    alert_manager: AlertManager,
}

impl SecurityLogger {
    pub fn log_authentication_attempt(&self, context: AuthContext) {
        let event = json!({
            "event_type": "authentication_attempt",
            "timestamp": Utc::now().to_rfc3339(),
            "source_ip": context.source_ip,
            "user_agent": context.user_agent,
            "process_id": context.process_id,
            "success": context.success,
            "failure_reason": context.failure_reason,
            "geolocation": context.geolocation,
        });
        
        self.audit_sink.log_security_event(event);
        
        if !context.success {
            self.alert_manager.raise_alert(SecurityAlert::AuthenticationFailure {
                source_ip: context.source_ip,
                attempts: context.failed_attempts,
            });
        }
    }
    
    pub fn log_suspicious_activity(&self, activity: SuspiciousActivity) {
        let event = json!({
            "event_type": "suspicious_activity",
            "timestamp": Utc::now().to_rfc3339(),
            "activity_type": activity.activity_type,
            "source": activity.source,
            "details": activity.details,
            "risk_score": activity.risk_score,
        });
        
        self.audit_sink.log_security_event(event);
        
        if activity.risk_score > 8.0 {
            self.alert_manager.raise_alert(SecurityAlert::HighRiskActivity(activity));
        }
    }
}
```

### Intrusion Detection

Real-time intrusion detection:

```rust
pub struct IntrusionDetectionSystem {
    behavioral_analyzer: BehavioralAnalyzer,
    pattern_matcher: PatternMatcher,
    threat_intelligence: ThreatIntelligence,
}

impl IntrusionDetectionSystem {
    pub async fn analyze_request(&self, request: &IncomingRequest) -> SecurityResult<ThreatLevel> {
        let mut threat_indicators = Vec::new();
        
        // Behavioral analysis
        if let Some(anomaly) = self.behavioral_analyzer.detect_anomaly(request).await? {
            threat_indicators.push(ThreatIndicator::BehavioralAnomaly(anomaly));
        }
        
        // Pattern matching
        if let Some(pattern) = self.pattern_matcher.match_malicious_pattern(request).await? {
            threat_indicators.push(ThreatIndicator::MaliciousPattern(pattern));
        }
        
        // Threat intelligence
        if self.threat_intelligence.is_known_bad_actor(&request.source_ip).await? {
            threat_indicators.push(ThreatIndicator::KnownBadActor);
        }
        
        Ok(self.calculate_threat_level(threat_indicators))
    }
}
```

## Incident Response

### Automated Response

Automated security incident response:

```rust
pub struct IncidentResponseSystem {
    response_playbooks: HashMap<ThreatType, ResponsePlaybook>,
    notification_system: NotificationSystem,
    quarantine_manager: QuarantineManager,
}

impl IncidentResponseSystem {
    pub async fn handle_security_incident(&self, incident: SecurityIncident) -> SecurityResult<()> {
        // Log the incident
        self.log_incident(&incident).await?;
        
        // Get appropriate response playbook
        let playbook = self.response_playbooks
            .get(&incident.threat_type)
            .ok_or(SecurityError::NoPlaybookFound)?;
            
        // Execute immediate response
        match incident.severity {
            Severity::Critical => {
                self.execute_critical_response(&incident, playbook).await?;
            },
            Severity::High => {
                self.execute_high_priority_response(&incident, playbook).await?;
            },
            Severity::Medium => {
                self.execute_standard_response(&incident, playbook).await?;
            },
            Severity::Low => {
                self.execute_monitoring_response(&incident, playbook).await?;
            },
        }
        
        // Notify stakeholders
        self.notification_system.notify_incident(&incident).await?;
        
        Ok(())
    }
    
    async fn execute_critical_response(
        &self,
        incident: &SecurityIncident,
        playbook: &ResponsePlaybook
    ) -> SecurityResult<()> {
        // Immediate isolation
        if let Some(source_ip) = &incident.source_ip {
            self.quarantine_manager.quarantine_ip(source_ip).await?;
        }
        
        // Stop affected processes
        for process_id in &incident.affected_processes {
            self.stop_process_immediately(process_id).await?;
        }
        
        // Enable enhanced logging
        self.enable_debug_logging().await?;
        
        // Create incident ticket
        self.create_incident_ticket(incident).await?;
        
        Ok(())
    }
}
```

## Security Best Practices

### Deployment Security Checklist

Pre-deployment security verification:

```bash
#!/bin/bash
# Security audit script

echo "🔒 MultiVM Process Security Audit"
echo "================================="

# Check file permissions
echo "📁 Checking file permissions..."
find /etc/multivm -type f -executable -exec echo "⚠️  Executable config file: {}" \;
find /etc/multivm -type f ! -perm 644 -exec echo "⚠️  Incorrect permissions: {}" \;

# Check for hardcoded secrets
echo "🔑 Checking for hardcoded secrets..."
grep -r "password\|secret\|key" /etc/multivm/multivm.toml && echo "⚠️  Potential secrets in config"

# Check TLS configuration
echo "🔐 Checking TLS configuration..."
openssl x509 -in /etc/multivm/certs/server.crt -text -noout | grep "Not After" | awk -F': ' '{print "📅 Certificate expires:", $2}'

# Check systemd security
echo "🛡️  Checking systemd security..."
systemctl show multivm-process | grep -E "(NoNewPrivileges|ProtectSystem|PrivateTmp)"

# Check network configuration
echo "🌐 Checking network configuration..."
ss -tlnp | grep -E ":8080|:9090|:26657" | while read line; do
    echo "📡 Open port: $line"
done

# Check log file permissions
echo "📝 Checking log file permissions..."
ls -la /var/log/multivm/

echo "✅ Security audit complete"
```

### Runtime Security Monitoring

Continuous security monitoring:

```bash
#!/bin/bash
# Security monitoring script (run via cron)

LOG_FILE="/var/log/multivm/security-monitor.log"
ALERT_THRESHOLD=10

# Monitor failed authentication attempts
FAILED_AUTHS=$(journalctl -u multivm-process --since "1 hour ago" | grep -c "authentication failed")
if [ $FAILED_AUTHS -gt $ALERT_THRESHOLD ]; then
    echo "$(date): ALERT - $FAILED_AUTHS failed authentication attempts in last hour" >> $LOG_FILE
    # Send alert to monitoring system
    curl -X POST "$ALERT_WEBHOOK" -d "{\"alert\": \"High authentication failures\", \"count\": $FAILED_AUTHS}"
fi

# Monitor resource usage
CPU_USAGE=$(top -bn1 | grep "multivm-process" | awk '{print $9}')
if (( $(echo "$CPU_USAGE > 90" | bc -l) )); then
    echo "$(date): ALERT - High CPU usage: $CPU_USAGE%" >> $LOG_FILE
fi

# Check for suspicious network connections
SUSPICIOUS_CONNECTIONS=$(ss -tn | grep ":26657" | grep -v "10.0.2." | wc -l)
if [ $SUSPICIOUS_CONNECTIONS -gt 0 ]; then
    echo "$(date): ALERT - $SUSPICIOUS_CONNECTIONS suspicious consensus connections" >> $LOG_FILE
fi

# Verify certificate validity
CERT_DAYS=$(openssl x509 -in /etc/multivm/certs/server.crt -noout -enddate | cut -d= -f2 | xargs -I {} date -d "{}" +%s)
CURRENT_DAYS=$(date +%s)
DAYS_TO_EXPIRE=$(( (CERT_DAYS - CURRENT_DAYS) / 86400 ))

if [ $DAYS_TO_EXPIRE -lt 30 ]; then
    echo "$(date): ALERT - Certificate expires in $DAYS_TO_EXPIRE days" >> $LOG_FILE
fi
```

### Security Configuration Templates

Production security configuration template:

```toml
# Production Security Configuration Template
[security]
# Enable all security features
enable_authentication = true
enable_encryption = true
enable_rate_limiting = true
enable_audit_logging = true

# Strong authentication
auth_algorithm = "HS256"           # Strong HMAC algorithm
auth_token_ttl = "4h"              # Short token lifetime
auth_token_refresh_threshold = "1h" # Frequent refresh
max_auth_attempts = 3              # Strict attempt limit
auth_lockout_duration = "30m"      # Longer lockout
failed_auth_window = "15m"         # Sliding window for attempts

# Advanced rate limiting
rate_limit_algorithm = "sliding_window"
rate_limit_messages = 50           # Conservative limit
rate_limit_window = "60s"
rate_limit_burst = 5               # Low burst
rate_limit_ip_whitelist = [        # Trusted IPs
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16"
]

# TLS hardening
[security.tls]
min_version = "1.3"
cipher_suites = ["TLS_AES_256_GCM_SHA384"]
require_sni = true
enable_ocsp_stapling = true
enable_ct_logs = true

# Certificate pinning
enable_cert_pinning = true
pinned_certificates = [
    "sha256/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
]

# Audit logging
[security.audit]
enable_detailed_logging = true
log_successful_auth = true
log_failed_auth = true
log_privilege_escalation = true
log_configuration_changes = true
log_system_events = true

# Compliance
[security.compliance]
enable_gdpr_compliance = true
enable_sox_compliance = true
enable_pci_compliance = false      # Enable if handling payment data
data_retention_period = "7y"       # Adjust based on requirements
```

## Concurrency Security

### Lock Ordering Protocol

Prevent deadlocks with hierarchical lock ordering:

```rust
pub enum LockLevel {
    CoordinatorState = 1,      // Highest level - acquire first
    Processes = 2,
    AccountMappings = 3,
    CrossVmTransfers = 4,
    ConnectionPools = 5,
    TransactionQueues = 6,     // Lowest level - acquire last
}

pub async fn acquire_locks_safely<T>(
    lock1: &RwLock<T>,
    lock1_level: LockLevel,
    lock2: &RwLock<T>,
    lock2_level: LockLevel
) -> Result<(RwLockWriteGuard<T>, RwLockWriteGuard<T>)> {
    // Always acquire locks in order of their level
    if lock1_level as u8 <= lock2_level as u8 {
        let guard1 = lock1.write().await;
        let guard2 = lock2.write().await;
        Ok((guard1, guard2))
    } else {
        let guard2 = lock2.write().await;
        let guard1 = lock1.write().await;
        Ok((guard1, guard2))
    }
}
```

### Two-Phase Commit for Cross-VM Transactions

Atomic cross-VM transactions with rollback support:

```rust
pub struct AtomicTransactionCoordinator {
    active_transactions: Arc<RwLock<HashMap<TransactionId, AtomicTransaction>>>,
    process_engines: Arc<ProcessEngineRegistry>,
}

impl AtomicTransactionCoordinator {
    pub async fn execute_cross_vm_transaction(
        &self,
        tx: CrossVmTransaction
    ) -> Result<TransactionReceipt> {
        let tx_id = self.generate_transaction_id();
        
        // Phase 1: Prepare
        let prepare_results = self.prepare_phase(&tx_id, &tx).await?;
        
        // Check if all participants voted to commit
        if prepare_results.iter().all(|r| r.can_commit) {
            // Phase 2: Commit
            self.commit_phase(&tx_id, &tx).await?
        } else {
            // Phase 2: Rollback
            self.rollback_phase(&tx_id, &tx).await?;
            return Err(TransactionError::PrepareFailed);
        }
        
        Ok(TransactionReceipt {
            id: tx_id,
            status: TransactionStatus::Committed,
            timestamp: SystemTime::now(),
        })
    }
}
```

## Security Improvements Summary

### Recent Security Enhancements

1. **Authentication Enhancement**:
   - JWT tokens now use 256-bit entropy (fixed weak token generation)
   - Added nonce-based replay protection
   - Implemented automatic token rotation

2. **Rate Limiting Improvements**:
   - Added per-peer rate limiting with governor crate
   - Implemented global rate limits
   - Added message size validation

3. **Cryptographic Enhancements**:
   - Added replay protection to signature verification
   - Implemented sequence number tracking
   - Added nonce expiry mechanism

4. **Concurrency Security**:
   - Implemented lock ordering protocol to prevent deadlocks
   - Fixed race conditions in process management
   - Added timeout mechanisms for lock acquisition

5. **Input Validation**:
   - Comprehensive sanitization preventing injection attacks
   - Field length limits
   - Special character restrictions

6. **P2P Security**:
   - Ed25519 signatures on all messages
   - Message authentication and validation
   - Transport encryption with Noise protocol option

---

Next: [Deployment Guide](DEPLOYMENT.md) | [Monitoring Guide](MONITORING.md)