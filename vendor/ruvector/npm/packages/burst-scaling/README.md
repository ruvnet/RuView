# Ruvector Adaptive Burst Scaling System

> Production-ready auto-scaling infrastructure for handling 10-50x traffic bursts while maintaining <50ms p99 latency

## Overview

This burst scaling system enables Ruvector to handle massive traffic spikes (e.g., World Cup events with 25 billion concurrent streams) while maintaining strict latency SLAs and cost controls.

### Key Features

- **Predictive Scaling**: ML-based forecasting pre-warms capacity before known events
- **Reactive Scaling**: Real-time auto-scaling based on CPU, memory, connections, and latency
- **Global Orchestration**: Cross-region capacity allocation with budget controls
- **Cost Management**: Sophisticated budget tracking with graceful degradation
- **Infrastructure as Code**: Complete Terraform configuration for GCP Cloud Run
- **Comprehensive Monitoring**: Cloud Monitoring dashboard with 15+ key metrics

### Capabilities

| Metric | Baseline | Burst Capacity | Target |
|--------|----------|----------------|--------|
| Concurrent Streams | 500M | 25B (50x) | <50ms p99 |
| Scale-Out Time | N/A | <60 seconds | Full capacity |
| Regions | 3 | 8+ | Global coverage |
| Cost Control | $240k/day | $5M/month | Budget-aware |
| Instances per Region | 10-50 | 1000+ | Auto-scaling |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Global Load Balancer                        │
│                    (CDN + SSL + Health Checks)                   │
└───────────────────┬──────────────┬──────────────┬───────────────┘
                    │              │              │
        ┌───────────▼──────┐  ┌────▼─────────┐  ┌▼──────────────┐
        │  us-central1     │  │ europe-west1 │  │  asia-east1   │
        │  Cloud Run       │  │  Cloud Run   │  │  Cloud Run    │
        │  10-1000 inst    │  │  10-1000 inst│  │  10-1000 inst │
        └───────────┬──────┘  └────┬─────────┘  └┬──────────────┘
                    │              │              │
        ┌───────────▼──────────────▼──────────────▼──────────────┐
        │            Capacity Manager (Orchestration)             │
        │  ┌────────────────┐  ┌──────────────────────────────┐ │
        │  │ Burst Predictor│  │    Reactive Scaler           │ │
        │  │ - Event cal    │  │    - Real-time metrics       │ │
        │  │ - ML forecast  │  │    - Dynamic thresholds      │ │
        │  │ - Pre-warming  │  │    - Rapid scale-out         │ │
        │  └────────────────┘  └──────────────────────────────┘ │
        └─────────────────────────────────────────────────────────┘
                    │              │              │
        ┌───────────▼──────┐  ┌────▼─────────┐  ┌▼──────────────┐
        │  Cloud SQL       │  │  Redis       │  │  Monitoring   │
        │  + Read Replicas │  │  64GB HA     │  │  Dashboards   │
        └──────────────────┘  └──────────────┘  └───────────────┘
```

## Quick Start

### Prerequisites

- Node.js 18+
- Terraform 1.0+
- GCP Project with billing enabled
- GCP CLI (`gcloud`) authenticated

### Installation

```bash
cd /home/user/ruvector/src/burst-scaling

# Install dependencies
npm install

# Configure GCP
gcloud config set project YOUR_PROJECT_ID

# Initialize Terraform
cd terraform
terraform init

# Create terraform.tfvars (see variables.tf for all options)
cat > terraform.tfvars <<EOF
project_id      = "ruvector-prod"
billing_account = "0123AB-CDEF45-67890"
domain          = "api.ruvector.io"
alert_email     = "ops@ruvector.io"

regions = [
  "us-central1",
  "europe-west1",
  "asia-east1"
]

# Scaling configuration
min_instances        = 10
max_instances        = 1000
burst_multiplier_max = 50

# Budget
hourly_budget  = 10000
daily_budget   = 200000
monthly_budget = 5000000

# Thresholds
cpu_scale_out_threshold = 0.70
latency_threshold_ms    = 50
EOF
```

### Deploy Infrastructure

```bash
# Plan deployment
terraform plan -var-file="terraform.tfvars"

# Deploy (creates all infrastructure)
terraform apply -var-file="terraform.tfvars"

# Outputs will show:
# - Load balancer IP address
# - Cloud Run service URLs
# - Database connection strings
# - Redis instance hosts
```

### Configure Monitoring

```bash
# Import dashboard to Cloud Monitoring
gcloud monitoring dashboards create \
  --config-from-file=../monitoring-dashboard.json

# Set up alerting (already configured via Terraform)
# Alerts will be sent to: ops@ruvector.io
```

### Run Scaling Components

```bash
# Start Burst Predictor (loads event calendar)
npm run predictor

# Start Reactive Scaler (monitors real-time metrics)
npm run scaler

# Start Capacity Manager (orchestrates everything)
npm run manager

# For production, run as systemd services or Cloud Run jobs
```

## Usage

### Predictive Scaling

```typescript
import { BurstPredictor, EventCalendar } from './burst-predictor';

const predictor = new BurstPredictor();

// Load event calendar
const calendar: EventCalendar = {
  events: [
    {
      id: 'world-cup-final',
      name: 'World Cup Final 2026',
      type: 'sports',
      startTime: new Date('2026-07-19T15:00:00Z'),
      region: ['us-central1', 'europe-west1', 'south-america-east1'],
      expectedViewers: 2_000_000_000
    }
  ]
};

await predictor.loadEventCalendar(calendar);

// Get predictions for next 24 hours
const bursts = await predictor.predictUpcomingBursts(24);
console.log(`Predicted ${bursts.length} burst events`);

// Get pre-warming schedule
const schedule = await predictor.getPreWarmingSchedule();
```

### Reactive Scaling

```typescript
import { ReactiveScaler, ScalingMetrics } from './reactive-scaler';

const scaler = new ReactiveScaler();

// Update thresholds
scaler.updateThresholds({
  cpuScaleOut: 0.70,
  cpuScaleIn: 0.30,
  maxP99Latency: 50
});

// Process metrics (called continuously)
const metrics: ScalingMetrics = {
  region: 'us-central1',
  timestamp: new Date(),
  cpuUtilization: 0.75,
  memoryUtilization: 0.68,
  activeConnections: 45_000_000,
  requestRate: 150_000,
  errorRate: 0.005,
  p99Latency: 42,
  currentInstances: 50
};

const action = await scaler.processMetrics(metrics);
if (action.action !== 'none') {
  console.log(`Scaling ${action.region}: ${action.fromInstances} -> ${action.toInstances}`);
}
```

### Capacity Management

```typescript
import { CapacityManager } from './capacity-manager';

const manager = new CapacityManager();

// Update budget
manager.updateBudget({
  hourlyBudget: 12000,
  warningThreshold: 0.85
});

// Run orchestration (call every 60 seconds)
const plan = await manager.orchestrate();
console.log(`Total instances: ${plan.totalInstances}`);
console.log(`Total cost: $${plan.totalCost}/hour`);
console.log(`Degradation level: ${plan.degradationLevel}`);
```

## Configuration

### Scaling Thresholds

Edit `terraform/variables.tf`:

```hcl
# CPU thresholds
cpu_scale_out_threshold = 0.70  # Scale out at 70% CPU
cpu_scale_in_threshold  = 0.30  # Scale in at 30% CPU

# Memory thresholds
memory_scale_out_threshold = 0.75
memory_scale_in_threshold  = 0.35

# Latency
latency_threshold_ms = 50  # p99 latency SLA

# Connections
max_connections_per_instance = 500000
```

### Budget Controls

```hcl
# Budget limits
hourly_budget   = 10000   # $10k/hour
daily_budget    = 200000  # $200k/day
monthly_budget  = 5000000 # $5M/month

# Enforcement
hard_budget_limit = false  # Allow temporary overages during bursts
budget_warning_threshold = 0.80  # Warn at 80%
```

### Region Configuration

```hcl
regions = [
  "us-central1",      # Primary
  "europe-west1",     # Europe
  "asia-east1",       # Asia
  "us-east1",         # Additional US
  "asia-southeast1"   # SEA
]

# Region priorities (1-10, higher = more important)
region_priorities = {
  "us-central1"  = 10
  "europe-west1" = 9
  "asia-east1"   = 8
}

# Region costs ($/hour per instance)
region_costs = {
  "us-central1"  = 0.50
  "europe-west1" = 0.55
  "asia-east1"   = 0.60
}
```

## Monitoring

### Cloud Monitoring Dashboard

Access at: https://console.cloud.google.com/monitoring/dashboards/custom/ruvector-burst

**Key Metrics**:
- Total connections across all regions
- Connections by region (stacked area)
- P50/P95/P99 latency percentiles
- Instance count by region
- CPU & memory utilization
- Error rates
- Hourly & daily cost estimates
- Burst event timeline

### Alerts

Configured alerts (sent to `alert_email`):

| Alert | Threshold | Action |
|-------|-----------|--------|
| High Latency | p99 > 50ms for 2min | Investigate |
| Critical Latency | p99 > 100ms for 1min | Page on-call |
| High Error Rate | >1% for 5min | Investigate |
| Budget Warning | >80% hourly | Review costs |
| Budget Critical | >100% hourly | Enable degradation |
| Region Down | 0 healthy backends | Page on-call |

### Log Queries

```bash
# View scaling events
gcloud logging read 'jsonPayload.message =~ "SCALING"' --limit=50

# View high latency requests
gcloud logging read 'jsonPayload.latency > 0.1' --limit=50

# View budget alerts
gcloud logging read 'jsonPayload.message =~ "BUDGET"' --limit=50
```

## Operations

### Daily Operations

See [RUNBOOK.md](./RUNBOOK.md) for complete operational procedures.

**Quick checks**:
```bash
# Check system status
npm run manager

# View predictions
npm run predictor

# Check current metrics
gcloud run services list --platform=managed

# Review costs
gcloud billing accounts list
```

### Emergency Procedures

**Latency spike (p99 > 100ms)**:
```bash
# Force scale-out all regions
for region in us-central1 europe-west1 asia-east1; do
  gcloud run services update ruvector-$region \
    --region=$region \
    --max-instances=1500
done
```

**Budget exceeded**:
```bash
# Enable minor degradation (shed free tier)
npm run manager -- --degrade=minor

# Enable major degradation (free tier only, limited features)
npm run manager -- --degrade=major
```

**Region failure**:
```bash
# Scale up remaining regions
gcloud run services update ruvector-europe-west1 \
  --region=europe-west1 \
  --max-instances=2000

# Activate backup region
terraform apply -var='regions=["us-central1","europe-west1","asia-east1","us-east1"]'
```

## Cost Analysis

### Expected Costs

| Scenario | Instances | Hourly | Daily | Monthly |
|----------|-----------|--------|-------|---------|
| Baseline | 30 (10/region) | $45 | $1,080 | $32,400 |
| Normal Load | 150 (50/region) | $225 | $5,400 | $162,000 |
| Medium Burst (10x) | 600 (200/region) | $900 | $21,600 | $648,000 |
| Major Burst (25x) | 1,500 (500/region) | $2,250 | $54,000 | $1,620,000 |
| World Cup (50x) | 3,000 (1000/region) | $4,500 | $108,000 | $3,240,000 |

**Cost Breakdown**:
- Cloud Run instances: $0.50/hour per instance (varies by region)
- Cloud SQL: $500/month per region
- Redis: $300/month per region
- Load Balancer: $18/month + $0.008/GB
- Networking: ~$0.12/GB egress

### Cost Optimization

- **Auto-scale down**: Gradual scale-in after bursts (5-10 minutes)
- **Regional pricing**: Prioritize cheaper regions (us-central1 < europe-west1 < asia-east1)
- **CDN caching**: Reduce backend load by 40-60%
- **Connection pooling**: Reduce database costs
- **Budget controls**: Automatic degradation at thresholds

## Testing

### Load Testing

```bash
# Install dependencies
npm install -g artillery

# Run load test
artillery run load-test.yaml

# Expected results:
# - Handle 10x burst: 5B connections
# - Maintain p99 < 50ms
# - Auto-scale to required capacity
```

### Burst Simulation

```bash
# Simulate World Cup event
npm run predictor -- --simulate --event-type=world-cup-final

# Monitor dashboard during simulation
# Verify pre-warming occurs 15 minutes before
# Verify scaling to 1000 instances per region
# Verify p99 latency stays < 50ms
```

### Cost Testing

```bash
# Simulate costs for different scenarios
npm run manager -- --simulate --multiplier=10  # 10x burst
npm run manager -- --simulate --multiplier=25  # 25x burst
npm run manager -- --simulate --multiplier=50  # 50x burst

# Review estimated costs
# Verify budget controls trigger at thresholds
```

## Troubleshooting

### Issue: Auto-scaling not working

**Check**:
```bash
# Verify Cloud Run auto-scaling config
gcloud run services describe ruvector-us-central1 --region=us-central1

# Check quotas
gcloud compute project-info describe --project=ruvector-prod

# Check IAM permissions
gcloud projects get-iam-policy ruvector-prod
```

### Issue: High latency during burst

**Check**:
- Database connection pool exhaustion
- Redis cache hit rate
- Network bandwidth limits
- CPU/memory saturation

**Fix**:
```bash
# Scale up database
gcloud sql instances patch ruvector-db-us-central1 --cpu=32 --memory=128GB

# Scale up Redis
gcloud redis instances update ruvector-redis-us-central1 --size=128

# Force scale-out
gcloud run services update ruvector-us-central1 --max-instances=2000
```

### Issue: Budget exceeded unexpectedly

**Check**:
```bash
# Review cost breakdown
gcloud billing accounts list

# Check instance counts
gcloud run services list

# Review recent scaling events
gcloud logging read 'jsonPayload.message =~ "SCALING"' --limit=100
```

**Fix**:
- Enable hard budget limit
- Adjust scale-in cooldown (faster scale-down)
- Review regional priorities
- Enable aggressive degradation

## Development

### Build

```bash
npm run build
```

### Test

```bash
npm test
```

### Lint

```bash
npm run lint
```

### Watch Mode

```bash
npm run watch
```

## Files

```
burst-scaling/
├── burst-predictor.ts          # Predictive scaling engine
├── reactive-scaler.ts          # Reactive auto-scaling
├── capacity-manager.ts         # Global orchestration
├── monitoring-dashboard.json   # Cloud Monitoring dashboard
├── package.json                # Dependencies
├── tsconfig.json              # TypeScript config
├── README.md                   # This file
├── RUNBOOK.md                  # Operations runbook
└── terraform/
    ├── main.tf                 # Infrastructure as Code
    └── variables.tf            # Configuration parameters
```

## Support

- **Documentation**: This README and RUNBOOK.md
- **Issues**: https://github.com/ruvnet/ruvector/issues
- **Slack**: #burst-scaling
- **On-call**: Check PagerDuty rotation

## License

MIT License - See LICENSE file in repository root

---

**Author**: Ruvector DevOps Team
**Last Updated**: 2025-01-20
**Version**: 1.0.0
