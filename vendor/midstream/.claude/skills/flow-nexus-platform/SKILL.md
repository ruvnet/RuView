---
name: flow-nexus-platform
description: Comprehensive Flow Nexus platform management - authentication, sandboxes, app deployment, payments, and challenges
category: platform
version: 1.0.0
author: Flow Nexus
tags: [authentication, sandboxes, deployment, payments, gamification, cloud]
---

# Flow Nexus Platform Management

Comprehensive platform management for Flow Nexus - covering authentication, sandbox execution, app deployment, credit management, and coding challenges.

## Table of Contents
1. [Authentication & User Management](#authentication--user-management)
2. [Sandbox Management](#sandbox-management)
3. [App Store & Deployment](#app-store--deployment)
4. [Payments & Credits](#payments--credits)
5. [Challenges & Achievements](#challenges--achievements)
6. [Storage & Real-time](#storage--real-time)
7. [System Utilities](#system-utilities)

---

## Authentication & User Management

### Registration & Login

**Register New Account**
```javascript
mcp__flow-nexus__user_register({
  email: "user@example.com",
  password: "secure_password",
  full_name: "Your Name",
  username: "unique_username" // optional
})
```

**Login**
```javascript
mcp__flow-nexus__user_login({
  email: "user@example.com",
  password: "your_password"
})
```

**Check Authentication Status**
```javascript
mcp__flow-nexus__auth_status({ detailed: true })
```

**Logout**
```javascript
mcp__flow-nexus__user_logout()
```

### Password Management

**Request Password Reset**
```javascript
mcp__flow-nexus__user_reset_password({
  email: "user@example.com"
})
```

**Update Password with Token**
```javascript
mcp__flow-nexus__user_update_password({
  token: "reset_token_from_email",
  new_password: "new_secure_password"
})
```

**Verify Email**
```javascript
mcp__flow-nexus__user_verify_email({
  token: "verification_token_from_email"
})
```

### Profile Management

**Get User Profile**
```javascript
mcp__flow-nexus__user_profile({
  user_id: "your_user_id"
})
```

**Update Profile**
```javascript
mcp__flow-nexus__user_update_profile({
  user_id: "your_user_id",
  updates: {
    full_name: "Updated Name",
    bio: "AI Developer and researcher",
    github_username: "yourusername",
    twitter_handle: "@yourhandle"
  }
})
```

**Get User Statistics**
```javascript
mcp__flow-nexus__user_stats({
  user_id: "your_user_id"
})
```

**Upgrade User Tier**
```javascript
mcp__flow-nexus__user_upgrade({
  user_id: "your_user_id",
  tier: "pro" // pro, enterprise
})
```

---

## Sandbox Management

### Create & Configure Sandboxes

**Create Sandbox**
```javascript
mcp__flow-nexus__sandbox_create({
  template: "node", // node, python, react, nextjs, vanilla, base, claude-code
  name: "my-sandbox",
  env_vars: {
    API_KEY: "your_api_key",
    NODE_ENV: "development",
    DATABASE_URL: "postgres://..."
  },
  install_packages: ["express", "cors", "dotenv"],
  startup_script: "npm run dev",
  timeout: 3600, // seconds
  metadata: {
    project: "my-project",
    environment: "staging"
  }
})
```

**Configure Existing Sandbox**
```javascript
mcp__flow-nexus__sandbox_configure({
  sandbox_id: "sandbox_id",
  env_vars: {
    NEW_VAR: "value"
  },
  install_packages: ["axios", "lodash"],
  run_commands: ["npm run migrate", "npm run seed"],
  anthropic_key: "sk-ant-..." // For Claude Code integration
})
```

### Execute Code

**Run Code in Sandbox**
```javascript
mcp__flow-nexus__sandbox_execute({
  sandbox_id: "sandbox_id",
  code: `
    console.log('Hello from sandbox!');
    const result = await fetch('https://api.example.com/data');
    const data = await result.json();
    return data;
  `,
  language: "javascript",
  capture_output: true,
  timeout: 60, // seconds
  working_dir: "/app",
  env_vars: {
    TEMP_VAR: "override"
  }
})
```

### Manage Sandboxes

**List Sandboxes**
```javascript
mcp__flow-nexus__sandbox_list({
  status: "running" // running, stopped, all
})
```

**Get Sandbox Status**
```javascript
mcp__flow-nexus__sandbox_status({
  sandbox_id: "sandbox_id"
})
```

**Upload File to Sandbox**
```javascript
mcp__flow-nexus__sandbox_upload({
  sandbox_id: "sandbox_id",
  file_path: "/app/config/database.json",
  content: JSON.stringify(databaseConfig, null, 2)
})
```

**Get Sandbox Logs**
```javascript
mcp__flow-nexus__sandbox_logs({
  sandbox_id: "sandbox_id",
  lines: 100 // max 1000
})
```

**Stop Sandbox**
```javascript
mcp__flow-nexus__sandbox_stop({
  sandbox_id: "sandbox_id"
})
```

**Delete Sandbox**
```javascript
mcp__flow-nexus__sandbox_delete({
  sandbox_id: "sandbox_id"
})
```

### Sandbox Templates

- **node**: Node.js environment with npm
- **python**: Python 3.x with pip
- **react**: React development setup
- **nextjs**: Next.js full-stack framework
- **vanilla**: Basic HTML/CSS/JS
- **base**: Minimal Linux environment
- **claude-code**: Claude Code integrated environment

### Common Sandbox Patterns

**API Development Sandbox**
```javascript
mcp__flow-nexus__sandbox_create({
  template: "node",
  name: "api-development",
  install_packages: [
    "express",
    "cors",
    "helmet",
    "dotenv",
    "jsonwebtoken",
    "bcrypt"
  ],
  env_vars: {
    PORT: "3000",
    NODE_ENV: "development"
  },
  startup_script: "npm run dev"
})
```

**Machine Learning Sandbox**
```javascript
mcp__flow-nexus__sandbox_create({
  template: "python",
  name: "ml-training",
  install_packages: [
    "numpy",
    "pandas",
    "scikit-learn",
    "matplotlib",
    "tensorflow"
  ],
  env_vars: {
    CUDA_VISIBLE_DEVICES: "0"
  }
})
```

**Full-Stack Development**
```javascript
mcp__flow-nexus__sandbox_create({
  template: "nextjs",
  name: "fullstack-app",
  install_packages: [
    "prisma",
    "@prisma/client",
    "next-auth",
    "zod"
  ],
  env_vars: {
    DATABASE_URL: "postgresql://...",
    NEXTAUTH_SECRET: "secret"
  }
})
```

---

## App Store & Deployment

### Browse & Search

**Search Applications**
```javascript
mcp__flow-nexus__app_search({
  search: "authentication api",
  category: "backend",
  featured: true,
  limit: 20
})
```

**Get App Details**
```javascript
mcp__flow-nexus__app_get({
  app_id: "app_id"
})
```

**List Templates**
```javascript
mcp__flow-nexus__app_store_list_templates({
  category: "web-api",
  tags: ["express", "jwt", "typescript"],
  limit: 20
})
```

**Get Template Details**
```javascript
mcp__flow-nexus__template_get({
  template_name: "express-api-starter",
  template_id: "template_id" // alternative
})
```

**List All Available Templates**
```javascript
mcp__flow-nexus__template_list({
  category: "backend",
  template_type: "starter",
  featured: true,
  limit: 50
})
```

### Publish Applications

**Publish App to Store**
```javascript
mcp__flow-nexus__app_store_publish_app({
  name: "JWT Authentication Service",
  description: "Production-ready JWT authentication microservice with refresh tokens",
  category: "backend",
  version: "1.0.0",
  source_code: sourceCodeString,
  tags: ["auth", "jwt", "express", "typescript", "security"],
  metadata: {
    author: "Your Name",
    license: "MIT",
    repository: "github.com/username/repo",
    homepage: "https://yourapp.com",
    documentation: "https://docs.yourapp.com"
  }
})
```

**Update Application**
```javascript
mcp__flow-nexus__app_update({
  app_id: "app_id",
  updates: {
    version: "1.1.0",
    description: "Added OAuth2 support",
    tags: ["auth", "jwt", "oauth2", "express"],
    source_code: updatedSourceCode
  }
})
```

### Deploy Templates

**Deploy Template**
```javascript
mcp__flow-nexus__template_deploy({
  template_name: "express-api-starter",
  deployment_name: "my-production-api",
  variables: {
    api_key: "your_api_key",
    database_url: "postgres://user:pass@host:5432/db",
    redis_url: "redis://localhost:6379"
  },
  env_vars: {
    NODE_ENV: "production",
    PORT: "8080",
    LOG_LEVEL: "info"
  }
})
```

### Analytics & Management

**Get App Analytics**
```javascript
mcp__flow-nexus__app_analytics({
  app_id: "your_app_id",
  timeframe: "30d" // 24h, 7d, 30d, 90d
})
```

**View Installed Apps**
```javascript
mcp__flow-nexus__app_installed({
  user_id: "your_user_id"
})
```

**Get Market Statistics**
```javascript
mcp__flow-nexus__market_data()
```

### App Categories

- **web-api**: RESTful APIs and microservices
- **frontend**: React, Vue, Angular applications
- **full-stack**: Complete end-to-end applications
- **cli-tools**: Command-line utilities
- **data-processing**: ETL pipelines and analytics
- **ml-models**: Pre-trained machine learning models
- **blockchain**: Web3 and blockchain applications
- **mobile**: React Native and mobile apps

### Publishing Best Practices

1. **Documentation**: Include comprehensive README with setup instructions
2. **Examples**: Provide usage examples and sample configurations
3. **Testing**: Include test suite and CI/CD configuration
4. **Versioning**: Use semantic versioning (MAJOR.MINOR.PATCH)
5. **Licensing**: Add clear license information (MIT, Apache, etc.)
6. **Deployment**: Include Docker/docker-compose configurations
7. **Migrations**: Provide upgrade guides for version updates
8. **Security**: Document security considerations and best practices

### Revenue Sharing

- Earn rUv credits when others deploy your templates
- Set pricing (0 for free, or credits for premium)
- Track usage and earnings via analytics
- Withdraw credits or use for Flow Nexus services

---

## Payments & Credits

### Balance & Credits

**Check Credit Balance**
```javascript
mcp__flow-nexus__check_balance()
```

**Check rUv Balance**
```javascript
mcp__flow-nexus__ruv_balance({
  user_id: "your_user_id"
})
```

**View Transaction History**
```javascript
mcp__flow-nexus__ruv_history({
  user_id: "your_user_id",
  limit: 100
})
```

**Get Payment History**
```javascript
mcp__flow-nexus__get_payment_history({
  limit: 50
})
```

### Purchase Credits

**Create Payment Link**
```javascript
mcp__flow-nexus__create_payment_link({
  amount: 50 // USD, minimum $10
})
// Returns secure Stripe payment URL
```

### Auto-Refill Configuration

**Enable Auto-Refill**
```javascript
mcp__flow-nexus__configure_auto_refill({
  enabled: true,
  threshold: 100,  // Refill when credits drop below 100
  amount: 50       // Purchase $50 worth of credits
})
```

**Disable Auto-Refill**
```javascript
mcp__flow-nexus__configure_auto_refill({
  enabled: false
})
```

### Credit Pricing

**Service Costs:**
- **Swarm Operations**: 1-10 credits/hour
- **Sandbox Execution**: 0.5-5 credits/hour
- **Neural Training**: 5-50 credits/job
- **Workflow Runs**: 0.1-1 credit/execution
- **Storage**: 0.01 credits/GB/day
- **API Calls**: 0.001-0.01 credits/request

### Earning Credits

**Ways to Earn:**
1. **Complete Challenges**: 10-500 credits per challenge
2. **Publish Templates**: Earn when others deploy (you set pricing)
3. **Referral Program**: Bonus credits for user invites
4. **Daily Login**: Small daily bonus (5-10 credits)
5. **Achievements**: Unlock milestone rewards (50-1000 credits)
6. **App Store Sales**: Revenue share from paid templates

**Earn Credits Programmatically**
```javascript
mcp__flow-nexus__app_store_earn_ruv({
  user_id: "your_user_id",
  amount: 100,
  reason: "Completed expert algorithm challenge",
  source: "challenge" // challenge, app_usage, referral, etc.
})
```

### Subscription Tiers

**Free Tier**
- 100 free credits monthly
- Basic sandbox access (2 concurrent)
- Limited swarm agents (3 max)
- Community support
- 1GB storage

**Pro Tier ($29/month)**
- 1000 credits monthly
- Priority sandbox access (10 concurrent)
- Unlimited swarm agents
- Advanced workflows
- Email support
- 10GB storage
- Early access to features

**Enterprise Tier (Custom Pricing)**
- Unlimited credits
- Dedicated compute resources
- Custom neural models
- 99.9% SLA guarantee
- Priority 24/7 support
- Unlimited storage
- White-label options
- On-premise deployment

### Cost Optimization Tips

1. **Use Smaller Sandboxes**: Choose appropriate templates (base vs full-stack)
2. **Optimize Neural Training**: Tune hyperparameters, reduce epochs
3. **Batch Operations**: Group workflow executions together
4. **Clean Up Resources**: Delete unused sandboxes and storage
5. **Monitor Usage**: Check `user_stats` regularly
6. **Use Free Templates**: Leverage community templates
7. **Schedule Off-Peak**: Run heavy jobs during low-cost periods

---

## Challenges & Achievements

### Browse Challenges

**List Available Challenges**
```javascript
mcp__flow-nexus__challenges_list({
  difficulty: "intermediate", // beginner, intermediate, advanced, expert
  category: "algorithms",
  status: "active", // active, completed, locked
  limit: 20
})
```

**Get Challenge Details**
```javascript
mcp__flow-nexus__challenge_get({
  challenge_id: "two-sum-problem"
})
```

### Submit Solutions

**Submit Challenge Solution**
```javascript
mcp__flow-nexus__challenge_submit({
  challenge_id: "challenge_id",
  user_id: "your_user_id",
  solution_code: `
    function twoSum(nums, target) {
      const map = new Map();
      for (let i = 0; i < nums.length; i++) {
        const complement = target - nums[i];
        if (map.has(complement)) {
          return [map.get(complement), i];
        }
        map.set(nums[i], i);
      }
      return [];
    }
  `,
  language: "javascript",
  execution_time: 45 // milliseconds (optional)
})
```

**Mark Challenge as Complete**
```javascript
mcp__flow-nexus__app_store_complete_challenge({
  challenge_id: "challenge_id",
  user_id: "your_user_id",
  submission_data: {
    passed_tests: 10,
    total_tests: 10,
    execution_time: 45,
    memory_usage: 2048 // KB
  }
})
```

### Leaderboards

**Global Leaderboard**
```javascript
mcp__flow-nexus__leaderboard_get({
  type: "global", // global, weekly, monthly, challenge
  limit: 100
})
```

**Challenge-Specific Leaderboard**
```javascript
mcp__flow-nexus__leaderboard_get({
  type: "challenge",
  challenge_id: "specific_challenge_id",
  limit: 50
})
```

### Achievements & Badges

**List User Achievements**
```javascript
mcp__flow-nexus__achievements_list({
  user_id: "your_user_id",
  category: "speed_demon" // Optional filter
})
```

### Challenge Categories

- **algorithms**: Classic algorithm problems (sorting, searching, graphs)
- **data-structures**: DS implementation (trees, heaps, tries)
- **system-design**: Architecture and scalability challenges
- **optimization**: Performance and efficiency problems
- **security**: Security-focused vulnerabilities and fixes
- **ml-basics**: Machine learning fundamentals
- **distributed-systems**: Concurrency and distributed computing
- **databases**: Query optimization and schema design

### Challenge Difficulty Rewards

- **Beginner**: 10-25 credits
- **Intermediate**: 50-100 credits
- **Advanced**: 150-300 credits
- **Expert**: 400-500 credits
- **Master**: 600-1000 credits

### Achievement Types

- **Speed Demon**: Complete challenges in record time
- **Code Golf**: Minimize code length
- **Perfect Score**: 100% test pass rate
- **Streak Master**: Complete challenges N days in a row
- **Polyglot**: Solve in multiple languages
- **Debugger**: Fix broken code challenges
- **Optimizer**: Achieve top performance benchmarks

### Tips for Success

1. **Start Simple**: Begin with beginner challenges to build confidence
2. **Review Solutions**: Study top solutions after completing
3. **Optimize**: Aim for both correctness and performance
4. **Daily Practice**: Complete daily challenges for bonus credits
5. **Community**: Engage with discussions and learn from others
6. **Track Progress**: Monitor achievements and leaderboard position
7. **Experiment**: Try multiple approaches to problems

---

## Storage & Real-time

### File Storage

**Upload File**
```javascript
mcp__flow-nexus__storage_upload({
  bucket: "my-bucket", // public, private, shared, temp
  path: "data/users.json",
  content: JSON.stringify(userData, null, 2),
  content_type: "application/json"
})
```

**List Files**
```javascript
mcp__flow-nexus__storage_list({
  bucket: "my-bucket",
  path: "data/", // prefix filter
  limit: 100
})
```

**Get Public URL**
```javascript
mcp__flow-nexus__storage_get_url({
  bucket: "my-bucket",
  path: "data/report.pdf",
  expires_in: 3600 // seconds (default: 1 hour)
})
```

**Delete File**
```javascript
mcp__flow-nexus__storage_delete({
  bucket: "my-bucket",
  path: "data/old-file.json"
})
```

### Storage Buckets

- **public**: Publicly accessible files (CDN-backed)
- **private**: User-only access with authentication
- **shared**: Team collaboration with ACL
- **temp**: Auto-deleted after 24 hours

### Real-time Subscriptions

**Subscribe to Database Changes**
```javascript
mcp__flow-nexus__realtime_subscribe({
  table: "tasks",
  event: "INSERT", // INSERT, UPDATE, DELETE, *
  filter: "status=eq.pending AND priority=eq.high"
})
```

**List Active Subscriptions**
```javascript
mcp__flow-nexus__realtime_list()
```

**Unsubscribe**
```javascript
mcp__flow-nexus__realtime_unsubscribe({
  subscription_id: "subscription_id"
})
```

### Execution Monitoring

**Subscribe to Execution Stream**
```javascript
mcp__flow-nexus__execution_stream_subscribe({
  stream_type: "claude-flow-swarm", // claude-code, claude-flow-swarm, claude-flow-hive-mind, github-integration
  deployment_id: "deployment_id",
  sandbox_id: "sandbox_id" // alternative
})
```

**Get Stream Status**
```javascript
mcp__flow-nexus__execution_stream_status({
  stream_id: "stream_id"
})
```

**List Generated Files**
```javascript
mcp__flow-nexus__execution_files_list({
  stream_id: "stream_id",
  created_by: "claude-flow", // claude-code, claude-flow, git-clone, user
  file_type: "javascript" // filter by extension
})
```

**Get File Content from Execution**
```javascript
mcp__flow-nexus__execution_file_get({
  file_id: "file_id",
  file_path: "/path/to/file.js" // alternative
})
```

---

## System Utilities

### Queen Seraphina AI Assistant

**Seek Guidance from Seraphina**
```javascript
mcp__flow-nexus__seraphina_chat({
  message: "How should I architect a distributed microservices system?",
  enable_tools: true, // Allow her to create swarms, deploy code, etc.
  conversation_history: [
    { role: "user", content: "I need help with system architecture" },
    { role: "assistant", content: "I can help you design that. What are your requirements?" }
  ]
})
```

Queen Seraphina is an advanced AI assistant with:
- Deep expertise in distributed systems
- Ability to create swarms and orchestrate agents
- Code deployment and architecture design
- Multi-turn conversation with context retention
- Tool usage for hands-on assistance

### System Health & Monitoring

**Check System Health**
```javascript
mcp__flow-nexus__system_health()
```

**View Audit Logs**
```javascript
mcp__flow-nexus__audit_log({
  user_id: "your_user_id", // optional filter
  limit: 100
})
```

### Authentication Management

**Initialize Authentication**
```javascript
mcp__flow-nexus__auth_init({
  mode: "user" // user, service
})
```

---

## Quick Start Guide

### Step 1: Register & Login

```javascript
// Register
mcp__flow-nexus__user_register({
  email: "dev@example.com",
  password: "SecurePass123!",
  full_name: "Developer Name"
})

// Login
mcp__flow-nexus__user_login({
  email: "dev@example.com",
  password: "SecurePass123!"
})

// Check auth status
mcp__flow-nexus__auth_status({ detailed: true })
```

### Step 2: Configure Billing

```javascript
// Check current balance
mcp__flow-nexus__check_balance()

// Add credits
const paymentLink = mcp__flow-nexus__create_payment_link({
  amount: 50 // $50
})

// Setup auto-refill
mcp__flow-nexus__configure_auto_refill({
  enabled: true,
  threshold: 100,
  amount: 50
})
```

### Step 3: Create Your First Sandbox

```javascript
// Create development sandbox
const sandbox = mcp__flow-nexus__sandbox_create({
  template: "node",
  name: "dev-environment",
  install_packages: ["express", "dotenv"],
  env_vars: {
    NODE_ENV: "development"
  }
})

// Execute code
mcp__flow-nexus__sandbox_execute({
  sandbox_id: sandbox.id,
  code: 'console.log("Hello Flow Nexus!")',
  language: "javascript"
})
```

### Step 4: Deploy an App

```javascript
// Browse templates
mcp__flow-nexus__template_list({
  category: "backend",
  featured: true
})

// Deploy template
mcp__flow-nexus__template_deploy({
  template_name: "express-api-starter",
  deployment_name: "my-api",
  variables: {
    database_url: "postgres://..."
  }
})
```

### Step 5: Complete a Challenge

```javascript
// Find challenges
mcp__flow-nexus__challenges_list({
  difficulty: "beginner",
  category: "algorithms"
})

// Submit solution
mcp__flow-nexus__challenge_submit({
  challenge_id: "fizzbuzz",
  user_id: "your_id",
  solution_code: "...",
  language: "javascript"
})
```

---

## Best Practices

### Security
1. Never hardcode API keys - use environment variables
2. Enable 2FA when available
3. Regularly rotate passwords and tokens
4. Use private buckets for sensitive data
5. Review audit logs periodically
6. Set appropriate file expiration times

### Performance
1. Clean up unused sandboxes to save credits
2. Use smaller sandbox templates when possible
3. Optimize storage by deleting old files
4. Batch operations to reduce API calls
5. Monitor usage via `user_stats`
6. Use temp buckets for transient data

### Development
1. Start with sandbox testing before deployment
2. Version your applications semantically
3. Document all templates thoroughly
4. Include tests in published apps
5. Use execution monitoring for debugging
6. Leverage real-time subscriptions for live updates

### Cost Management
1. Set auto-refill thresholds carefully
2. Monitor credit usage regularly
3. Complete daily challenges for bonus credits
4. Publish templates to earn passive credits
5. Use free-tier resources when appropriate
6. Schedule heavy jobs during off-peak times

---

## Troubleshooting

### Authentication Issues
- **Login Failed**: Check email/password, verify email first
- **Token Expired**: Re-login to get fresh tokens
- **Permission Denied**: Check tier limits, upgrade if needed

### Sandbox Issues
- **Sandbox Won't Start**: Check template compatibility, verify credits
- **Execution Timeout**: Increase timeout parameter or optimize code
- **Out of Memory**: Use larger template or optimize memory usage
- **Package Install Failed**: Check package name, verify npm/pip availability

### Payment Issues
- **Payment Failed**: Check payment method, sufficient funds
- **Credits Not Applied**: Allow 5-10 minutes for processing
- **Auto-refill Not Working**: Verify payment method on file

### Challenge Issues
- **Submission Rejected**: Check code syntax, ensure all tests pass
- **Wrong Answer**: Review test cases, check edge cases
- **Performance Too Slow**: Optimize algorithm complexity

---

## Support & Resources

- **Documentation**: https://docs.flow-nexus.ruv.io
- **API Reference**: https://api.flow-nexus.ruv.io/docs
- **Status Page**: https://status.flow-nexus.ruv.io
- **Community Forum**: https://community.flow-nexus.ruv.io
- **GitHub Issues**: https://github.com/ruvnet/flow-nexus/issues
- **Discord**: https://discord.gg/flow-nexus
- **Email Support**: support@flow-nexus.ruv.io (Pro/Enterprise only)

---

## Progressive Disclosure

<details>
<summary><strong>Advanced Sandbox Configuration</strong></summary>

### Custom Docker Images
```javascript
mcp__flow-nexus__sandbox_create({
  template: "base",
  name: "custom-environment",
  startup_script: `
    apt-get update
    apt-get install -y custom-package
    git clone https://github.com/user/repo
    cd repo && npm install
  `
})
```

### Multi-Stage Execution
```javascript
// Stage 1: Setup
mcp__flow-nexus__sandbox_execute({
  sandbox_id: "id",
  code: "npm install && npm run build"
})

// Stage 2: Run
mcp__flow-nexus__sandbox_execute({
  sandbox_id: "id",
  code: "npm start",
  working_dir: "/app/dist"
})
```

</details>

<details>
<summary><strong>Advanced Storage Patterns</strong></summary>

### Large File Upload (Chunked)
```javascript
const chunkSize = 5 * 1024 * 1024 // 5MB chunks
for (let i = 0; i < chunks.length; i++) {
  await mcp__flow-nexus__storage_upload({
    bucket: "private",
    path: `large-file.bin.part${i}`,
    content: chunks[i]
  })
}
```

### Storage Lifecycle
```javascript
// Upload to temp for processing
mcp__flow-nexus__storage_upload({
  bucket: "temp",
  path: "processing/data.json",
  content: data
})

// Move to permanent storage after processing
mcp__flow-nexus__storage_upload({
  bucket: "private",
  path: "archive/processed-data.json",
  content: processedData
})
```

</details>

<details>
<summary><strong>Advanced Real-time Patterns</strong></summary>

### Multi-Table Sync
```javascript
const tables = ["users", "tasks", "notifications"]
tables.forEach(table => {
  mcp__flow-nexus__realtime_subscribe({
    table,
    event: "*",
    filter: `user_id=eq.${userId}`
  })
})
```

### Event-Driven Workflows
```javascript
// Subscribe to task completion
mcp__flow-nexus__realtime_subscribe({
  table: "tasks",
  event: "UPDATE",
  filter: "status=eq.completed"
})

// Trigger notification workflow on event
// (handled by your application logic)
```

</details>

---

## Version History

- **v1.0.0** (2025-10-19): Initial comprehensive platform skill
  - Authentication & user management
  - Sandbox creation and execution
  - App store and deployment
  - Payments and credits
  - Challenges and achievements
  - Storage and real-time features
  - System utilities and Queen Seraphina integration

---

*This skill consolidates 6 Flow Nexus command modules into a single comprehensive platform management interface.*
