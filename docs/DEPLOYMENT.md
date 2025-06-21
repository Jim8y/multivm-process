# Deployment Guide

Complete guide for deploying MultiVM Process in production environments, from single-node development setups to high-availability enterprise deployments.

## Table of Contents

- [Deployment Overview](#deployment-overview)
- [Infrastructure Requirements](#infrastructure-requirements)
- [Single Node Deployment](#single-node-deployment)
- [High Availability Deployment](#high-availability-deployment)
- [Container Deployment](#container-deployment)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Cloud Deployment](#cloud-deployment)
- [Monitoring & Observability](#monitoring--observability)
- [Backup & Recovery](#backup--recovery)
- [Maintenance & Updates](#maintenance--updates)

## Deployment Overview

MultiVM Process supports multiple deployment architectures:

```
Development → Single Node → Multi-Node → High Availability → Cloud Native
     ↓            ↓           ↓              ↓               ↓
   Local       Production   Redundancy   Zero Downtime   Auto-scaling
   Testing      Ready       & Performance  & Disaster    & Multi-region
                                          Recovery
```

### Deployment Architectures

| Architecture | Nodes | Consensus | Use Case | Availability |
|--------------|-------|-----------|----------|--------------|
| **Development** | 1 | Single validator | Local testing | 95% |
| **Single Node** | 1 | Multi-validator simulation | Small production | 99% |
| **Multi-Node** | 3-5 | Distributed consensus | Production | 99.9% |
| **High Availability** | 7+ | Byzantine fault tolerant | Enterprise | 99.99% |
| **Cloud Native** | Auto-scaling | Dynamic consensus | Global scale | 99.999% |

## Infrastructure Requirements

### Hardware Requirements

#### Minimum Production Requirements
```
CPU:     8 cores (x86_64)
RAM:     18GB
Storage: 600GB SSD
Network: 1Gbps
OS:      Linux (Ubuntu 22.04+ recommended)
```

#### Recommended Production Requirements
```
CPU:     16 cores (x86_64, 3.0GHz+)
RAM:     36GB
Storage: 1.2TB NVMe SSD
Network: 10Gbps
OS:      Linux (Ubuntu 22.04 LTS)
```

#### High-Availability Requirements
```
CPU:     32 cores (x86_64, 3.2GHz+)
RAM:     72GB
Storage: 2.4TB NVMe SSD (RAID 1)
Network: 25Gbps (redundant)
OS:      Linux (Ubuntu 22.04 LTS)
```

### Network Requirements

```
Port Requirements:
- 8080/tcp:  Health checks and management API
- 9090/tcp:  Metrics endpoint (internal)
- 26657/tcp: Consensus communication
- 22/tcp:    SSH management (restricted)

Bandwidth Requirements:
- Minimum: 100Mbps sustained
- Recommended: 1Gbps sustained
- High-availability: 10Gbps sustained

Latency Requirements:
- Inter-node: <10ms RTT
- Client connections: <100ms RTT
- Consensus network: <5ms RTT
```

## Single Node Deployment

### System Preparation

```bash
#!/bin/bash
# System preparation script for single node deployment

# Update system
sudo apt update && sudo apt upgrade -y

# Create multivm user
sudo useradd -r -s /bin/bash -d /opt/multivm -m multivm

# Create directory structure
sudo mkdir -p /opt/multivm/{bin,config,data,logs}
sudo mkdir -p /etc/multivm/certs
sudo mkdir -p /var/lib/multivm
sudo mkdir -p /var/log/multivm

# Set permissions
sudo chown -R multivm:multivm /opt/multivm /var/lib/multivm /var/log/multivm
sudo chown -R root:multivm /etc/multivm
sudo chmod -R 750 /etc/multivm

# Install system dependencies
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    jq \
    htop \
    iotop \
    netstat-nat

# Configure log rotation
sudo tee /etc/logrotate.d/multivm << EOF
/var/log/multivm/*.log {
    daily
    missingok
    rotate 30
    compress
    delaycompress
    notifempty
    create 644 multivm multivm
    postrotate
        systemctl reload multivm-process
    endscript
}
EOF
```

### Installation

```bash
#!/bin/bash
# Installation script

# Download and install MultiVM Process
MULTIVM_VERSION="v1.0.0"
wget "https://github.com/your-org/multivm-process/releases/download/${MULTIVM_VERSION}/multivm-process-${MULTIVM_VERSION}-linux-x86_64.tar.gz"

# Verify checksum
wget "https://github.com/your-org/multivm-process/releases/download/${MULTIVM_VERSION}/SHA256SUMS"
sha256sum -c SHA256SUMS --ignore-missing

# Extract and install
tar -xzf "multivm-process-${MULTIVM_VERSION}-linux-x86_64.tar.gz"
sudo cp multivm-process /opt/multivm/bin/
sudo chmod +x /opt/multivm/bin/multivm-process

# Create symlink
sudo ln -sf /opt/multivm/bin/multivm-process /usr/local/bin/multivm-process

# Copy configuration
sudo cp config/multivm.toml /etc/multivm/
sudo cp examples/systemd/multivm-process.service /etc/systemd/system/

# Generate certificates
sudo /opt/multivm/bin/multivm-process generate-certs \
    --output-dir /etc/multivm/certs \
    --san localhost \
    --san $(hostname) \
    --san $(hostname -I | awk '{print $1}')

# Set up environment
sudo tee /etc/multivm/environment << EOF
RUST_LOG=info
MULTIVM_CONFIG_PATH=/etc/multivm/multivm.toml
MULTIVM_DATA_DIR=/var/lib/multivm
MULTIVM_LOG_DIR=/var/log/multivm
EOF

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable multivm-process
sudo systemctl start multivm-process

# Verify installation
sleep 10
sudo systemctl status multivm-process
curl -f http://localhost:8080/health || echo "Health check failed"
```

### Configuration

Single node production configuration:

```toml
# /etc/multivm/multivm.toml - Single Node Production
[coordinator]
health_check_interval = "30s"
block_timeout = "60s"
max_concurrent_blocks = 15
enable_recovery = true

[consensus]
validator_id = "validator-primary"
listen_addr = "0.0.0.0:26657"
timeout_ms = 5000
max_block_size = "4MB"
block_time = "6s"

# Single node with simulated validators for development
validators = [
    { id = "validator-primary", address = "127.0.0.1:26657", weight = 1 }
]
min_validators = 1

[security]
enable_authentication = true
enable_encryption = true
enable_rate_limiting = true
rate_limit_messages = 200
rate_limit_window = "60s"

[monitoring]
enable_metrics = true
metrics_bind_addr = "0.0.0.0:9090"
health_check_bind_addr = "0.0.0.0:8080"

[logging]
level = "info"
format = "json"
target = "file"
file_path = "/var/log/multivm/multivm.log"
max_file_size = "100MB"
max_files = 10
```

## High Availability Deployment

### Multi-Node Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Load Balancer                            │
│                  (HAProxy/Nginx)                           │
└─────────────────┬─────────────┬─────────────┬───────────────┘
                  │             │             │
┌─────────────────┴───┐  ┌──────┴──────┐  ┌──┴───────────────┐
│   MultiVM Node 1    │  │ MultiVM     │  │   MultiVM Node 3 │
│   (Primary)         │  │ Node 2      │  │   (Secondary)    │
│                     │  │             │  │                  │
│ Validator-0         │  │ Validator-1 │  │ Validator-2      │
│ Health: 8080        │  │ Health:8080 │  │ Health: 8080     │
│ Metrics: 9090       │  │ Metrics:9090│  │ Metrics: 9090    │
│ Consensus: 26657    │  │ Consensus:  │  │ Consensus: 26657 │
└─────────────────────┘  │ 26657       │  └──────────────────┘
                         └─────────────┘
                              
┌─────────────────────────────────────────────────────────────┐
│                Shared Storage/Backup                        │
│              (NFS/GlusterFS/Ceph)                          │
└─────────────────────────────────────────────────────────────┘
```

### Node Configuration

#### Node 1 (Primary) Configuration

```toml
# /etc/multivm/multivm.toml - Node 1
[coordinator]
health_check_interval = "10s"
block_timeout = "30s"
max_concurrent_blocks = 25
enable_recovery = true
node_role = "primary"

[consensus]
validator_id = "validator-0"
listen_addr = "0.0.0.0:26657"
timeout_ms = 3000
max_block_size = "8MB"
block_time = "3s"

# Full validator set
validators = [
    { id = "validator-0", address = "10.0.2.10:26657", weight = 1 },
    { id = "validator-1", address = "10.0.2.11:26657", weight = 1 },
    { id = "validator-2", address = "10.0.2.12:26657", weight = 1 },
    { id = "validator-3", address = "10.0.2.13:26657", weight = 1 },
    { id = "validator-4", address = "10.0.2.14:26657", weight = 1 },
    { id = "validator-5", address = "10.0.2.15:26657", weight = 1 },
    { id = "validator-6", address = "10.0.2.16:26657", weight = 1 }
]
min_validators = 5
byzantine_fault_tolerance = 2

[security]
enable_authentication = true
enable_encryption = true
tls_cert_path = "/etc/multivm/certs/node-1.crt"
tls_key_path = "/etc/multivm/certs/node-1.key"

[performance]
worker_threads = 16
blocking_threads = 32
io_buffer_size = "256KB"
socket_buffer_size = "1MB"

[monitoring]
enable_metrics = true
metrics_bind_addr = "0.0.0.0:9090"
node_id = "node-1"
cluster_name = "multivm-production"

[storage]
data_dir = "/var/lib/multivm"
backup_dir = "/mnt/shared/backups/node-1"
enable_backup = true
backup_interval = "1h"
```

### Load Balancer Configuration

HAProxy configuration for high availability:

```
# /etc/haproxy/haproxy.cfg
global
    maxconn 4096
    log stdout local0
    user haproxy
    group haproxy
    daemon

defaults
    mode http
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 50000ms
    option httplog

# Health check endpoint
frontend multivm_health
    bind *:8080
    option httplog
    default_backend multivm_health_backend

backend multivm_health_backend
    balance roundrobin
    option httpchk GET /health
    http-check expect status 200
    server node1 10.0.2.10:8080 check inter 10s fall 3 rise 2
    server node2 10.0.2.11:8080 check inter 10s fall 3 rise 2
    server node3 10.0.2.12:8080 check inter 10s fall 3 rise 2
    server node4 10.0.2.13:8080 check inter 10s fall 3 rise 2 backup
    server node5 10.0.2.14:8080 check inter 10s fall 3 rise 2 backup

# Metrics endpoint (internal only)
frontend multivm_metrics
    bind 127.0.0.1:9090
    default_backend multivm_metrics_backend

backend multivm_metrics_backend
    balance roundrobin
    server node1 10.0.2.10:9090 check
    server node2 10.0.2.11:9090 check
    server node3 10.0.2.12:9090 check

# Statistics
stats enable
stats uri /stats
stats refresh 30s
stats admin if TRUE
```

### Cluster Management Scripts

#### Cluster Health Check Script

```bash
#!/bin/bash
# cluster-health.sh - Check cluster health

NODES=(
    "10.0.2.10:8080"
    "10.0.2.11:8080"
    "10.0.2.12:8080"
    "10.0.2.13:8080"
    "10.0.2.14:8080"
    "10.0.2.15:8080"
    "10.0.2.16:8080"
)

echo "🏥 MultiVM Cluster Health Check"
echo "==============================="
echo "Timestamp: $(date)"
echo ""

HEALTHY_NODES=0
TOTAL_NODES=${#NODES[@]}

for node in "${NODES[@]}"; do
    echo -n "Checking $node... "
    
    if curl -sf "http://$node/health" > /dev/null 2>&1; then
        echo "✅ HEALTHY"
        ((HEALTHY_NODES++))
    else
        echo "❌ UNHEALTHY"
        
        # Get detailed status
        echo "   Details:"
        curl -s "http://$node/health/detailed" | jq '.' 2>/dev/null || echo "   No response"
    fi
done

echo ""
echo "Summary: $HEALTHY_NODES/$TOTAL_NODES nodes healthy"

# Calculate consensus health
CONSENSUS_THRESHOLD=$((TOTAL_NODES * 2 / 3 + 1))
if [ $HEALTHY_NODES -ge $CONSENSUS_THRESHOLD ]; then
    echo "✅ Consensus: HEALTHY (>= 2/3 nodes)"
    exit 0
else
    echo "❌ Consensus: UNHEALTHY (< 2/3 nodes)"
    exit 1
fi
```

#### Rolling Update Script

```bash
#!/bin/bash
# rolling-update.sh - Perform rolling update

NEW_VERSION="$1"
if [ -z "$NEW_VERSION" ]; then
    echo "Usage: $0 <new-version>"
    exit 1
fi

NODES=(
    "10.0.2.10"
    "10.0.2.11" 
    "10.0.2.12"
    "10.0.2.13"
    "10.0.2.14"
    "10.0.2.15"
    "10.0.2.16"
)

echo "🔄 Rolling Update to Version $NEW_VERSION"
echo "========================================"

# Update nodes one by one
for node in "${NODES[@]}"; do
    echo "Updating node $node..."
    
    # Health check before update
    if ! curl -sf "http://$node:8080/health" > /dev/null; then
        echo "❌ Node $node is unhealthy, skipping update"
        continue
    fi
    
    # Download new version to node
    ssh "multivm@$node" "
        cd /tmp
        wget https://github.com/your-org/multivm-process/releases/download/$NEW_VERSION/multivm-process-$NEW_VERSION-linux-x86_64.tar.gz
        tar -xzf multivm-process-$NEW_VERSION-linux-x86_64.tar.gz
    "
    
    # Stop service gracefully
    echo "  Stopping service..."
    ssh "multivm@$node" "sudo systemctl stop multivm-process"
    
    # Backup current binary
    ssh "multivm@$node" "sudo cp /opt/multivm/bin/multivm-process /opt/multivm/bin/multivm-process.backup"
    
    # Install new binary
    echo "  Installing new binary..."
    ssh "multivm@$node" "
        sudo cp /tmp/multivm-process /opt/multivm/bin/
        sudo chmod +x /opt/multivm/bin/multivm-process
    "
    
    # Start service
    echo "  Starting service..."
    ssh "multivm@$node" "sudo systemctl start multivm-process"
    
    # Wait for health check
    echo "  Waiting for health check..."
    for i in {1..30}; do
        if curl -sf "http://$node:8080/health" > /dev/null; then
            echo "  ✅ Node $node updated successfully"
            break
        fi
        sleep 2
    done
    
    # Verify cluster consensus
    if ! ./cluster-health.sh > /dev/null; then
        echo "❌ Cluster consensus lost, rolling back node $node"
        ssh "multivm@$node" "
            sudo systemctl stop multivm-process
            sudo cp /opt/multivm/bin/multivm-process.backup /opt/multivm/bin/multivm-process
            sudo systemctl start multivm-process
        "
        exit 1
    fi
    
    echo "  ✅ Node $node update complete"
    echo ""
done

echo "🎉 Rolling update completed successfully"
```

## Container Deployment

### Docker Configuration

Production-ready Dockerfile:

```dockerfile
# Multi-stage build for minimal production image
FROM rust:1.70-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1001 multivm

# Set working directory
WORKDIR /app

# Copy source code
COPY . .

# Build application
RUN cargo build --release --bin multivm-process

# Production image
FROM debian:12-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1001 multivm

# Create directories
RUN mkdir -p /opt/multivm/{config,data,logs} && \
    chown -R multivm:multivm /opt/multivm

# Copy binary from builder
COPY --from=builder /app/target/release/multivm-process /usr/local/bin/
RUN chmod +x /usr/local/bin/multivm-process

# Copy configuration
COPY --chown=multivm:multivm config/container.toml /opt/multivm/config/multivm.toml

# Set up volumes
VOLUME ["/opt/multivm/data", "/opt/multivm/logs"]

# Security settings
USER multivm
WORKDIR /opt/multivm

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose ports
EXPOSE 8080 9090 26657

# Start application
CMD ["multivm-process", "--config", "/opt/multivm/config/multivm.toml"]
```

### Docker Compose Deployment

Production Docker Compose setup:

```yaml
# docker-compose.yml
version: '3.8'

services:
  multivm-node-1:
    image: multivm-process:latest
    container_name: multivm-node-1
    restart: unless-stopped
    networks:
      - multivm-consensus
      - multivm-public
    ports:
      - "8080:8080"
      - "9090:9090"
      - "26657:26657"
    volumes:
      - ./config/node-1.toml:/opt/multivm/config/multivm.toml:ro
      - ./certs:/opt/multivm/certs:ro
      - multivm-node-1-data:/opt/multivm/data
      - multivm-node-1-logs:/opt/multivm/logs
    environment:
      - RUST_LOG=info
      - MULTIVM_NODE_ID=node-1
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s
    security_opt:
      - no-new-privileges:true
    read_only: true
    tmpfs:
      - /tmp:noexec,nosuid,size=100m

  multivm-node-2:
    image: multivm-process:latest
    container_name: multivm-node-2
    restart: unless-stopped
    networks:
      - multivm-consensus
      - multivm-public
    ports:
      - "8081:8080"
      - "9091:9090"
      - "26658:26657"
    volumes:
      - ./config/node-2.toml:/opt/multivm/config/multivm.toml:ro
      - ./certs:/opt/multivm/certs:ro
      - multivm-node-2-data:/opt/multivm/data
      - multivm-node-2-logs:/opt/multivm/logs
    environment:
      - RUST_LOG=info
      - MULTIVM_NODE_ID=node-2
    depends_on:
      - multivm-node-1

  multivm-node-3:
    image: multivm-process:latest
    container_name: multivm-node-3
    restart: unless-stopped
    networks:
      - multivm-consensus
      - multivm-public
    ports:
      - "8082:8080"
      - "9092:9090"
      - "26659:26657"
    volumes:
      - ./config/node-3.toml:/opt/multivm/config/multivm.toml:ro
      - ./certs:/opt/multivm/certs:ro
      - multivm-node-3-data:/opt/multivm/data
      - multivm-node-3-logs:/opt/multivm/logs
    environment:
      - RUST_LOG=info
      - MULTIVM_NODE_ID=node-3
    depends_on:
      - multivm-node-1

  # Load balancer
  haproxy:
    image: haproxy:2.8-alpine
    container_name: multivm-lb
    restart: unless-stopped
    networks:
      - multivm-public
    ports:
      - "80:80"
      - "8404:8404"  # HAProxy stats
    volumes:
      - ./haproxy.cfg:/usr/local/etc/haproxy/haproxy.cfg:ro
    depends_on:
      - multivm-node-1
      - multivm-node-2
      - multivm-node-3

  # Monitoring
  prometheus:
    image: prom/prometheus:latest
    container_name: multivm-prometheus
    restart: unless-stopped
    networks:
      - multivm-public
    ports:
      - "9100:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'

  grafana:
    image: grafana/grafana:latest
    container_name: multivm-grafana
    restart: unless-stopped
    networks:
      - multivm-public
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=secure_password_here
    depends_on:
      - prometheus

networks:
  multivm-consensus:
    driver: bridge
    internal: true
  multivm-public:
    driver: bridge

volumes:
  multivm-node-1-data:
  multivm-node-1-logs:
  multivm-node-2-data:
  multivm-node-2-logs:
  multivm-node-3-data:
  multivm-node-3-logs:
  prometheus-data:
  grafana-data:
```

## Kubernetes Deployment

### Kubernetes Manifests

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: multivm-system
  labels:
    name: multivm-system

---
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: multivm-config
  namespace: multivm-system
data:
  multivm.toml: |
    [coordinator]
    health_check_interval = "30s"
    block_timeout = "60s"
    max_concurrent_blocks = 20
    
    [consensus]
    validator_id = "validator-k8s"
    listen_addr = "0.0.0.0:26657"
    timeout_ms = 5000
    
    [security]
    enable_authentication = true
    enable_encryption = true
    
    [monitoring]
    enable_metrics = true
    metrics_bind_addr = "0.0.0.0:9090"
    health_check_bind_addr = "0.0.0.0:8080"

---
# secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: multivm-certs
  namespace: multivm-system
type: Opaque
data:
  tls.crt: # base64 encoded certificate
  tls.key: # base64 encoded private key
  ca.crt:  # base64 encoded CA certificate

---
# statefulset.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: multivm-process
  namespace: multivm-system
  labels:
    app: multivm-process
spec:
  serviceName: multivm-process-headless
  replicas: 3
  selector:
    matchLabels:
      app: multivm-process
  template:
    metadata:
      labels:
        app: multivm-process
    spec:
      securityContext:
        runAsUser: 1001
        runAsGroup: 1001
        fsGroup: 1001
      containers:
      - name: multivm-process
        image: multivm-process:latest
        imagePullPolicy: Always
        ports:
        - containerPort: 8080
          name: health
        - containerPort: 9090
          name: metrics
        - containerPort: 26657
          name: consensus
        env:
        - name: RUST_LOG
          value: "info"
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_IP
          valueFrom:
            fieldRef:
              fieldPath: status.podIP
        volumeMounts:
        - name: config
          mountPath: /opt/multivm/config
          readOnly: true
        - name: certs
          mountPath: /opt/multivm/certs
          readOnly: true
        - name: data
          mountPath: /opt/multivm/data
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 60
          periodSeconds: 30
          timeoutSeconds: 10
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        securityContext:
          allowPrivilegeEscalation: false
          capabilities:
            drop:
            - ALL
          readOnlyRootFilesystem: true
          runAsNonRoot: true
      volumes:
      - name: config
        configMap:
          name: multivm-config
      - name: certs
        secret:
          secretName: multivm-certs
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: [ "ReadWriteOnce" ]
      storageClassName: "fast-ssd"
      resources:
        requests:
          storage: 100Gi

---
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: multivm-process-headless
  namespace: multivm-system
  labels:
    app: multivm-process
spec:
  clusterIP: None
  selector:
    app: multivm-process
  ports:
  - name: consensus
    port: 26657
    targetPort: 26657

---
apiVersion: v1
kind: Service
metadata:
  name: multivm-process
  namespace: multivm-system
  labels:
    app: multivm-process
spec:
  selector:
    app: multivm-process
  ports:
  - name: health
    port: 8080
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090

---
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: multivm-process
  namespace: multivm-system
  annotations:
    kubernetes.io/ingress.class: "nginx"
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/backend-protocol: "HTTP"
    nginx.ingress.kubernetes.io/rate-limit: "100"
spec:
  tls:
  - hosts:
    - multivm.example.com
    secretName: multivm-tls
  rules:
  - host: multivm.example.com
    http:
      paths:
      - path: /health
        pathType: Prefix
        backend:
          service:
            name: multivm-process
            port:
              number: 8080

---
# hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: multivm-process
  namespace: multivm-system
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: StatefulSet
    name: multivm-process
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

### Kubernetes Deployment Script

```bash
#!/bin/bash
# deploy-k8s.sh - Deploy to Kubernetes

set -e

echo "🚀 Deploying MultiVM Process to Kubernetes"
echo "==========================================="

# Check kubectl connectivity
if ! kubectl cluster-info > /dev/null 2>&1; then
    echo "❌ Cannot connect to Kubernetes cluster"
    exit 1
fi

# Create namespace
echo "📁 Creating namespace..."
kubectl apply -f k8s/namespace.yaml

# Create secrets (certificates)
echo "🔐 Creating certificates..."
if [ ! -f certs/tls.crt ]; then
    echo "Generating self-signed certificates..."
    ./scripts/generate-k8s-certs.sh
fi

kubectl create secret tls multivm-certs \
    --cert=certs/tls.crt \
    --key=certs/tls.key \
    -n multivm-system \
    --dry-run=client -o yaml | kubectl apply -f -

# Apply configuration
echo "⚙️  Applying configuration..."
kubectl apply -f k8s/configmap.yaml

# Deploy application
echo "🎯 Deploying application..."
kubectl apply -f k8s/statefulset.yaml
kubectl apply -f k8s/service.yaml
kubectl apply -f k8s/ingress.yaml
kubectl apply -f k8s/hpa.yaml

# Wait for deployment
echo "⏳ Waiting for deployment to be ready..."
kubectl wait --for=condition=ready pod -l app=multivm-process -n multivm-system --timeout=300s

# Verify deployment
echo "✅ Verifying deployment..."
kubectl get pods -n multivm-system
kubectl get svc -n multivm-system

# Test health endpoint
echo "🏥 Testing health endpoint..."
kubectl port-forward svc/multivm-process 8080:8080 -n multivm-system &
sleep 5
if curl -f http://localhost:8080/health; then
    echo "✅ Health check passed"
else
    echo "❌ Health check failed"
fi
pkill -f "kubectl port-forward"

echo "🎉 Deployment completed successfully!"
echo ""
echo "Access the application:"
echo "  Health: https://multivm.example.com/health"
echo "  Metrics: kubectl port-forward svc/multivm-process 9090:9090 -n multivm-system"
echo ""
echo "Monitor the deployment:"
echo "  kubectl logs -f -l app=multivm-process -n multivm-system"
echo "  kubectl get pods -n multivm-system -w"
```

## Monitoring & Observability

### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "multivm_rules.yml"

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093

scrape_configs:
  - job_name: 'multivm-process'
    static_configs:
      - targets: 
        - 'multivm-node-1:9090'
        - 'multivm-node-2:9090'
        - 'multivm-node-3:9090'
    scrape_interval: 10s
    metrics_path: /metrics
    
  - job_name: 'multivm-health'
    static_configs:
      - targets:
        - 'multivm-node-1:8080'
        - 'multivm-node-2:8080'
        - 'multivm-node-3:8080'
    scrape_interval: 30s
    metrics_path: /health/metrics
```

### Alerting Rules

```yaml
# multivm_rules.yml
groups:
- name: multivm.rules
  rules:
  - alert: MultivmNodeDown
    expr: up{job="multivm-process"} == 0
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "MultiVM node is down"
      description: "MultiVM node {{ $labels.instance }} has been down for more than 5 minutes."

  - alert: MultivmHighCPU
    expr: cpu_usage_percent{job="multivm-process"} > 90
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "High CPU usage on MultiVM node"
      description: "CPU usage on {{ $labels.instance }} is {{ $value }}% for more than 10 minutes."

  - alert: MultivmConsensusFailure
    expr: consensus_leader_changes > 5
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "Frequent consensus leader changes"
      description: "Consensus has changed leaders {{ $value }} times in the last 5 minutes."
```

---

Next: [Monitoring Guide](MONITORING.md) | [Troubleshooting Guide](TROUBLESHOOTING.md)