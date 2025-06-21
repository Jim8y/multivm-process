# Environment-Based Authentication

This document describes the production-ready environment-based authentication system for MultiVM.

## Overview

The MultiVM application now supports secure environment-based authentication using:
- **Environment variables** for API key validation
- **Secure JWT secret loading** from environment variables
- **Admin API key bootstrap** mechanism
- **Production-ready security defaults**

## Environment Variables

### JWT Configuration

```bash
# Required in production - JWT secret must be at least 32 characters
export MULTIVM_JWT_SECRET="your-super-secure-jwt-secret-key-that-is-at-least-32-characters-long"

# Optional - JWT token expiration in seconds (default: 3600)
export MULTIVM_JWT_EXPIRATION="7200"
```

### API Key Configuration

```bash
# Enable API key authentication (default: true)
export MULTIVM_ENABLE_API_KEYS="true"

# Set API key validation method to environment
export MULTIVM_API_KEY_VALIDATION="environment"

# Admin API key for bootstrap access
export MULTIVM_ADMIN_API_KEY="admin_key_123456789abcdef"

# Optional admin contact info
export MULTIVM_ADMIN_EMAIL="admin@example.com"
export MULTIVM_ADMIN_IP_WHITELIST="192.168.1.0/24,10.0.0.0/8"
```

### API Key Definitions

API keys are defined using environment variables with the pattern:
```bash
MULTIVM_API_KEY_<NAME>=key:user_id:role:permissions[:rate_limit[:email[:ip_whitelist]]]
```

#### Examples:

```bash
# Basic user key
export MULTIVM_API_KEY_USER1="user_key_123:user1:user:read_system_status,read_network_info:1000"

# Power user with more permissions
export MULTIVM_API_KEY_POWERUSER="power_key_456:poweruser1:poweruser:read_system_status,read_network_info,read_blockchain_data,write_transactions:5000:poweruser@example.com"

# Admin user with all permissions
export MULTIVM_API_KEY_ADMIN="admin_key_789:admin1:admin:*:10000:admin@example.com:192.168.1.0/24"

# Service account with specific permissions
export MULTIVM_API_KEY_SERVICE="service_key_abc:service1:user:read_blockchain_data,cross_vm_operations:2000:service@example.com:10.0.0.100"
```

## User Roles

| Role | Description | Default Permissions |
|------|-------------|-------------------|
| `guest` | Read-only access | `read_system_status`, `read_network_info` |
| `user` | Standard user | All read permissions, basic write operations |
| `poweruser` | Advanced user | Extended permissions including cross-VM operations |
| `admin` | Administrator | Most administrative permissions |
| `superadmin` | Super administrator | All permissions |

## Permissions

### System Permissions
- `read_system_status` - Read system health and status
- `read_network_info` - Read network information
- `read_blockchain_data` - Read blockchain data
- `read_metrics` - Read system metrics

### Operational Permissions
- `write_transactions` - Submit transactions
- `cross_vm_operations` - Perform cross-VM operations
- `admin_operations` - Administrative operations

### Administrative Permissions
- `manage_api_keys` - Manage API keys
- `manage_users` - Manage users
- `system_configuration` - System configuration
- `manage_consensus` - Manage consensus
- `manage_p2p` - Manage P2P networking
- `write_configuration` - Write configuration

### Special Values
- `*` or `all` - All available permissions

## Production Deployment

### Docker Environment

```dockerfile
# In your Dockerfile or docker-compose.yml
ENV RUST_ENV=production
ENV MULTIVM_JWT_SECRET=your-production-jwt-secret-key-32-chars-minimum
ENV MULTIVM_API_KEY_VALIDATION=environment
ENV MULTIVM_ADMIN_API_KEY=your-secure-admin-key
ENV MULTIVM_API_KEY_SERVICE=service_key:service_user:user:read_blockchain_data:5000
```

### Kubernetes Secrets

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: multivm-auth
type: Opaque
stringData:
  MULTIVM_JWT_SECRET: "your-production-jwt-secret-key-32-chars-minimum"
  MULTIVM_ADMIN_API_KEY: "your-secure-admin-key"
  MULTIVM_API_KEY_SERVICE: "service_key:service_user:user:read_blockchain_data:5000"
```

### Environment File

```bash
# .env.production
RUST_ENV=production
MULTIVM_JWT_SECRET=your-production-jwt-secret-key-32-chars-minimum
MULTIVM_API_KEY_VALIDATION=environment
MULTIVM_ADMIN_API_KEY=your-secure-admin-key
MULTIVM_API_KEY_USER1=user_key_123:user1:user:read_system_status,read_network_info:1000
MULTIVM_API_KEY_SERVICE=service_key_abc:service1:user:read_blockchain_data,cross_vm_operations:2000
```

## Security Features

### 🔒 **Production Safety**
- **Required JWT secret** in production environment
- **Minimum 32-character** JWT secret validation
- **No default secrets** in production builds
- **Environment-only** API key storage

### 🛡️ **Access Control**
- **Role-based permissions** system
- **IP address whitelisting** support
- **Rate limiting** per API key
- **Granular permission** control

### 🔍 **Monitoring & Auditing**
- **Usage statistics** tracking
- **Authentication logging**
- **Failed attempt** monitoring
- **Key usage** analytics

## API Usage

### Using API Keys

```bash
# Via header
curl -H "X-API-Key: your-api-key" https://api.multivm.example.com/v1/status

# Via query parameter
curl "https://api.multivm.example.com/v1/status?api_key=your-api-key"
```

### Using JWT Tokens

```bash
# Get token first (admin endpoint)
TOKEN=$(curl -X POST -H "X-API-Key: admin_key" \
  https://api.multivm.example.com/auth/token \
  -d '{"user_id":"user123","role":"user"}' | jq -r '.token')

# Use token
curl -H "Authorization: Bearer $TOKEN" \
  https://api.multivm.example.com/v1/blockchain/svm/accounts
```

## Migration from Database/Redis

To migrate from database or Redis-based API key validation to environment-based:

1. **Export existing keys** to environment variables format
2. **Update configuration** to use `environment` validation
3. **Set environment variables** in your deployment
4. **Restart the application**

## Troubleshooting

### Common Issues

1. **JWT secret too short**
   ```
   Error: JWT secret must be at least 32 characters long
   ```
   **Solution**: Use a longer JWT secret (minimum 32 characters)

2. **Invalid permission**
   ```
   Error: Unknown permission: invalid_permission
   ```
   **Solution**: Use valid permission names from the list above

3. **Invalid role**
   ```
   Error: Invalid role: invalid_role
   ```
   **Solution**: Use: guest, user, poweruser, admin, or superadmin

4. **Production secret not set**
   ```
   Error: MULTIVM_JWT_SECRET environment variable must be set in production
   ```
   **Solution**: Set the MULTIVM_JWT_SECRET environment variable

### Debug Mode

Set `RUST_LOG=debug` to see detailed authentication logs:

```bash
export RUST_LOG=multivm_application::auth=debug
./multivm-application
```

## Best Practices

1. **Use strong, unique API keys** (minimum 32 characters)
2. **Rotate keys regularly** in production
3. **Use IP whitelisting** for service accounts
4. **Monitor API key usage** and disable unused keys
5. **Use separate keys** for different services/users
6. **Store secrets securely** (vault, k8s secrets, etc.)
7. **Never commit secrets** to version control
8. **Use least privilege** permission model