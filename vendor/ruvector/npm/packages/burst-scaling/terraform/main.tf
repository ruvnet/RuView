# Ruvector Burst Scaling Infrastructure
#
# This Terraform configuration manages:
# - Cloud Run services with auto-scaling
# - Load balancers
# - Cloud SQL and Redis with scaling policies
# - Monitoring and alerting
# - Budget alerts

terraform {
  required_version = ">= 1.0"

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

  backend "gcs" {
    bucket = "ruvector-terraform-state"
    prefix = "burst-scaling"
  }
}

provider "google" {
  project = var.project_id
  region  = var.primary_region
}

provider "google-beta" {
  project = var.project_id
  region  = var.primary_region
}

# ===== Cloud Run Services =====

resource "google_cloud_run_v2_service" "ruvector" {
  for_each = toset(var.regions)

  name     = "ruvector-${each.key}"
  location = each.key

  template {
    scaling {
      min_instance_count = var.min_instances
      max_instance_count = var.max_instances
    }

    containers {
      image = var.container_image

      resources {
        limits = {
          cpu    = var.cpu_limit
          memory = var.memory_limit
        }

        cpu_idle = true
        startup_cpu_boost = true
      }

      ports {
        container_port = 8080
        name          = "http1"
      }

      env {
        name  = "REGION"
        value = each.key
      }

      env {
        name  = "MAX_CONNECTIONS"
        value = tostring(var.max_connections_per_instance)
      }

      env {
        name = "DATABASE_URL"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.database_url.id
            version = "latest"
          }
        }
      }

      env {
        name = "REDIS_URL"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.redis_url.id
            version = "latest"
          }
        }
      }
    }

    # Aggressive auto-scaling configuration
    max_instance_request_concurrency = var.max_concurrency

    service_account = google_service_account.ruvector.email

    timeout = "300s"
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }

  depends_on = [
    google_project_service.cloud_run,
    google_secret_manager_secret_iam_member.cloud_run_database,
    google_secret_manager_secret_iam_member.cloud_run_redis
  ]
}

# Auto-scaling policies for Cloud Run
resource "google_monitoring_alert_policy" "high_cpu" {
  for_each = toset(var.regions)

  display_name = "High CPU - ${each.key}"
  combiner     = "OR"

  conditions {
    display_name = "CPU utilization above ${var.cpu_scale_out_threshold * 100}%"

    condition_threshold {
      filter          = "resource.type = \"cloud_run_revision\" AND resource.labels.service_name = \"ruvector-${each.key}\" AND metric.type = \"run.googleapis.com/container/cpu/utilizations\""
      duration        = "60s"
      comparison      = "COMPARISON_GT"
      threshold_value = var.cpu_scale_out_threshold

      aggregations {
        alignment_period   = "60s"
        per_series_aligner = "ALIGN_MEAN"
      }
    }
  }

  notification_channels = [google_monitoring_notification_channel.email.id]

  alert_strategy {
    auto_close = "1800s"
  }
}

# ===== Global Load Balancer =====

resource "google_compute_global_address" "ruvector" {
  name = "ruvector-lb-ip"
}

resource "google_compute_global_forwarding_rule" "ruvector" {
  name                  = "ruvector-lb-forwarding-rule"
  target                = google_compute_target_https_proxy.ruvector.id
  port_range            = "443"
  ip_address            = google_compute_global_address.ruvector.address
  load_balancing_scheme = "EXTERNAL_MANAGED"
}

resource "google_compute_target_https_proxy" "ruvector" {
  name             = "ruvector-https-proxy"
  url_map          = google_compute_url_map.ruvector.id
  ssl_certificates = [google_compute_managed_ssl_certificate.ruvector.id]
}

resource "google_compute_managed_ssl_certificate" "ruvector" {
  name = "ruvector-ssl-cert"

  managed {
    domains = [var.domain]
  }
}

resource "google_compute_url_map" "ruvector" {
  name            = "ruvector-url-map"
  default_service = google_compute_backend_service.ruvector.id
}

resource "google_compute_backend_service" "ruvector" {
  name                  = "ruvector-backend"
  protocol              = "HTTP"
  port_name             = "http"
  timeout_sec           = 30
  load_balancing_scheme = "EXTERNAL_MANAGED"

  # Health check
  health_checks = [google_compute_health_check.ruvector.id]

  # CDN configuration
  enable_cdn = true
  cdn_policy {
    cache_mode                   = "CACHE_ALL_STATIC"
    default_ttl                  = 3600
    client_ttl                   = 3600
    max_ttl                      = 86400
    negative_caching             = true
    serve_while_stale            = 86400
  }

  # IAP for admin endpoints
  iap {
    enabled = var.enable_iap
    oauth2_client_id     = var.iap_client_id
    oauth2_client_secret = var.iap_client_secret
  }

  # Add backends for each region
  dynamic "backend" {
    for_each = toset(var.regions)

    content {
      group = google_compute_region_network_endpoint_group.ruvector[backend.key].id

      balancing_mode               = "UTILIZATION"
      capacity_scaler              = 1.0
      max_utilization              = var.backend_max_utilization

      # Connection draining
      max_connections_per_instance = var.max_connections_per_instance
    }
  }

  # Circuit breaker
  circuit_breakers {
    max_connections = var.circuit_breaker_max_connections
  }

  # Outlier detection
  outlier_detection {
    consecutive_errors                    = 5
    interval {
      seconds = 10
    }
    base_ejection_time {
      seconds = 30
    }
    max_ejection_percent                  = 50
    enforcing_consecutive_errors          = 100
  }

  # Log configuration
  log_config {
    enable      = true
    sample_rate = var.log_sample_rate
  }
}

resource "google_compute_region_network_endpoint_group" "ruvector" {
  for_each = toset(var.regions)

  name                  = "ruvector-neg-${each.key}"
  network_endpoint_type = "SERVERLESS"
  region                = each.key

  cloud_run {
    service = google_cloud_run_v2_service.ruvector[each.key].name
  }
}

resource "google_compute_health_check" "ruvector" {
  name                = "ruvector-health-check"
  check_interval_sec  = 10
  timeout_sec         = 5
  healthy_threshold   = 2
  unhealthy_threshold = 3

  http_health_check {
    port               = 8080
    request_path       = "/health"
    proxy_header       = "NONE"
  }
}

# ===== Cloud SQL (PostgreSQL) =====

resource "google_sql_database_instance" "ruvector" {
  for_each = toset(var.regions)

  name             = "ruvector-db-${each.key}"
  database_version = "POSTGRES_15"
  region           = each.key

  settings {
    tier              = var.database_tier
    availability_type = "REGIONAL"
    disk_autoresize   = true
    disk_size         = var.database_disk_size
    disk_type         = "PD_SSD"

    backup_configuration {
      enabled                        = true
      point_in_time_recovery_enabled = true
      start_time                     = "03:00"
      transaction_log_retention_days = 7
      backup_retention_settings {
        retained_backups = 30
      }
    }

    ip_configuration {
      ipv4_enabled    = false
      private_network = google_compute_network.ruvector.id
      require_ssl     = true
    }

    insights_config {
      query_insights_enabled  = true
      query_string_length     = 1024
      record_application_tags = true
      record_client_address   = true
    }

    database_flags {
      name  = "max_connections"
      value = var.database_max_connections
    }

    database_flags {
      name  = "shared_buffers"
      value = "262144" # 2GB
    }

    database_flags {
      name  = "effective_cache_size"
      value = "786432" # 6GB
    }
  }

  deletion_protection = var.enable_deletion_protection

  depends_on = [
    google_project_service.sql_admin,
    google_service_networking_connection.private_vpc_connection
  ]
}

# Read replicas for scaling reads
resource "google_sql_database_instance" "ruvector_replica" {
  for_each = var.enable_read_replicas ? toset(var.regions) : toset([])

  name                 = "ruvector-db-${each.key}-replica"
  master_instance_name = google_sql_database_instance.ruvector[each.key].name
  region               = each.key
  database_version     = "POSTGRES_15"

  replica_configuration {
    failover_target = false
  }

  settings {
    tier              = var.database_replica_tier
    availability_type = "ZONAL"
    disk_autoresize   = true
    disk_type         = "PD_SSD"

    ip_configuration {
      ipv4_enabled    = false
      private_network = google_compute_network.ruvector.id
    }
  }

  deletion_protection = var.enable_deletion_protection
}

# ===== Redis (Memorystore) =====

resource "google_redis_instance" "ruvector" {
  for_each = toset(var.regions)

  name               = "ruvector-redis-${each.key}"
  tier               = "STANDARD_HA"
  memory_size_gb     = var.redis_memory_size
  region             = each.key
  redis_version      = "REDIS_7_0"
  display_name       = "Ruvector Redis - ${each.key}"

  authorized_network = google_compute_network.ruvector.id
  connect_mode       = "PRIVATE_SERVICE_ACCESS"

  redis_configs = {
    maxmemory-policy = "allkeys-lru"
    notify-keyspace-events = "Ex"
  }

  maintenance_policy {
    weekly_maintenance_window {
      day = "SUNDAY"
      start_time {
        hours   = 3
        minutes = 0
      }
    }
  }

  depends_on = [
    google_project_service.redis,
    google_service_networking_connection.private_vpc_connection
  ]
}

# ===== Networking =====

resource "google_compute_network" "ruvector" {
  name                    = "ruvector-network"
  auto_create_subnetworks = false
}

resource "google_compute_subnetwork" "ruvector" {
  for_each = toset(var.regions)

  name          = "ruvector-subnet-${each.key}"
  ip_cidr_range = cidrsubnet(var.vpc_cidr, 8, index(var.regions, each.key))
  region        = each.key
  network       = google_compute_network.ruvector.id

  private_ip_google_access = true
}

resource "google_compute_global_address" "private_ip_address" {
  name          = "ruvector-private-ip"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = 16
  network       = google_compute_network.ruvector.id
}

resource "google_service_networking_connection" "private_vpc_connection" {
  network                 = google_compute_network.ruvector.id
  service                 = "servicenetworking.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.private_ip_address.name]
}

# ===== IAM & Service Accounts =====

resource "google_service_account" "ruvector" {
  account_id   = "ruvector-service"
  display_name = "Ruvector Service Account"
}

resource "google_project_iam_member" "ruvector_monitoring" {
  project = var.project_id
  role    = "roles/monitoring.metricWriter"
  member  = "serviceAccount:${google_service_account.ruvector.email}"
}

resource "google_project_iam_member" "ruvector_logging" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.ruvector.email}"
}

resource "google_project_iam_member" "ruvector_trace" {
  project = var.project_id
  role    = "roles/cloudtrace.agent"
  member  = "serviceAccount:${google_service_account.ruvector.email}"
}

# ===== Secrets Manager =====

resource "google_secret_manager_secret" "database_url" {
  secret_id = "ruvector-database-url"

  replication {
    auto {}
  }
}

resource "google_secret_manager_secret" "redis_url" {
  secret_id = "ruvector-redis-url"

  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_iam_member" "cloud_run_database" {
  secret_id = google_secret_manager_secret.database_url.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.ruvector.email}"
}

resource "google_secret_manager_secret_iam_member" "cloud_run_redis" {
  secret_id = google_secret_manager_secret.redis_url.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.ruvector.email}"
}

# ===== Monitoring & Alerts =====

resource "google_monitoring_notification_channel" "email" {
  display_name = "Email Notifications"
  type         = "email"

  labels = {
    email_address = var.alert_email
  }
}

resource "google_monitoring_notification_channel" "pagerduty" {
  count = var.pagerduty_integration_key != "" ? 1 : 0

  display_name = "PagerDuty"
  type         = "pagerduty"

  sensitive_labels {
    service_key = var.pagerduty_integration_key
  }
}

# Budget alerts
resource "google_billing_budget" "ruvector" {
  billing_account = var.billing_account
  display_name    = "Ruvector Budget"

  budget_filter {
    projects = ["projects/${var.project_id}"]
  }

  amount {
    specified_amount {
      currency_code = "USD"
      units         = tostring(var.monthly_budget)
    }
  }

  threshold_rules {
    threshold_percent = 0.5
  }

  threshold_rules {
    threshold_percent = 0.8
  }

  threshold_rules {
    threshold_percent = 0.9
  }

  threshold_rules {
    threshold_percent = 1.0
  }

  threshold_rules {
    threshold_percent = 1.2
    spend_basis       = "FORECASTED_SPEND"
  }

  all_updates_rule {
    monitoring_notification_channels = [
      google_monitoring_notification_channel.email.id
    ]
    disable_default_iam_recipients = false
  }
}

# ===== Enable Required APIs =====

resource "google_project_service" "cloud_run" {
  service            = "run.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "compute" {
  service            = "compute.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "sql_admin" {
  service            = "sqladmin.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "redis" {
  service            = "redis.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "monitoring" {
  service            = "monitoring.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "logging" {
  service            = "logging.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "secretmanager" {
  service            = "secretmanager.googleapis.com"
  disable_on_destroy = false
}

# ===== Outputs =====

output "load_balancer_ip" {
  description = "Global load balancer IP address"
  value       = google_compute_global_address.ruvector.address
}

output "cloud_run_services" {
  description = "Cloud Run service URLs by region"
  value = {
    for region, service in google_cloud_run_v2_service.ruvector :
    region => service.uri
  }
}

output "database_instances" {
  description = "Cloud SQL instance connection names"
  value = {
    for region, db in google_sql_database_instance.ruvector :
    region => db.connection_name
  }
}

output "redis_instances" {
  description = "Redis instance hosts"
  value = {
    for region, redis in google_redis_instance.ruvector :
    region => redis.host
  }
  sensitive = true
}
