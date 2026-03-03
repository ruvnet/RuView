---
name: flow-nexus-sandbox
description: E2B sandbox management for isolated code execution
---

# Flow Nexus Sandboxes

Deploy and manage isolated execution environments.

## Create Sandbox
```javascript
mcp__flow-nexus__sandbox_create({
  template: "node", // node, python, react, nextjs, vanilla, base
  name: "my-sandbox",
  env_vars: {
    API_KEY: "your_api_key",
    NODE_ENV: "development"
  },
  timeout: 3600 // seconds
})
```

## Execute Code
```javascript
mcp__flow-nexus__sandbox_execute({
  sandbox_id: "sandbox_id",
  code: `
    console.log('Hello from sandbox!');
    const result = await fetch('https://api.example.com');
    return result.json();
  `,
  language: "javascript",
  capture_output: true
})
```

## Manage Sandboxes
```javascript
// List all sandboxes
mcp__flow-nexus__sandbox_list({ status: "running" })

// Get status
mcp__flow-nexus__sandbox_status({ sandbox_id: "id" })

// Upload file
mcp__flow-nexus__sandbox_upload({
  sandbox_id: "id",
  file_path: "/app/data.json",
  content: JSON.stringify(data)
})

// Stop sandbox
mcp__flow-nexus__sandbox_stop({ sandbox_id: "id" })

// Delete sandbox
mcp__flow-nexus__sandbox_delete({ sandbox_id: "id" })
```

## Templates
- **node**: Node.js environment
- **python**: Python 3.x environment  
- **react**: React development setup
- **nextjs**: Next.js full-stack
- **vanilla**: Basic HTML/CSS/JS
- **base**: Minimal Linux environment

## Common Patterns
```javascript
// API development sandbox
mcp__flow-nexus__sandbox_create({
  template: "node",
  name: "api-dev",
  install_packages: ["express", "cors", "dotenv"],
  startup_script: "npm run dev"
})

// ML sandbox
mcp__flow-nexus__sandbox_create({
  template: "python",
  name: "ml-training",
  install_packages: ["numpy", "pandas", "scikit-learn"]
})
```