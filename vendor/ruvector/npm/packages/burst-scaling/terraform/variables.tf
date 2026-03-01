# Terraform Variables for Ruvector Burst Scaling

# ===== Project Configuration =====

variable "project_id" {
  description = "GCP Project ID"
  type        = string
}

variable "billing_account" {
  description = "GCP Billing Account ID"
  type        = string
}

variable "primary_region" {
  description = "Primary GCP region"
  type        = string
  default     = "us-central1"
}

variable "regions" {
  description = "List of regions to deploy to"
  type        = list(string)
  default     = ["us-central1", "europe-west1", "asia-east1"]
}

variable "domain" {
  description = "Domain name for the application"
  type        = string
}

# ===== Cloud Run Configuration =====

variable "container_image" {
  description = "Container image for Cloud Run"
  type        = string
  default     = "gcr.io/ruvector/app:latest"
}

variable "min_instances" {
  description = "Minimum number of Cloud Run instances per region"
  type        = number
  default     = 10
}

variable "max_instances" {
  description = "Maximum number of Cloud Run instances per region"
  type        = number
  default     = 1000
}

variable "cpu_limit" {
  description = "CPU limit for Cloud Run containers"
  type        = string
  default     = "4000m" # 4 vCPUs
}

variable "memory_limit" {
  description = "Memory limit for Cloud Run containers"
  type        = string
  default     = "8Gi" # 8GB
}

variable "max_concurrency" {
  description = "Maximum concurrent requests per Cloud Run instance"
  type        = number
  default     = 1000
}

variable "max_connections_per_instance" {
  description = "Maximum connections per Cloud Run instance"
  type        = number
  default     = 500000
}

# ===== Scaling Thresholds =====

variable "cpu_scale_out_threshold" {
  description = "CPU utilization threshold for scaling out (0-1)"
  type        = number
  default     = 0.70
}

variable "cpu_scale_in_threshold" {
  description = "CPU utilization threshold for scaling in (0-1)"
  type        = number
  default     = 0.30
}

variable "memory_scale_out_threshold" {
  description = "Memory utilization threshold for scaling out (0-1)"
  type        = number
  default     = 0.75
}

variable "memory_scale_in_threshold" {
  description = "Memory utilization threshold for scaling in (0-1)"
  type        = number
  default     = 0.35
}

variable "latency_threshold_ms" {
  description = "P99 latency threshold in milliseconds"
  type        = number
  default     = 50
}

# ===== Load Balancer Configuration =====

variable "backend_max_utilization" {
  description = "Maximum backend utilization before load balancer scales (0-1)"
  type        = number
  default     = 0.80
}

variable "circuit_breaker_max_connections" {
  description = "Maximum connections before circuit breaker trips"
  type        = number
  default     = 10000
}

variable "log_sample_rate" {
  description = "Sampling rate for load balancer logs (0-1)"
  type        = number
  default     = 0.1
}

variable "enable_iap" {
  description = "Enable Identity-Aware Proxy for admin endpoints"
  type        = bool
  default     = false
}

variable "iap_client_id" {
  description = "IAP OAuth2 Client ID"
  type        = string
  default     = ""
  sensitive   = true
}

variable "iap_client_secret" {
  description = "IAP OAuth2 Client Secret"
  type        = string
  default     = ""
  sensitive   = true
}

# ===== Database Configuration =====

variable "database_tier" {
  description = "Cloud SQL instance tier"
  type        = string
  default     = "db-custom-16-65536" # 16 vCPUs, 64GB RAM
}

variable "database_replica_tier" {
  description = "Cloud SQL read replica instance tier"
  type        = string
  default     = "db-custom-8-32768" # 8 vCPUs, 32GB RAM
}

variable "database_disk_size" {
  description = "Cloud SQL disk size in GB"
  type        = number
  default     = 500
}

variable "database_max_connections" {
  description = "Maximum database connections"
  type        = string
  default     = "5000"
}

variable "enable_read_replicas" {
  description = "Enable Cloud SQL read replicas"
  type        = bool
  default     = true
}

# ===== Redis Configuration =====

variable "redis_memory_size" {
  description = "Redis memory size in GB"
  type        = number
  default     = 64
}

# ===== Network Configuration =====

variable "vpc_cidr" {
  description = "VPC CIDR block"
  type        = string
  default     = "10.0.0.0/16"
}

# ===== Budget Configuration =====

variable "hourly_budget" {
  description = "Hourly budget limit in USD"
  type        = number
  default     = 10000
}

variable "daily_budget" {
  description = "Daily budget limit in USD"
  type        = number
  default     = 200000
}

variable "monthly_budget" {
  description = "Monthly budget limit in USD"
  type        = number
  default     = 5000000
}

variable "budget_warning_threshold" {
  description = "Budget warning threshold (0-1)"
  type        = number
  default     = 0.80
}

variable "hard_budget_limit" {
  description = "Enforce hard budget limit (stop scaling when reached)"
  type        = bool
  default     = false
}

# ===== Alerting Configuration =====

variable "alert_email" {
  description = "Email address for alerts"
  type        = string
}

variable "pagerduty_integration_key" {
  description = "PagerDuty integration key for critical alerts"
  type        = string
  default     = ""
  sensitive   = true
}

# ===== Burst Event Configuration =====

variable "burst_multiplier_max" {
  description = "Maximum burst multiplier (e.g., 50 for 50x normal load)"
  type        = number
  default     = 50
}

variable "pre_warm_time_seconds" {
  description = "Time in seconds to start pre-warming before predicted burst"
  type        = number
  default     = 900 # 15 minutes
}

variable "scale_out_step" {
  description = "Number of instances to add during scale-out"
  type        = number
  default     = 10
}

variable "scale_in_step" {
  description = "Number of instances to remove during scale-in"
  type        = number
  default     = 2
}

variable "scale_out_cooldown_seconds" {
  description = "Cooldown period after scale-out in seconds"
  type        = number
  default     = 60
}

variable "scale_in_cooldown_seconds" {
  description = "Cooldown period after scale-in in seconds"
  type        = number
  default     = 300
}

# ===== Cost Optimization =====

variable "enable_deletion_protection" {
  description = "Enable deletion protection for databases"
  type        = bool
  default     = true
}

variable "enable_preemptible_instances" {
  description = "Use preemptible instances for non-critical workloads"
  type        = bool
  default     = false
}

# ===== Regional Cost Configuration =====

variable "region_costs" {
  description = "Hourly cost per instance by region (USD)"
  type        = map(number)
  default = {
    "us-central1"         = 0.50
    "us-east1"            = 0.52
    "us-west1"            = 0.54
    "europe-west1"        = 0.55
    "europe-west4"        = 0.58
    "asia-east1"          = 0.60
    "asia-southeast1"     = 0.62
    "south-america-east1" = 0.65
  }
}

variable "region_priorities" {
  description = "Priority ranking for regions (1-10, higher = more important)"
  type        = map(number)
  default = {
    "us-central1"         = 10
    "us-east1"            = 9
    "europe-west1"        = 9
    "asia-east1"          = 8
    "us-west1"            = 7
    "asia-southeast1"     = 6
    "europe-west4"        = 6
    "south-america-east1" = 5
  }
}

# ===== Monitoring Configuration =====

variable "metrics_retention_days" {
  description = "Number of days to retain monitoring metrics"
  type        = number
  default     = 90
}

variable "enable_cloud_trace" {
  description = "Enable Cloud Trace for distributed tracing"
  type        = bool
  default     = true
}

variable "trace_sample_rate" {
  description = "Sampling rate for Cloud Trace (0-1)"
  type        = number
  default     = 0.1
}

variable "enable_cloud_profiler" {
  description = "Enable Cloud Profiler for performance profiling"
  type        = bool
  default     = true
}

# ===== Environment =====

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "tags" {
  description = "Additional tags for resources"
  type        = map(string)
  default = {
    "managed-by" = "terraform"
    "project"    = "ruvector"
    "component"  = "burst-scaling"
  }
}

# ===== Feature Flags =====

variable "enable_adaptive_scaling" {
  description = "Enable adaptive scaling with ML predictions"
  type        = bool
  default     = true
}

variable "enable_traffic_shedding" {
  description = "Enable traffic shedding during extreme load"
  type        = bool
  default     = true
}

variable "enable_graceful_degradation" {
  description = "Enable graceful degradation features"
  type        = bool
  default     = true
}

# ===== Example terraform.tfvars =====

# Copy this to terraform.tfvars and customize:
#
# project_id      = "ruvector-prod"
# billing_account = "0123AB-CDEF45-67890"
# domain          = "api.ruvector.io"
# alert_email     = "ops@ruvector.io"
#
# regions = [
#   "us-central1",
#   "europe-west1",
#   "asia-east1"
# ]
#
# # Burst scaling
# min_instances       = 10
# max_instances       = 1000
# burst_multiplier_max = 50
#
# # Budget
# hourly_budget   = 10000
# daily_budget    = 200000
# monthly_budget  = 5000000
#
# # Thresholds
# cpu_scale_out_threshold = 0.70
# latency_threshold_ms    = 50
