---
name: flow-nexus-payments
description: Credit management, billing, and payment configuration
---

# Flow Nexus Payments

Manage credits, configure billing, and track usage.

## Check Balance
```javascript
mcp__flow-nexus__check_balance()
```

## Purchase Credits
```javascript
// Create payment link
mcp__flow-nexus__create_payment_link({
  amount: 50 // USD, minimum $10
})
// Returns secure payment URL to complete purchase
```

## Auto-Refill Configuration
```javascript
// Enable auto-refill
mcp__flow-nexus__configure_auto_refill({
  enabled: true,
  threshold: 100,  // Refill when credits drop below 100
  amount: 50       // Refill with $50 worth of credits
})

// Disable auto-refill
mcp__flow-nexus__configure_auto_refill({
  enabled: false
})
```

## Payment History
```javascript
mcp__flow-nexus__get_payment_history({
  limit: 50
})
```

## rUv Credits Management
```javascript
// Check balance
mcp__flow-nexus__ruv_balance({
  user_id: "your_id"
})

// Transaction history
mcp__flow-nexus__ruv_history({
  user_id: "your_id",
  limit: 100
})
```

## Upgrade Tier
```javascript
mcp__flow-nexus__user_upgrade({
  user_id: "your_id",
  tier: "pro" // pro, enterprise
})
```

## Usage Statistics
```javascript
mcp__flow-nexus__user_stats({
  user_id: "your_id"
})
```

## Credit Pricing
- **Swarm Operations**: 1-10 credits/hour
- **Sandbox Execution**: 0.5-5 credits/hour
- **Neural Training**: 5-50 credits/job
- **Workflow Runs**: 0.1-1 credit/execution
- **Storage**: 0.01 credits/GB/day

## Earning Credits
1. **Complete Challenges**: 10-500 credits per challenge
2. **Publish Templates**: Earn when others use
3. **Referrals**: Bonus credits for invites
4. **Daily Login**: Small daily bonus
5. **Achievements**: Unlock milestone rewards

## Tiers
### Free Tier
- 100 free credits monthly
- Basic sandbox access
- Limited swarm agents (3 max)
- Community support

### Pro Tier ($29/month)
- 1000 credits monthly
- Priority sandbox access
- Unlimited agents
- Advanced workflows
- Email support

### Enterprise Tier (Custom)
- Unlimited credits
- Dedicated resources
- Custom models
- SLA guarantee
- Priority support

## Cost Optimization Tips
1. Use smaller sandboxes when possible
2. Optimize neural network training parameters
3. Batch workflow executions
4. Clean up unused resources
5. Monitor usage regularly
6. Use templates to avoid redundant work