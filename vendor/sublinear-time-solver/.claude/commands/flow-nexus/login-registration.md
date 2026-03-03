---
name: flow-nexus-auth
description: Flow Nexus authentication and user management
---

# Flow Nexus Authentication

Quick commands for Flow Nexus login and registration.

## Register New Account
```javascript
mcp__flow-nexus__user_register({
  email: "user@example.com",
  password: "secure_password",
  full_name: "Your Name" // optional
})
```

## Login
```javascript
mcp__flow-nexus__user_login({
  email: "user@example.com",
  password: "your_password"
})
```

## Check Auth Status
```javascript
mcp__flow-nexus__auth_status({ detailed: true })
```

## Logout
```javascript
mcp__flow-nexus__user_logout()
```

## Password Reset
```javascript
// Request reset
mcp__flow-nexus__user_reset_password({ email: "user@example.com" })

// Update with token
mcp__flow-nexus__user_update_password({
  token: "reset_token",
  new_password: "new_secure_password"
})
```

## Profile Management
```javascript
// Get profile
mcp__flow-nexus__user_profile({ user_id: "your_id" })

// Update profile
mcp__flow-nexus__user_update_profile({
  user_id: "your_id",
  updates: { full_name: "New Name" }
})
```

## Quick Start
1. Register with your email
2. Check your email for verification
3. Login to access all features
4. Configure auto-refill for uninterrupted service