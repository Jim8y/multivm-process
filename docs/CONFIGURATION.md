# MultiVM Configuration Guide

This document describes all configuration options available for the MultiVM system.

## Environment Variables

MultiVM supports configuration through environment variables. All environment variables follow the pattern `MULTIVM_<COMPONENT>_<FIELD>`.

### Authentication

| Variable | Description | Default | Required in Production |
|----------|-------------|---------|----------------------|
| `MULTIVM_JWT_SECRET` | JWT signing secret (min 32 chars) | Generated | **Yes** |
| `MULTIVM_JWT_EXPIRATION_HOURS` | JWT token expiration time | 24 | No |
| `MULTIVM_ENABLE_API_KEYS` | Enable API key authentication | true | No |

### Server Ports

| Variable | Description | Default |
|----------|-------------|---------|
| `MULTIVM_REST_PORT` | REST API server port | 8080 |
| `MULTIVM_GRAPHQL_PORT` | GraphQL server port | 8081 |
| `MULTIVM_WEBSOCKET_PORT` | WebSocket server port | 8082 |
| `MULTIVM_ADMIN_PORT` | Admin interface port | 8083 |

### Monitoring

| Variable | Description | Default |
|----------|-------------|---------|
| `MULTIVM_LOG_LEVEL` | Logging level (trace/debug/info/warn/error) | info |
| `MULTIVM_ENABLE_METRICS` | Enable metrics collection | true |

### Cache

| Variable | Description | Default |
|----------|-------------|---------|
| `MULTIVM_REDIS_URL` | Redis connection URL | redis://localhost:6379 |
| `MULTIVM_CACHE_TTL_SECONDS` | Default cache TTL | 300 |

### Features

| Variable | Description | Default |
|----------|-------------|---------|
| `MULTIVM_ENABLE_EXPERIMENTAL` | Enable experimental features | false |

### Environment

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_ENV` | Runtime environment (development/production) | development |

## Configuration Validation

The system performs comprehensive validation on startup:

### Port Validation
- All ports must be between 1 and 65535
- No two services can use the same port
- Ports are checked for conflicts across all services

### Security Validation
- JWT secret must be at least 32 characters
- JWT expiration must be between 1 hour and 30 days
- Admin interface requires authentication in production

### Resource Limits
- Request timeout: 1-300 seconds
- WebSocket connections: > 0
- GraphQL query depth: > 0
- Cache TTL: > 0 seconds

### URL Validation
- Redis URLs must start with `redis://` or `rediss://`
- All URLs are validated for proper format

## Configuration Files

Configuration can also be loaded from TOML files:

```toml
# config.toml example
[auth]
jwt_secret = "your-secret-key-at-least-32-characters-long"
jwt_expiration_hours = 24
enable_api_keys = true

[server.rest]
host = "0.0.0.0"
port = 8080
enable_cors = false
request_timeout_seconds = 30

[cache.redis]
url = "redis://localhost:6379"
key_prefix = "multivm:app:"

[monitoring]
enable_metrics = true
log_level = "info"
```

## Security Best Practices

1. **Always set `MULTIVM_JWT_SECRET` in production** - Never use the default generated secret
2. **Use strong secrets** - JWT secrets should be at least 32 characters of random data
3. **Restrict CORS origins** - Don't use wildcard (*) in production
4. **Enable authentication** - Ensure admin interface requires authentication
5. **Use HTTPS** - Configure TLS for all production deployments
6. **Rotate secrets regularly** - Use the JWT secret rotation feature for enhanced security

## Default Values

The system provides sensible defaults for development, but these should be reviewed for production:

- **Ports**: Chosen to avoid conflicts with common services
- **Timeouts**: Balanced for responsiveness vs resource usage  
- **Limits**: Set to handle moderate load, adjust based on your needs
- **Security**: Defaults favor security over convenience