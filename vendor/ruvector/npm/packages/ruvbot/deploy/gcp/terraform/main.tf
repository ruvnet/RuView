# =============================================================================
# RuvBot - Google Cloud Platform Infrastructure
# =============================================================================
# Cost-optimized deployment using:
# - Cloud Run (serverless, pay-per-use)
# - Cloud SQL PostgreSQL (smallest instance, can scale)
# - Memorystore Redis (optional, can use in-memory)
# - Secret Manager (for credentials)
# - Cloud Storage (for file uploads)
#
# Estimated monthly cost (low traffic): $15-30/month
# - Cloud Run: ~$0 (generous free tier)
# - Cloud SQL: ~$10-15/month (db-f1-micro)
# - Secret Manager: ~$0.06/secret/month
# - Cloud Storage: ~$0.02/GB/month
# =============================================================================

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    google-beta = {
      source  = "hashicorp/google-beta"
      version = "~> 5.0"
    }
  }

  # Uncomment for remote state (recommended for production)
  # backend "gcs" {
  #   bucket = "your-terraform-state-bucket"
  #   prefix = "ruvbot/state"
  # }
}

# -----------------------------------------------------------------------------
# Variables
# -----------------------------------------------------------------------------

variable "project_id" {
  description = "GCP Project ID"
  type        = string
}

variable "region" {
  description = "GCP Region"
  type        = string
  default     = "us-central1"
}

variable "environment" {
  description = "Environment (dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "enable_cloud_sql" {
  description = "Enable Cloud SQL PostgreSQL (adds ~$10/month)"
  type        = bool
  default     = true
}

variable "enable_redis" {
  description = "Enable Memorystore Redis (adds ~$30/month)"
  type        = bool
  default     = false  # Disabled by default for cost savings
}

variable "anthropic_api_key" {
  description = "Anthropic API Key"
  type        = string
  sensitive   = true
}

variable "openrouter_api_key" {
  description = "OpenRouter API Key"
  type        = string
  sensitive   = true
  default     = ""
}

# -----------------------------------------------------------------------------
# Provider Configuration
# -----------------------------------------------------------------------------

provider "google" {
  project = var.project_id
  region  = var.region
}

provider "google-beta" {
  project = var.project_id
  region  = var.region
}

# -----------------------------------------------------------------------------
# Enable Required APIs
# -----------------------------------------------------------------------------

resource "google_project_service" "services" {
  for_each = toset([
    "run.googleapis.com",
    "cloudbuild.googleapis.com",
    "secretmanager.googleapis.com",
    "sqladmin.googleapis.com",
    "storage.googleapis.com",
    "redis.googleapis.com",
    "vpcaccess.googleapis.com",
  ])

  service            = each.value
  disable_on_destroy = false
}

# -----------------------------------------------------------------------------
# Service Account for Cloud Run
# -----------------------------------------------------------------------------

resource "google_service_account" "ruvbot_runner" {
  account_id   = "ruvbot-runner"
  display_name = "RuvBot Cloud Run Service Account"
  description  = "Service account for RuvBot Cloud Run service"
}

resource "google_project_iam_member" "ruvbot_runner_roles" {
  for_each = toset([
    "roles/secretmanager.secretAccessor",
    "roles/cloudsql.client",
    "roles/storage.objectAdmin",
    "roles/logging.logWriter",
    "roles/monitoring.metricWriter",
  ])

  project = var.project_id
  role    = each.value
  member  = "serviceAccount:${google_service_account.ruvbot_runner.email}"
}

# -----------------------------------------------------------------------------
# Secret Manager - Store API Keys Securely
# -----------------------------------------------------------------------------

resource "google_secret_manager_secret" "anthropic_api_key" {
  secret_id = "anthropic-api-key"

  replication {
    auto {}
  }

  depends_on = [google_project_service.services]
}

resource "google_secret_manager_secret_version" "anthropic_api_key" {
  secret      = google_secret_manager_secret.anthropic_api_key.id
  secret_data = var.anthropic_api_key
}

resource "google_secret_manager_secret" "openrouter_api_key" {
  count     = var.openrouter_api_key != "" ? 1 : 0
  secret_id = "openrouter-api-key"

  replication {
    auto {}
  }

  depends_on = [google_project_service.services]
}

resource "google_secret_manager_secret_version" "openrouter_api_key" {
  count       = var.openrouter_api_key != "" ? 1 : 0
  secret      = google_secret_manager_secret.openrouter_api_key[0].id
  secret_data = var.openrouter_api_key
}

# -----------------------------------------------------------------------------
# Cloud SQL PostgreSQL (Cost-Optimized)
# -----------------------------------------------------------------------------

resource "google_sql_database_instance" "ruvbot" {
  count            = var.enable_cloud_sql ? 1 : 0
  name             = "ruvbot-${var.environment}"
  database_version = "POSTGRES_16"
  region           = var.region

  settings {
    # db-f1-micro: 0.6GB RAM, shared CPU - ~$10/month
    tier              = "db-f1-micro"
    availability_type = "ZONAL"  # Single zone for cost savings

    disk_size       = 10  # Minimum 10GB
    disk_type       = "PD_SSD"
    disk_autoresize = true

    backup_configuration {
      enabled                        = true
      point_in_time_recovery_enabled = false  # Disable for cost savings
      backup_retention_settings {
        retained_backups = 7
      }
    }

    ip_configuration {
      ipv4_enabled = true
      # For production, use private IP with VPC connector
      authorized_networks {
        name  = "allow-cloud-run"
        value = "0.0.0.0/0"  # Cloud Run uses public IP; restrict in production
      }
    }

    database_flags {
      name  = "max_connections"
      value = "50"
    }
  }

  deletion_protection = var.environment == "prod"

  depends_on = [google_project_service.services]
}

resource "google_sql_database" "ruvbot" {
  count    = var.enable_cloud_sql ? 1 : 0
  name     = "ruvbot"
  instance = google_sql_database_instance.ruvbot[0].name
}

resource "google_sql_user" "ruvbot" {
  count    = var.enable_cloud_sql ? 1 : 0
  name     = "ruvbot"
  instance = google_sql_database_instance.ruvbot[0].name
  password = random_password.db_password[0].result
}

resource "random_password" "db_password" {
  count   = var.enable_cloud_sql ? 1 : 0
  length  = 32
  special = false
}

resource "google_secret_manager_secret" "database_url" {
  count     = var.enable_cloud_sql ? 1 : 0
  secret_id = "database-url"

  replication {
    auto {}
  }

  depends_on = [google_project_service.services]
}

resource "google_secret_manager_secret_version" "database_url" {
  count       = var.enable_cloud_sql ? 1 : 0
  secret      = google_secret_manager_secret.database_url[0].id
  secret_data = "postgresql://ruvbot:${random_password.db_password[0].result}@${google_sql_database_instance.ruvbot[0].public_ip_address}:5432/ruvbot"
}

# -----------------------------------------------------------------------------
# Cloud Storage Bucket (for file uploads)
# -----------------------------------------------------------------------------

resource "google_storage_bucket" "ruvbot_data" {
  name          = "${var.project_id}-ruvbot-data"
  location      = var.region
  force_destroy = var.environment != "prod"

  uniform_bucket_level_access = true

  lifecycle_rule {
    condition {
      age = 30
    }
    action {
      type          = "SetStorageClass"
      storage_class = "NEARLINE"
    }
  }

  lifecycle_rule {
    condition {
      age = 90
    }
    action {
      type          = "SetStorageClass"
      storage_class = "COLDLINE"
    }
  }

  versioning {
    enabled = var.environment == "prod"
  }
}

# -----------------------------------------------------------------------------
# Cloud Run Service
# -----------------------------------------------------------------------------

resource "google_cloud_run_v2_service" "ruvbot" {
  name     = "ruvbot"
  location = var.region
  ingress  = "INGRESS_TRAFFIC_ALL"

  template {
    service_account = google_service_account.ruvbot_runner.email

    scaling {
      min_instance_count = 0  # Scale to zero when not in use
      max_instance_count = 10
    }

    containers {
      image = "gcr.io/${var.project_id}/ruvbot:latest"

      ports {
        container_port = 8080
      }

      resources {
        limits = {
          cpu    = "1"
          memory = "512Mi"
        }
        cpu_idle          = true  # Reduce cost during idle
        startup_cpu_boost = true  # Faster cold starts
      }

      env {
        name  = "NODE_ENV"
        value = "production"
      }

      env {
        name  = "GCS_BUCKET"
        value = google_storage_bucket.ruvbot_data.name
      }

      env {
        name = "ANTHROPIC_API_KEY"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.anthropic_api_key.secret_id
            version = "latest"
          }
        }
      }

      dynamic "env" {
        for_each = var.enable_cloud_sql ? [1] : []
        content {
          name = "DATABASE_URL"
          value_source {
            secret_key_ref {
              secret  = google_secret_manager_secret.database_url[0].secret_id
              version = "latest"
            }
          }
        }
      }

      startup_probe {
        http_get {
          path = "/health"
          port = 8080
        }
        initial_delay_seconds = 5
        timeout_seconds       = 3
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        http_get {
          path = "/health"
          port = 8080
        }
        timeout_seconds   = 3
        period_seconds    = 30
        failure_threshold = 3
      }
    }

    max_instance_request_concurrency = 80
    timeout                          = "300s"
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  depends_on = [
    google_project_service.services,
    google_secret_manager_secret_version.anthropic_api_key,
  ]
}

# -----------------------------------------------------------------------------
# Allow Unauthenticated Access to Cloud Run
# -----------------------------------------------------------------------------

resource "google_cloud_run_v2_service_iam_member" "public_access" {
  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.ruvbot.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

# -----------------------------------------------------------------------------
# Outputs
# -----------------------------------------------------------------------------

output "cloud_run_url" {
  description = "Cloud Run service URL"
  value       = google_cloud_run_v2_service.ruvbot.uri
}

output "cloud_sql_connection_name" {
  description = "Cloud SQL connection name"
  value       = var.enable_cloud_sql ? google_sql_database_instance.ruvbot[0].connection_name : "N/A"
}

output "storage_bucket" {
  description = "Cloud Storage bucket name"
  value       = google_storage_bucket.ruvbot_data.name
}

output "estimated_monthly_cost" {
  description = "Estimated monthly cost"
  value       = <<-EOT
    Estimated Monthly Cost (low traffic):
    - Cloud Run: ~$0 (free tier covers ~2M requests)
    - Cloud SQL: ${var.enable_cloud_sql ? "~$10-15" : "$0"}
    - Secret Manager: ~$0.18 (3 secrets)
    - Cloud Storage: ~$0.02/GB
    - Redis: ${var.enable_redis ? "~$30" : "$0 (disabled)"}
    ----------------------------------------
    Total: ~$${var.enable_cloud_sql ? (var.enable_redis ? "45-50" : "15-20") : "5-10"}/month
  EOT
}
