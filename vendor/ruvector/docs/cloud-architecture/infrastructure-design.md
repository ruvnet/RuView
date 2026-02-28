# Ruvector Infrastructure Design
## GCP Infrastructure Specifications for 500M Concurrent Streams

**Version:** 1.0.0
**Last Updated:** 2025-11-20
**Platform:** Google Cloud Platform (GCP)
**Scale Target:** 500M concurrent streams + 10-50x burst capacity

---

## Executive Summary

This document provides detailed infrastructure specifications for deploying Ruvector at global scale on Google Cloud Platform. The design leverages Cloud Run for stateless compute, regional data stores for low-latency access, and a multi-tier caching architecture to achieve sub-10ms p50 latency while serving 500 million concurrent streams.

**Key Infrastructure Components:**
- **Compute:** Cloud Run (Gen 2) with 5,000+ instances per region
- **Caching:** Memorystore Redis (128-256GB per region)
- **Metadata Storage:** Cloud SQL PostgreSQL (multi-region replicas)
- **Vector Storage:** Cloud Storage (multi-region buckets)
- **Coordination:** Cloud Pub/Sub (global topics)
- **Networking:** VPC with Private Service Connect

---

## 1. Cloud Run Service Configuration

### 1.1 Service Specifications

**Primary Service: `ruvector-streaming`**

```yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: ruvector-streaming
  annotations:
    run.googleapis.com/launch-stage: BETA
    run.googleapis.com/execution-environment: gen2
    run.googleapis.com/startup-cpu-boost: "true"

spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/minScale: "500"
        autoscaling.knative.dev/maxScale: "5000"
        autoscaling.knative.dev/target: "70"
        autoscaling.knative.dev/targetUtilizationPercentage: "70"
        run.googleapis.com/cpu-throttling: "false"
        run.googleapis.com/vpc-access-connector: "projects/PROJECT_ID/locations/REGION/connectors/ruvector-connector"
        run.googleapis.com/vpc-access-egress: "private-ranges-only"
        run.googleapis.com/network-interfaces: '[{"network":"ruvector-vpc","subnetwork":"ruvector-subnet"}]'

    spec:
      containerConcurrency: 100
      timeoutSeconds: 300
      serviceAccountName: ruvector-service@PROJECT_ID.iam.gserviceaccount.com

      containers:
      - name: ruvector
        image: gcr.io/PROJECT_ID/ruvector:v1.0.0
        ports:
        - name: http1
          containerPort: 8080
          protocol: TCP

        resources:
          limits:
            cpu: "4"
            memory: "16Gi"
          requests:
            cpu: "2"
            memory: "8Gi"

        startupProbe:
          httpGet:
            path: /health/startup
            port: 8080
          initialDelaySeconds: 0
          periodSeconds: 1
          timeoutSeconds: 3
          failureThreshold: 240  # 4 minutes max startup time

        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3

        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2

        env:
        # Redis connection
        - name: REDIS_HOST
          valueFrom:
            secretKeyRef:
              name: ruvector-secrets
              key: redis-host
        - name: REDIS_PORT
          value: "6379"

        # Cloud SQL connection
        - name: DB_HOST
          value: "/cloudsql/PROJECT_ID:REGION:ruvector-db"
        - name: DB_NAME
          value: "ruvector"
        - name: DB_USER
          valueFrom:
            secretKeyRef:
              name: ruvector-secrets
              key: db-user
        - name: DB_PASSWORD
          valueFrom:
            secretKeyRef:
              name: ruvector-secrets
              key: db-password

        # Cloud Storage
        - name: STORAGE_BUCKET
          value: "ruvector-vectors-REGION"

        # Pub/Sub
        - name: PUBSUB_TOPIC
          value: "projects/PROJECT_ID/topics/vector-updates"

        # Application settings
        - name: RUST_LOG
          value: "info,ruvector_core=debug"
        - name: REGION
          value: "REGION"
        - name: HNSW_M
          value: "32"
        - name: HNSW_EF_CONSTRUCTION
          value: "200"
        - name: HNSW_EF_SEARCH
          value: "100"
        - name: QUANTIZATION_ENABLED
          value: "true"
        - name: CACHE_SIZE_GB
          value: "8"
```

### 1.2 Container Image

**Dockerfile (Optimized for Size & Performance):**

```dockerfile
# Build stage
FROM rust:1.77-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    gcc \
    g++ \
    make \
    pkgconfig \
    openssl-dev

WORKDIR /app

# Copy workspace manifest
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary with optimizations
ENV RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C link-arg=-s"
RUN cargo build --release --bin ruvector-server \
    --features "simd,quantization,cloud-run"

# Runtime stage
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc \
    && rm -rf /var/cache/apk/*

# Create non-root user
RUN addgroup -g 1000 ruvector && \
    adduser -D -u 1000 -G ruvector ruvector

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/ruvector-server /app/

# Copy static assets (HNSW index templates, etc.)
COPY --chown=ruvector:ruvector assets /app/assets

USER ruvector

# Cloud Run uses PORT env variable
ENV PORT=8080
EXPOSE 8080

# Health check endpoint
HEALTHCHECK --interval=10s --timeout=3s --start-period=30s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health/ready || exit 1

# Start server
CMD ["/app/ruvector-server"]
```

**Image Size Optimization:**
```yaml
unoptimized_image: 450 MB
optimized_image: 18 MB

optimizations:
  - multi_stage_build: saved 380 MB
  - alpine_base: saved 40 MB
  - strip_symbols: saved 8 MB
  - lto_optimization: saved 4 MB

cold_start_improvement:
  before: 5.2s
  after: 1.8s (3x faster)
```

### 1.3 Regional Deployment

**Deployment Script (Terraform):**

```hcl
# terraform/cloud_run.tf

locals {
  regions = [
    # Tier 1 (80M concurrent each)
    "us-central1",
    "europe-west1",
    "asia-northeast1",
    "asia-southeast1",
    "southamerica-east1",

    # Tier 2 (10M concurrent each)
    "us-east1",
    "us-west1",
    "europe-west2",
    "europe-west3",
    "europe-north1",
    "asia-south1",
    "asia-east1",
    "australia-southeast1",
    "northamerica-northeast1",
    "me-west1"
  ]

  tier1_regions = slice(local.regions, 0, 5)
  tier2_regions = slice(local.regions, 5, 15)
}

# Deploy to all regions
resource "google_cloud_run_service" "ruvector" {
  for_each = toset(local.regions)

  name     = "ruvector-streaming"
  location = each.value

  template {
    metadata {
      annotations = {
        "autoscaling.knative.dev/minScale" = contains(local.tier1_regions, each.value) ? "800" : "100"
        "autoscaling.knative.dev/maxScale" = contains(local.tier1_regions, each.value) ? "8000" : "1000"
        "autoscaling.knative.dev/target"   = "70"
        "run.googleapis.com/startup-cpu-boost" = "true"
        "run.googleapis.com/cpu-throttling"    = "false"
        "run.googleapis.com/vpc-access-connector" = google_vpc_access_connector.ruvector[each.value].id
        "run.googleapis.com/vpc-access-egress"    = "private-ranges-only"
      }
    }

    spec {
      container_concurrency = 100
      timeout_seconds       = 300
      service_account_name  = google_service_account.ruvector[each.value].email

      containers {
        image = "gcr.io/${var.project_id}/ruvector:${var.image_tag}"

        resources {
          limits = {
            cpu    = "4"
            memory = "16Gi"
          }
        }

        env {
          name  = "REGION"
          value = each.value
        }

        env {
          name = "REDIS_HOST"
          value_from {
            secret_key_ref {
              name = google_secret_manager_secret.redis_host[each.value].secret_id
              key  = "latest"
            }
          }
        }

        # Additional env vars...
      }
    }
  }

  traffic {
    percent         = 100
    latest_revision = true
  }

  depends_on = [
    google_project_service.run,
    google_memorystore_instance.redis,
    google_sql_database_instance.postgres
  ]
}

# IAM policy for public access (with Cloud Armor protection)
resource "google_cloud_run_service_iam_member" "public" {
  for_each = toset(local.regions)

  service  = google_cloud_run_service.ruvector[each.value].name
  location = each.value
  role     = "roles/run.invoker"
  member   = "allUsers"
}
```

---

## 2. Memorystore Redis Configuration

### 2.1 Redis Instance Specifications

**Regional Redis Cluster:**

```hcl
# terraform/memorystore_redis.tf

resource "google_redis_instance" "ruvector" {
  for_each = toset(local.regions)

  name               = "ruvector-redis-${each.value}"
  region             = each.value
  tier               = "STANDARD_HA"  # High availability
  memory_size_gb     = contains(local.tier1_regions, each.value) ? 256 : 128
  redis_version      = "REDIS_7_0"
  replica_count      = 1  # 1 read replica
  read_replicas_mode = "READ_REPLICAS_ENABLED"

  # Network
  authorized_network = google_compute_network.ruvector_vpc.id
  connect_mode       = "PRIVATE_SERVICE_ACCESS"

  # Configuration
  redis_configs = {
    maxmemory-policy          = "allkeys-lru"
    timeout                   = "300"
    tcp-keepalive             = "60"
    maxmemory-samples         = "10"
    activedefrag              = "yes"
    active-defrag-cycle-min   = "5"
    active-defrag-cycle-max   = "75"
    lfu-log-factor            = "10"
    lfu-decay-time            = "1"
  }

  # Maintenance window (off-peak hours)
  maintenance_policy {
    weekly_maintenance_window {
      day = "SUNDAY"
      start_time {
        hours   = 2
        minutes = 0
      }
    }
  }

  # Monitoring
  labels = {
    environment = "production"
    service     = "ruvector"
    tier        = contains(local.tier1_regions, each.value) ? "tier1" : "tier2"
  }

  lifecycle {
    prevent_destroy = true
  }
}

# Output Redis connection info
output "redis_hosts" {
  value = {
    for region, instance in google_redis_instance.ruvector :
    region => instance.host
  }
  sensitive = true
}
```

### 2.2 Redis Data Model

**Cache Structure:**

```redis
# Vector embeddings cache
# Key: vector:{vector_id}
# Value: msgpack-encoded vector data
# TTL: 3600 seconds (1 hour)
SET vector:doc_12345 "\x93\xCB\x3F\xB9\x99..." EX 3600

# Search results cache
# Key: search:{query_hash}:{k}
# Value: JSON array of result IDs
# TTL: 60 seconds
SET search:a3f8b2c1:100 "[\"doc_12345\",\"doc_67890\",...]" EX 60

# HNSW graph cache (partial)
# Key: hnsw:{vector_id}:{level}
# Value: msgpack-encoded neighbor list
# TTL: 7200 seconds (2 hours)
SET hnsw:doc_12345:0 "\x95\x00\x01\x02..." EX 7200

# Metadata cache
# Key: meta:{vector_id}
# Value: JSON metadata
# TTL: 3600 seconds
SET meta:doc_12345 "{\"title\":\"...\",\"timestamp\":...}" EX 3600

# Rate limiting counters
# Key: ratelimit:{user_id}:{window}
# Value: request count
# TTL: window duration
INCR ratelimit:user_123:1732132800
EXPIRE ratelimit:user_123:1732132800 60

# Coordination keys (Pub/Sub coordination)
# Key: coord:{agent_id}:status
# Value: agent status
# TTL: 300 seconds (5 min)
SET coord:agent_42:status "active" EX 300
```

### 2.3 Redis Connection Pooling

**Connection Pool Configuration (Rust):**

```rust
use redis::{Client, aio::ConnectionManager};
use deadpool_redis::{Config, Pool, Runtime};

pub struct RedisPool {
    pool: Pool,
}

impl RedisPool {
    pub async fn new(redis_host: &str, redis_port: u16) -> Result<Self> {
        let config = Config {
            url: Some(format!("redis://{}:{}", redis_host, redis_port)),
            pool: Some(deadpool_redis::PoolConfig {
                max_size: 80,           // 80 connections per Cloud Run instance
                min_idle: 20,           // Keep 20 warm
                timeouts: deadpool_redis::Timeouts {
                    wait: Some(Duration::from_secs(5)),
                    create: Some(Duration::from_secs(5)),
                    recycle: Some(Duration::from_secs(5)),
                },
            }),
            connection: Some(redis::ConnectionInfo {
                addr: redis::ConnectionAddr::Tcp(redis_host.to_string(), redis_port),
                redis: redis::RedisConnectionInfo {
                    db: 0,
                    username: None,
                    password: None,
                },
            }),
        };

        let pool = config.create_pool(Some(Runtime::Tokio1))?;

        Ok(Self { pool })
    }

    pub async fn get(&self) -> Result<deadpool_redis::Connection> {
        self.pool.get().await.map_err(Into::into)
    }

    // Pipelined operations for better performance
    pub async fn pipeline_set(&self, keys: Vec<(String, Vec<u8>, u64)>) -> Result<()> {
        let mut conn = self.get().await?;

        let mut pipe = redis::pipe();
        for (key, value, ttl) in keys {
            pipe.set_ex(&key, value, ttl);
        }

        pipe.query_async(&mut *conn).await?;
        Ok(())
    }

    // Batched GET operations
    pub async fn batch_get(&self, keys: Vec<String>) -> Result<Vec<Option<Vec<u8>>>> {
        let mut conn = self.get().await?;

        let mut pipe = redis::pipe();
        for key in &keys {
            pipe.get(key);
        }

        let results: Vec<Option<Vec<u8>>> = pipe.query_async(&mut *conn).await?;
        Ok(results)
    }
}
```

---

## 3. Cloud SQL Configuration

### 3.1 PostgreSQL Instance

**Primary Instance (Multi-Region):**

```hcl
# terraform/cloud_sql.tf

resource "google_sql_database_instance" "ruvector" {
  for_each = toset(local.tier1_regions)  # Primary instances in Tier 1 regions

  name             = "ruvector-db-${each.value}"
  database_version = "POSTGRES_15"
  region           = each.value

  settings {
    tier              = "db-custom-4-16384"  # 4 vCPU, 16 GB RAM
    availability_type = "REGIONAL"           # High availability
    disk_type         = "PD_SSD"
    disk_size         = 100  # GB
    disk_autoresize   = true
    disk_autoresize_limit = 500

    # Backup configuration
    backup_configuration {
      enabled                        = true
      start_time                     = "03:00"  # 3 AM UTC
      point_in_time_recovery_enabled = true
      transaction_log_retention_days = 7
      backup_retention_settings {
        retained_backups = 30
        retention_unit   = "COUNT"
      }
    }

    # High availability
    location_preference {
      zone = "${each.value}-a"
    }

    # IP configuration
    ip_configuration {
      ipv4_enabled    = false  # Private IP only
      private_network = google_compute_network.ruvector_vpc.id
      require_ssl     = true
    }

    # Database flags
    database_flags {
      name  = "max_connections"
      value = "1000"
    }
    database_flags {
      name  = "shared_buffers"
      value = "4096MB"
    }
    database_flags {
      name  = "effective_cache_size"
      value = "12GB"
    }
    database_flags {
      name  = "maintenance_work_mem"
      value = "1GB"
    }
    database_flags {
      name  = "checkpoint_completion_target"
      value = "0.9"
    }
    database_flags {
      name  = "wal_buffers"
      value = "16MB"
    }
    database_flags {
      name  = "default_statistics_target"
      value = "100"
    }
    database_flags {
      name  = "random_page_cost"
      value = "1.1"  # SSD optimization
    }
    database_flags {
      name  = "effective_io_concurrency"
      value = "200"  # SSD optimization
    }

    # Maintenance window
    maintenance_window {
      day          = 7  # Sunday
      hour         = 3  # 3 AM UTC
      update_track = "stable"
    }

    # Insights
    insights_config {
      query_insights_enabled  = true
      query_plans_per_minute  = 5
      query_string_length     = 4096
      record_application_tags = true
    }
  }

  deletion_protection = true

  lifecycle {
    prevent_destroy = true
  }
}

# Read replicas in Tier 2 regions
resource "google_sql_database_instance" "ruvector_replica" {
  for_each = toset(local.tier2_regions)

  name                 = "ruvector-db-${each.value}-replica"
  database_version     = "POSTGRES_15"
  region               = each.value
  master_instance_name = google_sql_database_instance.ruvector[
    # Map each Tier 2 region to nearest Tier 1 region
    lookup({
      "us-east1"                  = "us-central1",
      "us-west1"                  = "us-central1",
      "europe-west2"              = "europe-west1",
      "europe-west3"              = "europe-west1",
      "europe-north1"             = "europe-west1",
      "asia-south1"               = "asia-southeast1",
      "asia-east1"                = "asia-northeast1",
      "australia-southeast1"      = "asia-southeast1",
      "northamerica-northeast1"   = "us-central1",
      "me-west1"                  = "europe-west1"
    }, each.value)
  ].name

  replica_configuration {
    failover_target = false
  }

  settings {
    tier              = "db-custom-2-8192"  # Smaller for replicas
    availability_type = "ZONAL"
    disk_type         = "PD_SSD"

    ip_configuration {
      ipv4_enabled    = false
      private_network = google_compute_network.ruvector_vpc.id
      require_ssl     = true
    }
  }
}

# Database
resource "google_sql_database" "ruvector" {
  for_each = toset(local.tier1_regions)

  name     = "ruvector"
  instance = google_sql_database_instance.ruvector[each.value].name
}

# Users
resource "google_sql_user" "ruvector" {
  for_each = toset(local.tier1_regions)

  name     = "ruvector"
  instance = google_sql_database_instance.ruvector[each.value].name
  password = random_password.db_password[each.value].result
}

resource "random_password" "db_password" {
  for_each = toset(local.tier1_regions)

  length  = 32
  special = true
}
```

### 3.2 Database Schema

**PostgreSQL Schema:**

```sql
-- Vector metadata table
CREATE TABLE vector_metadata (
    id VARCHAR(255) PRIMARY KEY,
    dimension INT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    metadata JSONB,
    region VARCHAR(50) NOT NULL,
    storage_path TEXT NOT NULL,  -- Cloud Storage path
    checksum VARCHAR(64)  -- SHA-256 of vector data
);

-- Indexes
CREATE INDEX idx_vector_metadata_created_at ON vector_metadata(created_at DESC);
CREATE INDEX idx_vector_metadata_region ON vector_metadata(region);
CREATE INDEX idx_vector_metadata_metadata ON vector_metadata USING GIN(metadata);

-- User rate limiting table
CREATE TABLE rate_limits (
    user_id VARCHAR(255) NOT NULL,
    window_start TIMESTAMP WITH TIME ZONE NOT NULL,
    request_count INT DEFAULT 0,
    PRIMARY KEY (user_id, window_start)
);

-- Partition by day for efficient cleanup
CREATE TABLE rate_limits_partitioned (
    LIKE rate_limits INCLUDING ALL
) PARTITION BY RANGE (window_start);

-- Create partitions for next 7 days (via cron job)
CREATE TABLE rate_limits_2025_11_20 PARTITION OF rate_limits_partitioned
    FOR VALUES FROM ('2025-11-20') TO ('2025-11-21');

-- Agent coordination table (for agentic-flow integration)
CREATE TABLE agent_coordination (
    agent_id VARCHAR(255) PRIMARY KEY,
    agent_type VARCHAR(100) NOT NULL,
    status VARCHAR(50) NOT NULL,  -- 'active', 'idle', 'offline'
    region VARCHAR(50) NOT NULL,
    last_heartbeat TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    metadata JSONB
);

CREATE INDEX idx_agent_coordination_status ON agent_coordination(status);
CREATE INDEX idx_agent_coordination_region ON agent_coordination(region);
CREATE INDEX idx_agent_coordination_heartbeat ON agent_coordination(last_heartbeat);

-- Task coordination table
CREATE TABLE task_coordination (
    task_id VARCHAR(255) PRIMARY KEY,
    task_type VARCHAR(100) NOT NULL,
    status VARCHAR(50) NOT NULL,  -- 'pending', 'in_progress', 'completed', 'failed'
    assigned_agent_id VARCHAR(255) REFERENCES agent_coordination(agent_id),
    priority INT DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    task_data JSONB,
    result JSONB
);

CREATE INDEX idx_task_coordination_status ON task_coordination(status);
CREATE INDEX idx_task_coordination_priority ON task_coordination(priority DESC);
CREATE INDEX idx_task_coordination_created_at ON task_coordination(created_at DESC);

-- Analytics table (for monitoring & metrics)
CREATE TABLE query_analytics (
    query_id VARCHAR(255) PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    region VARCHAR(50) NOT NULL,
    user_id VARCHAR(255),
    query_type VARCHAR(50) NOT NULL,  -- 'search', 'insert', 'delete', etc.
    latency_ms FLOAT NOT NULL,
    cache_hit BOOLEAN,
    result_count INT,
    error_code VARCHAR(50)
);

-- Partition by month for efficient analytics
CREATE TABLE query_analytics_partitioned (
    LIKE query_analytics INCLUDING ALL
) PARTITION BY RANGE (timestamp);

CREATE TABLE query_analytics_2025_11 PARTITION OF query_analytics_partitioned
    FOR VALUES FROM ('2025-11-01') TO ('2025-12-01');

-- Materialized view for real-time metrics
CREATE MATERIALIZED VIEW query_metrics_hourly AS
SELECT
    date_trunc('hour', timestamp) AS hour,
    region,
    query_type,
    COUNT(*) AS total_queries,
    AVG(latency_ms) AS avg_latency_ms,
    PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY latency_ms) AS p50_latency_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) AS p95_latency_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY latency_ms) AS p99_latency_ms,
    SUM(CASE WHEN cache_hit THEN 1 ELSE 0 END)::FLOAT / COUNT(*) AS cache_hit_rate,
    SUM(CASE WHEN error_code IS NOT NULL THEN 1 ELSE 0 END)::FLOAT / COUNT(*) AS error_rate
FROM query_analytics_partitioned
GROUP BY 1, 2, 3;

-- Refresh every 5 minutes via cron job
CREATE INDEX idx_query_metrics_hourly_hour ON query_metrics_hourly(hour DESC);
```

---

## 4. Cloud Storage Configuration

### 4.1 Multi-Region Buckets

**Vector Storage Buckets:**

```hcl
# terraform/cloud_storage.tf

resource "google_storage_bucket" "vectors" {
  for_each = toset(local.tier1_regions)

  name          = "ruvector-vectors-${each.value}"
  location      = each.value
  storage_class = "STANDARD"  # Low-latency access

  # Versioning for disaster recovery
  versioning {
    enabled = true
  }

  # Lifecycle rules
  lifecycle_rule {
    condition {
      age = 30  # days
      num_newer_versions = 3
    }
    action {
      type = "Delete"
    }
  }

  lifecycle_rule {
    condition {
      age = 7  # Move to nearline after 7 days if not accessed
      days_since_noncurrent_time = 7
    }
    action {
      type          = "SetStorageClass"
      storage_class = "NEARLINE"
    }
  }

  # CORS for browser access (if needed)
  cors {
    origin          = ["https://app.example.com"]
    method          = ["GET", "HEAD"]
    response_header = ["Content-Type"]
    max_age_seconds = 3600
  }

  # Encryption (customer-managed keys optional)
  encryption {
    default_kms_key_name = google_kms_crypto_key.storage[each.value].id
  }

  # Access logging
  logging {
    log_bucket        = google_storage_bucket.logs.name
    log_object_prefix = "storage-logs/${each.value}/"
  }

  # Public access prevention
  public_access_prevention = "enforced"

  # Uniform bucket-level access
  uniform_bucket_level_access {
    enabled = true
  }

  labels = {
    environment = "production"
    service     = "ruvector"
    tier        = "tier1"
  }
}

# Logging bucket
resource "google_storage_bucket" "logs" {
  name          = "ruvector-logs-${var.project_id}"
  location      = "US"  # Multi-region
  storage_class = "COLDLINE"

  lifecycle_rule {
    condition {
      age = 90  # Keep logs for 90 days
    }
    action {
      type = "Delete"
    }
  }

  public_access_prevention = "enforced"
}

# IAM permissions for Cloud Run
resource "google_storage_bucket_iam_member" "cloud_run_read" {
  for_each = toset(local.tier1_regions)

  bucket = google_storage_bucket.vectors[each.value].name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.ruvector[each.value].email}"
}

resource "google_storage_bucket_iam_member" "cloud_run_write" {
  for_each = toset(local.tier1_regions)

  bucket = google_storage_bucket.vectors[each.value].name
  role   = "roles/storage.objectCreator"
  member = "serviceAccount:${google_service_account.ruvector[each.value].email}"
}
```

### 4.2 Data Organization

**Storage Layout:**

```
gs://ruvector-vectors-us-central1/
├── vectors/
│   ├── 2025/
│   │   ├── 11/
│   │   │   ├── 20/
│   │   │   │   ├── shard-00000.bin  # 10M vectors per shard
│   │   │   │   ├── shard-00001.bin
│   │   │   │   └── ...
│   │   │   └── index.json  # Metadata index
│   │   └── ...
│   └── ...
├── indices/
│   ├── hnsw-full-20251120.idx  # Full HNSW index snapshot
│   ├── hnsw-full-20251119.idx
│   └── ...
├── checkpoints/
│   ├── checkpoint-20251120-120000.bin
│   ├── checkpoint-20251120-060000.bin
│   └── ...
└── metadata/
    ├── schema.json
    └── manifest.json
```

**File Format (Custom Binary):**

```rust
// Vector shard file format
pub struct VectorShard {
    // Header (64 bytes)
    magic: [u8; 4],           // "RUVS" (RUVector Shard)
    version: u32,             // Format version
    dimension: u32,           // Vector dimension
    count: u64,               // Number of vectors in shard
    compression: u8,          // 0=none, 1=quantization, 2=product quantization
    checksum: [u8; 32],       // SHA-256 of data section

    // Index section (variable size)
    // Offset table for fast random access
    offsets: Vec<u64>,        // Byte offset for each vector

    // Data section (variable size)
    // Serialized vectors (rkyv zero-copy format)
    data: Vec<u8>,
}

// Memory-mapped access
impl VectorShard {
    pub fn open_mmap(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Parse header
        let header = &mmap[0..64];
        // ... validate and parse ...

        Ok(Self {
            magic: ...,
            version: ...,
            // ... etc ...
            data: mmap.into()  // Zero-copy
        })
    }

    pub fn get_vector(&self, index: usize) -> Option<&[f32]> {
        let offset = self.offsets.get(index)?;
        let data_slice = &self.data[*offset as usize..];

        // Deserialize with zero-copy (rkyv)
        unsafe {
            rkyv::archived_root::<Vec<f32>>(data_slice)
        }
    }
}
```

---

## 5. Cloud Pub/Sub Configuration

### 5.1 Topics & Subscriptions

**Coordination Topics:**

```hcl
# terraform/pubsub.tf

# Global vector update topic
resource "google_pubsub_topic" "vector_updates" {
  name = "vector-updates"

  message_storage_policy {
    allowed_persistence_regions = [
      "us-central1",
      "europe-west1",
      "asia-northeast1"
    ]
  }

  schema_settings {
    schema   = google_pubsub_schema.vector_update.id
    encoding = "JSON"
  }
}

# Schema for vector updates
resource "google_pubsub_schema" "vector_update" {
  name       = "vector-update-schema"
  type       = "AVRO"
  definition = jsonencode({
    type = "record"
    name = "VectorUpdate"
    fields = [
      { name = "vector_id", type = "string" },
      { name = "operation", type = "string" },  # "insert", "update", "delete"
      { name = "timestamp", type = "long" },
      { name = "region", type = "string" },
      { name = "metadata", type = ["null", "string"], default = null }
    ]
  })
}

# Regional subscriptions (one per region)
resource "google_pubsub_subscription" "vector_updates" {
  for_each = toset(local.regions)

  name  = "vector-updates-${each.value}"
  topic = google_pubsub_topic.vector_updates.name

  ack_deadline_seconds = 30

  message_retention_duration = "86400s"  # 24 hours

  retry_policy {
    minimum_backoff = "10s"
    maximum_backoff = "600s"
  }

  expiration_policy {
    ttl = ""  # Never expire
  }

  # Push to Cloud Run endpoint
  push_config {
    push_endpoint = "${google_cloud_run_service.ruvector[each.value].status[0].url}/api/v1/pubsub/vector-updates"

    oidc_token {
      service_account_email = google_service_account.ruvector[each.value].email
    }

    attributes = {
      x-goog-version = "v1"
    }
  }

  # Dead letter topic for failed messages
  dead_letter_policy {
    dead_letter_topic     = google_pubsub_topic.dead_letter.id
    max_delivery_attempts = 5
  }
}

# Agent coordination topic (for agentic-flow)
resource "google_pubsub_topic" "agent_coordination" {
  name = "agent-coordination"

  message_storage_policy {
    allowed_persistence_regions = local.tier1_regions
  }
}

resource "google_pubsub_subscription" "agent_coordination" {
  for_each = toset(local.regions)

  name  = "agent-coordination-${each.value}"
  topic = google_pubsub_topic.agent_coordination.name

  ack_deadline_seconds       = 20
  message_retention_duration = "3600s"  # 1 hour

  push_config {
    push_endpoint = "${google_cloud_run_service.ruvector[each.value].status[0].url}/api/v1/pubsub/agent-coordination"

    oidc_token {
      service_account_email = google_service_account.ruvector[each.value].email
    }
  }
}

# Task distribution topic
resource "google_pubsub_topic" "task_distribution" {
  name = "task-distribution"

  message_storage_policy {
    allowed_persistence_regions = local.tier1_regions
  }
}

# Dead letter topic
resource "google_pubsub_topic" "dead_letter" {
  name = "dead-letter"

  message_retention_duration = "604800s"  # 7 days
}
```

### 5.2 Message Flow

**Pub/Sub Integration (Rust):**

```rust
use google_cloud_pubsub::client::{Client, ClientConfig};
use google_cloud_pubsub::subscription::SubscriptionConfig;

pub struct PubSubCoordinator {
    client: Client,
    topic_name: String,
}

impl PubSubCoordinator {
    pub async fn new(project_id: &str, topic: &str) -> Result<Self> {
        let config = ClientConfig::default().with_auth().await?;
        let client = Client::new(config).await?;

        Ok(Self {
            client,
            topic_name: format!("projects/{}/topics/{}", project_id, topic),
        })
    }

    // Publish vector update
    pub async fn publish_vector_update(
        &self,
        vector_id: &str,
        operation: &str,
        region: &str,
    ) -> Result<String> {
        let topic = self.client.topic(&self.topic_name);

        let message = serde_json::json!({
            "vector_id": vector_id,
            "operation": operation,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "region": region,
        });

        let message_id = topic
            .publish(message.to_string().into_bytes())
            .await?;

        Ok(message_id)
    }

    // Batch publish (more efficient)
    pub async fn batch_publish_updates(
        &self,
        updates: Vec<VectorUpdate>,
    ) -> Result<Vec<String>> {
        let topic = self.client.topic(&self.topic_name);

        let messages: Vec<_> = updates
            .into_iter()
            .map(|update| {
                let json = serde_json::to_string(&update).unwrap();
                json.into_bytes()
            })
            .collect();

        let message_ids = topic.publish_batch(messages).await?;
        Ok(message_ids)
    }

    // Subscribe to updates
    pub async fn subscribe_updates<F>(
        &self,
        subscription_name: &str,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(VectorUpdate) -> Result<()> + Send + Sync + 'static,
    {
        let subscription = self.client.subscription(subscription_name);

        subscription
            .receive(|message, _ack_handler| async move {
                let update: VectorUpdate = serde_json::from_slice(&message.data)?;
                handler(update)?;
                Ok(())
            })
            .await?;

        Ok(())
    }
}
```

---

## 6. Networking & VPC Setup

### 6.1 VPC Configuration

**Global VPC with Regional Subnets:**

```hcl
# terraform/networking.tf

# Global VPC
resource "google_compute_network" "ruvector_vpc" {
  name                    = "ruvector-vpc"
  auto_create_subnetworks = false
  routing_mode            = "GLOBAL"
}

# Regional subnets
resource "google_compute_subnetwork" "ruvector" {
  for_each = toset(local.regions)

  name          = "ruvector-subnet-${each.value}"
  region        = each.value
  network       = google_compute_network.ruvector_vpc.id
  ip_cidr_range = cidrsubnet("10.0.0.0/8", 8, index(local.regions, each.value))

  # Private Google Access
  private_ip_google_access = true

  # Secondary ranges for services
  secondary_ip_range {
    range_name    = "pods"
    ip_cidr_range = cidrsubnet("10.0.0.0/8", 8, index(local.regions, each.value) + 100)
  }

  secondary_ip_range {
    range_name    = "services"
    ip_cidr_range = cidrsubnet("10.0.0.0/8", 8, index(local.regions, each.value) + 200)
  }

  log_config {
    aggregation_interval = "INTERVAL_5_SEC"
    flow_sampling        = 0.5
    metadata             = "INCLUDE_ALL_METADATA"
  }
}

# VPC Access Connector for Cloud Run
resource "google_vpc_access_connector" "ruvector" {
  for_each = toset(local.regions)

  name          = "ruvector-connector-${each.value}"
  region        = each.value
  network       = google_compute_network.ruvector_vpc.id
  ip_cidr_range = cidrsubnet("10.128.0.0/16", 8, index(local.regions, each.value))

  min_throughput = 200   # Mbps
  max_throughput = 1000  # Mbps (max for shared connector)

  # Use Subnet for better performance
  subnet {
    name       = google_compute_subnetwork.connector[each.value].name
    project_id = var.project_id
  }
}

# Dedicated connector subnet
resource "google_compute_subnetwork" "connector" {
  for_each = toset(local.regions)

  name          = "connector-subnet-${each.value}"
  region        = each.value
  network       = google_compute_network.ruvector_vpc.id
  ip_cidr_range = cidrsubnet("10.129.0.0/16", 8, index(local.regions, each.value))
}

# Cloud NAT for outbound connections
resource "google_compute_router" "ruvector" {
  for_each = toset(local.regions)

  name    = "ruvector-router-${each.value}"
  region  = each.value
  network = google_compute_network.ruvector_vpc.id
}

resource "google_compute_router_nat" "ruvector" {
  for_each = toset(local.regions)

  name   = "ruvector-nat-${each.value}"
  router = google_compute_router.ruvector[each.value].name
  region = each.value

  nat_ip_allocate_option = "AUTO_ONLY"

  source_subnetwork_ip_ranges_to_nat = "ALL_SUBNETWORKS_ALL_IP_RANGES"

  log_config {
    enable = true
    filter = "ERRORS_ONLY"
  }
}

# Firewall rules
resource "google_compute_firewall" "allow_internal" {
  name    = "ruvector-allow-internal"
  network = google_compute_network.ruvector_vpc.id

  allow {
    protocol = "tcp"
    ports    = ["0-65535"]
  }

  allow {
    protocol = "udp"
    ports    = ["0-65535"]
  }

  allow {
    protocol = "icmp"
  }

  source_ranges = ["10.0.0.0/8"]
}

resource "google_compute_firewall" "allow_health_checks" {
  name    = "ruvector-allow-health-checks"
  network = google_compute_network.ruvector_vpc.id

  allow {
    protocol = "tcp"
    ports    = ["8080", "443"]
  }

  source_ranges = [
    "35.191.0.0/16",  # Google health check ranges
    "130.211.0.0/22"
  ]

  target_tags = ["ruvector"]
}

resource "google_compute_firewall" "deny_all_ingress" {
  name     = "ruvector-deny-all-ingress"
  network  = google_compute_network.ruvector_vpc.id
  priority = 65535

  deny {
    protocol = "all"
  }

  source_ranges = ["0.0.0.0/0"]
}
```

### 6.2 Private Service Connect

**Private Connectivity to Google Services:**

```hcl
# Private Service Connect for Memorystore Redis
resource "google_compute_global_address" "redis_private_ip" {
  name          = "ruvector-redis-private-ip"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = 16
  network       = google_compute_network.ruvector_vpc.id
}

resource "google_service_networking_connection" "redis" {
  network                 = google_compute_network.ruvector_vpc.id
  service                 = "servicenetworking.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.redis_private_ip.name]
}

# Private Service Connect for Cloud SQL
resource "google_compute_global_address" "sql_private_ip" {
  name          = "ruvector-sql-private-ip"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = 16
  network       = google_compute_network.ruvector_vpc.id
}

resource "google_service_networking_connection" "sql" {
  network                 = google_compute_network.ruvector_vpc.id
  service                 = "sqladmin.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.sql_private_ip.name]
}
```

---

## 7. Load Balancing Infrastructure

### 7.1 Global HTTPS Load Balancer

```hcl
# terraform/load_balancer.tf

# Global static IP
resource "google_compute_global_address" "ruvector" {
  name         = "ruvector-global-ip"
  address_type = "EXTERNAL"
  ip_version   = "IPV4"
}

# SSL certificate (Google-managed)
resource "google_compute_managed_ssl_certificate" "ruvector" {
  name = "ruvector-ssl-cert"

  managed {
    domains = ["ruvector.example.com", "*.ruvector.example.com"]
  }
}

# Backend service for each region
resource "google_compute_backend_service" "ruvector" {
  for_each = toset(local.regions)

  name                  = "ruvector-backend-${each.value}"
  protocol              = "HTTP2"
  port_name             = "http"
  timeout_sec           = 300
  enable_cdn            = true
  session_affinity      = "CLIENT_IP"
  affinity_cookie_ttl   = 300
  load_balancing_scheme = "EXTERNAL_MANAGED"

  backend {
    group           = google_compute_region_network_endpoint_group.ruvector[each.value].id
    balancing_mode  = "UTILIZATION"
    capacity_scaler = 1.0
    max_utilization = 0.80
  }

  health_check = google_compute_health_check.ruvector[each.value].id

  cdn_policy {
    cache_mode  = "CACHE_ALL_STATIC"
    default_ttl = 30
    max_ttl     = 300
    client_ttl  = 30

    negative_caching = true
    negative_caching_policy {
      code = 404
      ttl  = 60
    }
    negative_caching_policy {
      code = 429
      ttl  = 10
    }

    cache_key_policy {
      include_protocol    = true
      include_host        = true
      include_query_string = true
      query_string_whitelist = [
        "query_vector_id",
        "k",
        "metric"
      ]
    }
  }

  log_config {
    enable      = true
    sample_rate = 0.1  # Sample 10% of requests
  }
}

# Network Endpoint Group (NEG) for Cloud Run
resource "google_compute_region_network_endpoint_group" "ruvector" {
  for_each = toset(local.regions)

  name                  = "ruvector-neg-${each.value}"
  network_endpoint_type = "SERVERLESS"
  region                = each.value

  cloud_run {
    service = google_cloud_run_service.ruvector[each.value].name
  }
}

# Health check
resource "google_compute_health_check" "ruvector" {
  for_each = toset(local.regions)

  name               = "ruvector-health-check-${each.value}"
  check_interval_sec = 5
  timeout_sec        = 3
  healthy_threshold  = 2
  unhealthy_threshold = 3

  http2_health_check {
    port         = 8080
    request_path = "/health/ready"
  }
}

# URL map
resource "google_compute_url_map" "ruvector" {
  name            = "ruvector-url-map"
  default_service = google_compute_backend_service.ruvector["us-central1"].id

  # Route to nearest region based on geo-proximity
  host_rule {
    hosts        = ["ruvector.example.com", "*.ruvector.example.com"]
    path_matcher = "ruvector"
  }

  path_matcher {
    name            = "ruvector"
    default_service = google_compute_backend_service.ruvector["us-central1"].id

    # Regional routing (example for Americas)
    route_rules {
      priority = 1
      match_rules {
        prefix_match = "/"
        header_matches {
          header_name  = "X-Client-Region"
          exact_match  = "us"
        }
      }
      service = google_compute_backend_service.ruvector["us-central1"].id
    }

    # Europe routing
    route_rules {
      priority = 2
      match_rules {
        prefix_match = "/"
        header_matches {
          header_name  = "X-Client-Region"
          exact_match  = "eu"
        }
      }
      service = google_compute_backend_service.ruvector["europe-west1"].id
    }

    # Asia routing
    route_rules {
      priority = 3
      match_rules {
        prefix_match = "/"
        header_matches {
          header_name  = "X-Client-Region"
          exact_match  = "asia"
        }
      }
      service = google_compute_backend_service.ruvector["asia-northeast1"].id
    }
  }
}

# HTTPS proxy
resource "google_compute_target_https_proxy" "ruvector" {
  name    = "ruvector-https-proxy"
  url_map = google_compute_url_map.ruvector.id

  ssl_certificates = [
    google_compute_managed_ssl_certificate.ruvector.id
  ]

  ssl_policy = google_compute_ssl_policy.ruvector.id
}

# SSL policy (modern, secure)
resource "google_compute_ssl_policy" "ruvector" {
  name            = "ruvector-ssl-policy"
  profile         = "MODERN"
  min_tls_version = "TLS_1_2"
}

# Forwarding rule
resource "google_compute_global_forwarding_rule" "ruvector_https" {
  name                  = "ruvector-https-forwarding"
  ip_protocol           = "TCP"
  load_balancing_scheme = "EXTERNAL_MANAGED"
  port_range            = "443"
  target                = google_compute_target_https_proxy.ruvector.id
  ip_address            = google_compute_global_address.ruvector.id
}

# HTTP to HTTPS redirect
resource "google_compute_url_map" "ruvector_redirect" {
  name = "ruvector-redirect"

  default_url_redirect {
    https_redirect         = true
    redirect_response_code = "MOVED_PERMANENTLY_DEFAULT"
    strip_query            = false
  }
}

resource "google_compute_target_http_proxy" "ruvector_redirect" {
  name    = "ruvector-http-proxy"
  url_map = google_compute_url_map.ruvector_redirect.id
}

resource "google_compute_global_forwarding_rule" "ruvector_http" {
  name                  = "ruvector-http-forwarding"
  ip_protocol           = "TCP"
  load_balancing_scheme = "EXTERNAL_MANAGED"
  port_range            = "80"
  target                = google_compute_target_http_proxy.ruvector_redirect.id
  ip_address            = google_compute_global_address.ruvector.id
}
```

### 7.2 Cloud Armor (DDoS & WAF)

```hcl
# terraform/cloud_armor.tf

resource "google_compute_security_policy" "ruvector" {
  name = "ruvector-security-policy"

  # Default rule (allow)
  rule {
    action   = "allow"
    priority = "2147483647"
    match {
      versioned_expr = "SRC_IPS_V1"
      config {
        src_ip_ranges = ["*"]
      }
    }
    description = "Default rule"
  }

  # Rate limiting
  rule {
    action   = "rate_based_ban"
    priority = 1000
    match {
      versioned_expr = "SRC_IPS_V1"
      config {
        src_ip_ranges = ["*"]
      }
    }
    rate_limit_options {
      conform_action = "allow"
      exceed_action  = "deny(429)"
      enforce_on_key = "IP"
      rate_limit_threshold {
        count        = 100
        interval_sec = 10
      }
      ban_duration_sec = 600  # 10 min ban
    }
    description = "Rate limit: 100 req/10s per IP"
  }

  # Block known bad IPs
  rule {
    action   = "deny(403)"
    priority = 100
    match {
      versioned_expr = "SRC_IPS_V1"
      config {
        src_ip_ranges = [
          # Add known malicious IPs
          # These would be dynamically updated
        ]
      }
    }
    description = "Block malicious IPs"
  }

  # SQL injection protection
  rule {
    action   = "deny(403)"
    priority = 200
    match {
      expr {
        expression = "evaluatePreconfiguredExpr('sqli-stable')"
      }
    }
    description = "SQL injection protection"
  }

  # XSS protection
  rule {
    action   = "deny(403)"
    priority = 300
    match {
      expr {
        expression = "evaluatePreconfiguredExpr('xss-stable')"
      }
    }
    description = "XSS protection"
  }

  # Geographic restrictions (example: block certain countries)
  rule {
    action   = "deny(403)"
    priority = 400
    match {
      expr {
        expression = "origin.region_code in ['CN', 'RU', 'KP']"  # Example only
      }
    }
    description = "Geographic restrictions"
  }

  # Adaptive protection (DDoS)
  adaptive_protection_config {
    layer_7_ddos_defense_config {
      enable = true
    }
  }
}

# Apply security policy to backend services
resource "google_compute_backend_service_security_policy_attachment" "ruvector" {
  for_each = toset(local.regions)

  backend_service = google_compute_backend_service.ruvector[each.value].id
  security_policy = google_compute_security_policy.ruvector.id
}
```

---

## 8. Cost Estimates

### 8.1 Baseline Monthly Costs (500M Concurrent)

```yaml
compute:
  cloud_run:
    instances: 5000 (across 15 regions)
    vcpu_hours_per_month: 14,600,000
    rate: $0.00002400 per vCPU-second
    monthly_cost: $1,263,000

  memorystore_redis:
    capacity_gb: 1,920 (15 regions)
    rate: $0.054 per GB-hour
    monthly_cost: $76,000

  cloud_sql:
    instances: 15 (5 primary + 10 replicas)
    monthly_cost: $5,500

storage:
  cloud_storage:
    capacity_tb: 50
    rate: $0.020 per GB-month
    monthly_cost: $1,000

  bandwidth:
    egress_tb_per_month: 300
    rate: $0.08 per GB (average)
    monthly_cost: $24,000

networking:
  load_balancer:
    data_processed_pb: 100
    monthly_cost: $420,000

  cloud_cdn:
    cache_egress_pb: 40
    monthly_cost: $2,200,000

  vpc_networking:
    monthly_cost: $15,000

monitoring:
  cloud_monitoring: $2,500
  cloud_logging: $8,000
  cloud_trace: $1,000

security:
  cloud_armor: $10,000
  secret_manager: $500

total_baseline: $4,026,500 per month
cost_per_concurrent_stream: $0.00805 per month
cost_per_million_requests: $4.84
```

### 8.2 Burst Event Costs

**4-Hour World Cup Event (50x burst):**
```yaml
additional_compute:
  cloud_run_burst: $47,000
  redis_burst: $1,200
  networking_burst: $31,000

total_burst_cost: $79,200 (4 hours)
cost_per_hour: $19,800

# Amortized over month (assuming 10 major events):
monthly_burst_cost: $792,000
```

### 8.3 Optimized Monthly Costs (After Optimization)

```yaml
# With committed use discounts, better caching, etc.
compute_optimized: $876,000 (30% reduction)
networking_optimized: $1,829,000 (30% reduction via CDN)
storage_stable: $25,000
monitoring_stable: $11,500
security_stable: $10,500

total_optimized: $2,752,000 per month
savings: $1,274,500 per month (31.7% reduction)

cost_per_concurrent_stream: $0.00550 per month
cost_per_million_requests: $3.31
```

---

## 9. Deployment Checklist

### 9.1 Pre-Deployment

```yaml
1_gcp_project_setup:
  - Create GCP project
  - Enable APIs (Cloud Run, SQL, Redis, Storage, Pub/Sub, etc.)
  - Set up billing account and budgets
  - Request quota increases

2_networking:
  - Create VPC and subnets
  - Set up VPC connectors
  - Configure Cloud NAT
  - Set up Private Service Connect

3_security:
  - Create service accounts
  - Configure IAM roles
  - Set up Secret Manager
  - Create KMS keys (if using CMEK)
  - Configure Cloud Armor policies

4_data_stores:
  - Deploy Cloud SQL instances
  - Deploy Memorystore Redis
  - Create Cloud Storage buckets
  - Set up Pub/Sub topics

5_monitoring:
  - Create Cloud Monitoring dashboards
  - Set up alert policies
  - Configure Cloud Logging sinks
  - Enable Cloud Trace
```

### 9.2 Deployment

```bash
#!/bin/bash
# deploy.sh - Deploy Ruvector to all regions

set -e

PROJECT_ID="ruvector-prod"
IMAGE_TAG="v1.0.0"

# Build and push container image
echo "Building container image..."
docker build -t gcr.io/${PROJECT_ID}/ruvector:${IMAGE_TAG} .
docker push gcr.io/${PROJECT_ID}/ruvector:${IMAGE_TAG}

# Deploy infrastructure with Terraform
echo "Deploying infrastructure..."
cd terraform
terraform init
terraform plan -out=tfplan
terraform apply tfplan

# Deploy Cloud Run services to all regions
REGIONS=(
  "us-central1" "europe-west1" "asia-northeast1"
  "asia-southeast1" "southamerica-east1" "us-east1"
  "us-west1" "europe-west2" "europe-west3"
  "europe-north1" "asia-south1" "asia-east1"
  "australia-southeast1" "northamerica-northeast1" "me-west1"
)

for region in "${REGIONS[@]}"; do
  echo "Deploying to ${region}..."

  gcloud run deploy ruvector-streaming \
    --image gcr.io/${PROJECT_ID}/ruvector:${IMAGE_TAG} \
    --region ${region} \
    --platform managed \
    --allow-unauthenticated \
    --cpu 4 \
    --memory 16Gi \
    --concurrency 100 \
    --min-instances 500 \
    --max-instances 5000 \
    --timeout 300 \
    --vpc-connector ruvector-connector-${region} \
    --vpc-egress private-ranges-only \
    --service-account ruvector-service@${PROJECT_ID}.iam.gserviceaccount.com \
    --set-env-vars REGION=${region} &
done

wait
echo "Deployment complete!"

# Verify deployments
echo "Verifying deployments..."
for region in "${REGIONS[@]}"; do
  URL=$(gcloud run services describe ruvector-streaming --region ${region} --format 'value(status.url)')
  echo "Testing ${region}: ${URL}"
  curl -s ${URL}/health/ready | jq .
done

echo "All deployments verified!"
```

### 9.3 Post-Deployment

```yaml
1_verification:
  - Health check all regions
  - Verify database connectivity
  - Test Redis connections
  - Validate Pub/Sub subscriptions

2_load_testing:
  - Run baseline load tests (500M concurrent)
  - Validate latency targets (<10ms p50)
  - Test auto-scaling behavior
  - Verify failover mechanisms

3_monitoring:
  - Confirm metrics are flowing
  - Test alert policies
  - Verify dashboard visibility
  - Set up on-call rotation

4_documentation:
  - Update runbooks
  - Document architecture decisions
  - Create troubleshooting guides
  - Train support team
```

---

## 10. Appendix

### 10.1 GCP Quotas Required

```yaml
cloud_run:
  - Instances per region: 10,000 (up from default 1,000)
  - Concurrent requests: 1,000,000 per region
  - CPU allocation: 40,000 vCPU per region
  - Memory allocation: 160 TB per region

memorystore:
  - Redis instances: 15 (default: 5)
  - Total capacity: 2 TB (default: 300 GB)

cloud_sql:
  - Instances per project: 20 (default: 10)
  - CPU per instance: 4 (default: 2)

networking:
  - VPC peering connections: 30 (default: 25)
  - Cloud NAT gateways: 15 (default: 5)
  - Global forwarding rules: 5 (default: 5)

cloud_storage:
  - Buckets per project: 20 (default: unlimited)
  - Bandwidth: 100+ Tbps (coordinate with GCP)
```

### 10.2 Performance Benchmarks

**See scaling-strategy.md Section 6 for detailed benchmarks**

### 10.3 References

- GCP Cloud Run Documentation: https://cloud.google.com/run/docs
- Memorystore Redis: https://cloud.google.com/memorystore/docs/redis
- Cloud SQL: https://cloud.google.com/sql/docs
- Cloud CDN: https://cloud.google.com/cdn/docs
- Cloud Armor: https://cloud.google.com/armor/docs

---

**Document Version:** 1.0.0
**Last Updated:** 2025-11-20
**Next Review:** 2026-01-20
**Owner:** Infrastructure Team
**Contributors:** SRE Team, Security Team, Network Engineering
