# ADR-013: Google Cloud Platform Deployment Architecture

## Status
Accepted

## Date
2026-01-27

## Context

RuvBot needs a production-ready deployment option that:
1. Minimizes operational costs for low-traffic scenarios
2. Scales automatically with demand
3. Provides persistence for sessions, memory, and learning data
4. Secures API keys and credentials
5. Supports multi-tenant deployments

## Decision

Deploy RuvBot on Google Cloud Platform using serverless and managed services optimized for cost.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Google Cloud Platform                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                   │
│  │   Cloud      │    │   Cloud      │    │   Cloud      │                   │
│  │   Build      │───▶│   Registry   │───▶│   Run        │                   │
│  │   (CI/CD)    │    │   (Images)   │    │   (App)      │                   │
│  └──────────────┘    └──────────────┘    └──────┬───────┘                   │
│                                                  │                           │
│                     ┌────────────────────────────┼────────────────────────┐  │
│                     │                            │                        │  │
│              ┌──────▼──────┐   ┌────────────────▼───────────┐            │  │
│              │   Secret    │   │      Cloud SQL             │            │  │
│              │   Manager   │   │      (PostgreSQL)          │            │  │
│              │             │   │      db-f1-micro           │            │  │
│              └─────────────┘   └────────────────────────────┘            │  │
│                                                                          │  │
│              ┌─────────────┐   ┌────────────────────────────┐            │  │
│              │   Cloud     │   │      Memorystore           │            │  │
│              │   Storage   │   │      (Redis) - Optional    │            │  │
│              │   (Files)   │   │      Basic tier            │            │  │
│              └─────────────┘   └────────────────────────────┘            │  │
│                                                                          │  │
│                     └────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Cost Optimization Strategy

| Service | Configuration | Monthly Cost | Notes |
|---------|--------------|--------------|-------|
| Cloud Run | 0-10 instances, 512Mi RAM | ~$0-5 | Free tier: 2M requests |
| Cloud SQL | db-f1-micro, 10GB SSD | ~$10-15 | Smallest instance |
| Secret Manager | 3-5 secrets | ~$0.18 | $0.06/secret/month |
| Cloud Storage | Standard, lifecycle policies | ~$0.02/GB | Auto-tiering |
| Cloud Build | Free tier | ~$0 | 120 min/day free |
| **Total (low traffic)** | | **~$15-20/month** | |

### Service Configuration

#### Cloud Run (Compute)

```yaml
# Serverless container configuration
resources:
  cpu: "1"
  memory: "512Mi"
scaling:
  minInstances: 0      # Scale to zero when idle
  maxInstances: 10     # Limit for cost control
  concurrency: 80      # Requests per instance
features:
  cpuIdle: true        # Reduce CPU when idle (cost savings)
  startupCpuBoost: true # Faster cold starts
timeout: 300s          # 5 minutes for long operations
```

#### Cloud SQL (Database)

```hcl
# Cost-optimized PostgreSQL
tier         = "db-f1-micro"  # 0.6GB RAM, shared CPU
disk_size    = 10             # Minimum SSD
availability = "ZONAL"        # Single zone (cheaper)
backup_retention = 7          # 7 days

# Extensions enabled
- uuid-ossp     # UUID generation
- pgcrypto      # Cryptographic functions
- pg_trgm       # Text search (trigram similarity)
```

#### Secret Manager

Securely stores:
- `anthropic-api-key` - Anthropic API credentials
- `openrouter-api-key` - OpenRouter API credentials
- `database-url` - PostgreSQL connection string

#### Cloud Storage

```hcl
# Automatic cost optimization
lifecycle_rules = [
  { age = 30, action = "SetStorageClass", class = "NEARLINE" },
  { age = 90, action = "SetStorageClass", class = "COLDLINE" }
]
```

### Deployment Options

#### Option 1: Quick Deploy (gcloud CLI)

```bash
# Set environment variables
export ANTHROPIC_API_KEY="sk-ant-..."
export PROJECT_ID="my-project"

# Run deployment script
./deploy/gcp/deploy.sh --project-id $PROJECT_ID
```

#### Option 2: Infrastructure as Code (Terraform)

```bash
cd deploy/gcp/terraform

terraform init
terraform plan -var="project_id=my-project" -var="anthropic_api_key=sk-ant-..."
terraform apply
```

#### Option 3: CI/CD (Cloud Build)

```yaml
# Trigger on push to main branch
trigger:
  branch: main
  included_files:
    - "npm/packages/ruvbot/**"

# cloudbuild.yaml handles build and deploy
```

### Multi-Tenant Configuration

For multiple tenants:

```hcl
# Separate Cloud SQL databases
resource "google_sql_database" "tenant" {
  for_each = var.tenants
  name     = "ruvbot_${each.key}"
  instance = google_sql_database_instance.ruvbot.name
}

# Row-Level Security in PostgreSQL
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON sessions
  USING (tenant_id = current_setting('app.tenant_id')::uuid);
```

### Scaling Considerations

| Traffic Level | Cloud Run Instances | Cloud SQL | Estimated Cost |
|---------------|---------------------|-----------|----------------|
| Low (<1K req/day) | 0-1 | db-f1-micro | ~$15/month |
| Medium (<10K req/day) | 1-3 | db-g1-small | ~$40/month |
| High (<100K req/day) | 3-10 | db-custom | ~$150/month |
| Enterprise | 10-100 | Regional HA | ~$500+/month |

### Security Configuration

```hcl
# Service account with minimal permissions
roles = [
  "roles/secretmanager.secretAccessor",
  "roles/cloudsql.client",
  "roles/storage.objectAdmin",
  "roles/logging.logWriter",
  "roles/monitoring.metricWriter",
]

# Network security
ip_configuration {
  ipv4_enabled = false         # Production: use private IP
  private_network = google_compute_network.vpc.id
}
```

### Health Monitoring

```yaml
# Cloud Run health checks
startup_probe:
  http_get:
    path: /health
    port: 8080
  initial_delay_seconds: 5
  timeout_seconds: 3
  period_seconds: 10

liveness_probe:
  http_get:
    path: /health
    port: 8080
  timeout_seconds: 3
  period_seconds: 30
```

### File Structure

```
deploy/
├── gcp/
│   ├── cloudbuild.yaml      # CI/CD pipeline
│   ├── deploy.sh            # Quick deployment script
│   └── terraform/
│       └── main.tf          # Infrastructure as code
├── init-db.sql              # Database schema
├── Dockerfile               # Container image
└── docker-compose.yml       # Local development
```

## Consequences

### Positive
- **Cost-effective**: ~$15-20/month for low traffic
- **Serverless**: Scale to zero when not in use
- **Managed services**: No infrastructure maintenance
- **Security**: Secret Manager, IAM, VPC support
- **Observability**: Built-in logging and monitoring

### Negative
- **Cold starts**: First request after idle ~2-3 seconds
- **Vendor lock-in**: GCP-specific services
- **Complexity**: Multiple services to configure

### Trade-offs
- **Cloud SQL vs Firestore**: SQL chosen for complex queries, Row-Level Security
- **Cloud Run vs GKE**: Run chosen for simplicity, lower cost
- **db-f1-micro vs larger**: Cost vs performance trade-off

## Alternatives Considered

| Option | Pros | Cons | Estimated Cost |
|--------|------|------|----------------|
| GKE + Postgres | Full control, predictable | Complex, expensive | ~$100+/month |
| App Engine | Simple deployment | Less flexible | ~$30/month |
| Firebase + Functions | Easy scaling | No SQL, vendor lock | ~$20/month |
| **Cloud Run + SQL** | **Balanced** | **Some complexity** | **~$15/month** |

## References

- [Cloud Run Pricing](https://cloud.google.com/run/pricing)
- [Cloud SQL Pricing](https://cloud.google.com/sql/pricing)
- [Terraform GCP Provider](https://registry.terraform.io/providers/hashicorp/google/latest/docs)
- [Cloud Build CI/CD](https://cloud.google.com/build/docs)
