---
name: flow-nexus-app-store
description: Browse, publish, and deploy applications
---

# Flow Nexus App Store

Browse templates, publish apps, and deploy solutions.

## Browse Apps
```javascript
// Search apps
mcp__flow-nexus__app_search({
  search: "authentication",
  category: "backend",
  featured: true,
  limit: 20
})

// Get app details
mcp__flow-nexus__app_get({ app_id: "app_id" })

// List templates
mcp__flow-nexus__app_store_list_templates({
  category: "web-api",
  tags: ["express", "jwt"],
  limit: 20
})
```

## Publish App
```javascript
mcp__flow-nexus__app_store_publish_app({
  name: "My Auth Service",
  description: "JWT-based authentication microservice",
  category: "backend",
  version: "1.0.0",
  source_code: sourceCode,
  tags: ["auth", "jwt", "express"],
  metadata: {
    author: "Your Name",
    license: "MIT",
    repository: "github.com/user/repo"
  }
})
```

## Deploy Templates
```javascript
// Get template details
mcp__flow-nexus__template_get({
  template_name: "express-api-starter"
})

// Deploy template
mcp__flow-nexus__template_deploy({
  template_name: "express-api-starter",
  deployment_name: "my-api",
  variables: {
    api_key: "your_key",
    database_url: "postgres://..."
  },
  env_vars: {
    NODE_ENV: "production"
  }
})
```

## Analytics
```javascript
// Get app analytics
mcp__flow-nexus__app_analytics({
  app_id: "your_app_id",
  timeframe: "30d" // 24h, 7d, 30d, 90d
})

// View installed apps
mcp__flow-nexus__app_installed({
  user_id: "your_id"
})
```

## Update App
```javascript
mcp__flow-nexus__app_update({
  app_id: "app_id",
  updates: {
    version: "1.1.0",
    description: "Updated description",
    tags: ["new", "tags"]
  }
})
```

## Market Data
```javascript
// Get market statistics
mcp__flow-nexus__market_data()
```

## Template Categories
- **web-api**: RESTful APIs and microservices
- **frontend**: React, Vue, Angular apps
- **full-stack**: Complete applications
- **cli-tools**: Command-line utilities
- **data-processing**: ETL and analytics
- **ml-models**: Pre-trained models
- **blockchain**: Web3 applications
- **mobile**: React Native apps

## Publishing Best Practices
1. Include comprehensive documentation
2. Add example usage and configuration
3. Include tests and CI/CD setup
4. Use semantic versioning
5. Add clear license information
6. Include docker/deployment configs
7. Provide migration guides for updates

## Revenue Sharing
- Earn rUv credits when others use your templates
- Set pricing (0 for free templates)
- Track usage and earnings in analytics
- Withdraw credits or use for Flow Nexus services