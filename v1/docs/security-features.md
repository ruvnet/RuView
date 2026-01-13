# WiFi-DensePose Security Features Documentation

## Overview

This document details the authentication and rate limiting features implemented in the WiFi-DensePose API, including configuration options, usage examples, and security best practices.

## Table of Contents

1. [Authentication](#authentication)
2. [Rate Limiting](#rate-limiting)
3. [CORS Configuration](#cors-configuration)
4. [Security Headers](#security-headers)
5. [Configuration](#configuration)
6. [Testing](#testing)
7. [Best Practices](#best-practices)

## Authentication

### JWT Authentication

The API uses JWT (JSON Web Token) based authentication for securing endpoints.

#### Features

- **Token-based authentication**: Stateless authentication using JWT tokens
- **Role-based access control**: Support for different user roles (admin, user)
- **Token expiration**: Configurable token lifetime
- **Refresh token support**: Ability to refresh expired tokens
- **Multiple authentication sources**: Support for headers, query params, and cookies

#### Implementation Details

```python
# Location: src/api/middleware/auth.py
class AuthMiddleware(BaseHTTPMiddleware):
    """JWT Authentication middleware."""
```

**Public Endpoints** (No authentication required):
- `/` - Root endpoint
- `/health`, `/ready`, `/live` - Health check endpoints
- `/docs`, `/redoc`, `/openapi.json` - API documentation
- `/api/v1/pose/current` - Current pose data
- `/api/v1/pose/zones/*` - Zone information
- `/api/v1/pose/activities` - Activity data
- `/api/v1/pose/stats` - Statistics
- `/api/v1/stream/status` - Stream status

**Protected Endpoints** (Authentication required):
- `/api/v1/pose/analyze` - Pose analysis
- `/api/v1/pose/calibrate` - System calibration
- `/api/v1/pose/historical` - Historical data
- `/api/v1/stream/start` - Start streaming
- `/api/v1/stream/stop` - Stop streaming
- `/api/v1/stream/clients` - Client management
- `/api/v1/stream/broadcast` - Broadcasting

#### Usage Examples

**1. Obtaining a Token:**
```bash
# Login endpoint (if implemented)
curl -X POST http://localhost:8000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "user", "password": "password"}'
```

**2. Using Bearer Token:**
```bash
# Authorization header
curl -X POST http://localhost:8000/api/v1/pose/analyze \
  -H "Authorization: Bearer <your-jwt-token>" \
  -H "Content-Type: application/json" \
  -d '{"data": "..."}'
```

**3. WebSocket Authentication:**
```javascript
// Query parameter for WebSocket
const ws = new WebSocket('ws://localhost:8000/ws/pose?token=<your-jwt-token>');
```

### API Key Authentication

Alternative authentication method for service-to-service communication.

```python
# Location: src/api/middleware/auth.py
class APIKeyAuth:
    """Alternative API key authentication for service-to-service communication."""
```

**Features:**
- Simple key-based authentication
- Service identification
- Key management (add/revoke)

**Usage:**
```bash
# API Key in header
curl -X GET http://localhost:8000/api/v1/pose/current \
  -H "X-API-Key: your-api-key-here"
```

### Token Blacklist

Support for token revocation and logout functionality.

```python
class TokenBlacklist:
    """Simple in-memory token blacklist for logout functionality."""
```

## Rate Limiting

### Overview

The API implements sophisticated rate limiting using a sliding window algorithm with support for different user tiers.

#### Features

- **Sliding window algorithm**: Accurate request counting
- **Token bucket algorithm**: Alternative rate limiting method
- **User-based limits**: Different limits for anonymous/authenticated/admin users
- **Path-specific limits**: Custom limits for specific endpoints
- **Adaptive rate limiting**: Adjust limits based on system load
- **Temporary blocking**: Block clients after excessive violations

#### Implementation Details

```python
# Location: src/api/middleware/rate_limit.py
class RateLimitMiddleware(BaseHTTPMiddleware):
    """Rate limiting middleware with sliding window algorithm."""
```

**Default Rate Limits:**
- Anonymous users: 100 requests/hour (configurable)
- Authenticated users: 1000 requests/hour (configurable)
- Admin users: 10000 requests/hour

**Path-Specific Limits:**
- `/api/v1/pose/current`: 60 requests/minute
- `/api/v1/pose/analyze`: 10 requests/minute
- `/api/v1/pose/calibrate`: 1 request/5 minutes
- `/api/v1/stream/start`: 5 requests/minute
- `/api/v1/stream/stop`: 5 requests/minute

#### Response Headers

Rate limit information is included in response headers:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Window: 3600
X-RateLimit-Reset: 1641234567
```

When rate limit is exceeded:
```
HTTP/1.1 429 Too Many Requests
Retry-After: 60
X-RateLimit-Limit: Exceeded
X-RateLimit-Remaining: 0
```

### Adaptive Rate Limiting

The system can adjust rate limits based on system load:

```python
class AdaptiveRateLimit:
    """Adaptive rate limiting based on system load."""
```

**Load-based adjustments:**
- High load (>80%): Reduce limits by 50%
- Medium load (>60%): Reduce limits by 30%
- Low load (<30%): Increase limits by 20%

## CORS Configuration

### Overview

Cross-Origin Resource Sharing (CORS) configuration for browser-based clients.

#### Features

- **Configurable origins**: Whitelist specific origins
- **Wildcard support**: Allow all origins in development
- **Preflight handling**: Proper OPTIONS request handling
- **Credential support**: Allow cookies and auth headers
- **Custom headers**: Expose rate limit and other headers

#### Configuration

```python
# Development configuration
cors_config = {
    "allow_origins": ["*"],
    "allow_credentials": True,
    "allow_methods": ["*"],
    "allow_headers": ["*"]
}

# Production configuration
cors_config = {
    "allow_origins": ["https://app.example.com", "https://admin.example.com"],
    "allow_credentials": True,
    "allow_methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    "allow_headers": ["Authorization", "Content-Type"]
}
```

## Security Headers

The API includes various security headers for enhanced protection:

```python
class SecurityHeaders:
    """Security headers for API responses."""
```

**Headers included:**
- `X-Content-Type-Options: nosniff` - Prevent MIME sniffing
- `X-Frame-Options: DENY` - Prevent clickjacking
- `X-XSS-Protection: 1; mode=block` - Enable XSS protection
- `Referrer-Policy: strict-origin-when-cross-origin` - Control referrer
- `Content-Security-Policy` - Control resource loading

## Configuration

### Environment Variables

```bash
# Authentication
ENABLE_AUTHENTICATION=true
SECRET_KEY=your-secret-key-here
JWT_ALGORITHM=HS256
JWT_EXPIRE_HOURS=24

# Rate Limiting
ENABLE_RATE_LIMITING=true
RATE_LIMIT_REQUESTS=100
RATE_LIMIT_AUTHENTICATED_REQUESTS=1000
RATE_LIMIT_WINDOW=3600

# CORS
CORS_ENABLED=true
CORS_ORIGINS=["https://app.example.com"]
CORS_ALLOW_CREDENTIALS=true

# Security
ALLOWED_HOSTS=["api.example.com", "localhost"]
```

### Settings Class

```python
# src/config/settings.py
class Settings(BaseSettings):
    # Authentication settings
    enable_authentication: bool = Field(default=True)
    secret_key: str = Field(...)
    jwt_algorithm: str = Field(default="HS256")
    jwt_expire_hours: int = Field(default=24)
    
    # Rate limiting settings
    enable_rate_limiting: bool = Field(default=True)
    rate_limit_requests: int = Field(default=100)
    rate_limit_authenticated_requests: int = Field(default=1000)
    rate_limit_window: int = Field(default=3600)
    
    # CORS settings
    cors_enabled: bool = Field(default=True)
    cors_origins: List[str] = Field(default=["*"])
    cors_allow_credentials: bool = Field(default=True)
```

## Testing

### Test Script

A comprehensive test script is provided to verify security features:

```bash
# Run the test script
python test_auth_rate_limit.py
```

The test script covers:
- Public endpoint access
- Protected endpoint authentication
- JWT token validation
- Rate limiting behavior
- CORS headers
- Security headers
- Feature flag verification

### Manual Testing

**1. Test Authentication:**
```bash
# Without token (should fail)
curl -X POST http://localhost:8000/api/v1/pose/analyze

# With token (should succeed)
curl -X POST http://localhost:8000/api/v1/pose/analyze \
  -H "Authorization: Bearer <token>"
```

**2. Test Rate Limiting:**
```bash
# Send multiple requests quickly
for i in {1..150}; do
  curl -s -o /dev/null -w "%{http_code}\n" \
    http://localhost:8000/api/v1/pose/current
done
```

**3. Test CORS:**
```bash
# Preflight request
curl -X OPTIONS http://localhost:8000/api/v1/pose/current \
  -H "Origin: https://example.com" \
  -H "Access-Control-Request-Method: GET" \
  -H "Access-Control-Request-Headers: Authorization"
```

## Best Practices

### Security Recommendations

1. **Production Configuration:**
   - Always use strong secret keys
   - Disable debug mode
   - Restrict CORS origins
   - Use HTTPS only
   - Enable all security headers

2. **Token Management:**
   - Implement token refresh mechanism
   - Use short-lived tokens
   - Implement logout/blacklist functionality
   - Store tokens securely on client

3. **Rate Limiting:**
   - Set appropriate limits for your use case
   - Monitor and adjust based on usage
   - Implement different tiers for users
   - Use Redis for distributed systems

4. **API Keys:**
   - Use for service-to-service communication
   - Rotate keys regularly
   - Monitor key usage
   - Implement key scoping

### Monitoring

1. **Authentication Events:**
   - Log failed authentication attempts
   - Monitor suspicious patterns
   - Alert on repeated failures

2. **Rate Limit Violations:**
   - Track clients hitting limits
   - Identify potential abuse
   - Adjust limits as needed

3. **Security Headers:**
   - Verify headers in responses
   - Test with security tools
   - Regular security audits

### Troubleshooting

**Common Issues:**

1. **401 Unauthorized:**
   - Check token format
   - Verify token expiration
   - Ensure correct secret key

2. **429 Too Many Requests:**
   - Check rate limit configuration
   - Verify client identification
   - Look for Retry-After header

3. **CORS Errors:**
   - Verify allowed origins
   - Check preflight responses
   - Ensure credentials setting matches

## Disabling Security Features

For development or testing, security features can be disabled:

```bash
# Disable authentication
ENABLE_AUTHENTICATION=false

# Disable rate limiting
ENABLE_RATE_LIMITING=false

# Allow all CORS origins
CORS_ORIGINS=["*"]
```

**Warning:** Never disable security features in production!

## Future Enhancements

1. **OAuth2/OpenID Connect Support**
2. **API Key Scoping and Permissions**
3. **IP-based Rate Limiting**
4. **Geographic Restrictions**
5. **Request Signing**
6. **Mutual TLS Authentication**