provider "google" {}

locals {
  control_plane_email = "ruv-social-control@${var.project_id}.iam.gserviceaccount.com"
  heartbeat_email     = "ruv-social-heartbeat@${var.project_id}.iam.gserviceaccount.com"
  required_services = toset([
    "artifactregistry.googleapis.com",
    "cloudresourcemanager.googleapis.com",
    "iam.googleapis.com",
    "run.googleapis.com",
    "serviceusage.googleapis.com"
  ])
}

resource "google_project_service" "required" {
  for_each           = local.required_services
  project            = var.project_id
  service            = each.value
  disable_on_destroy = false
}

resource "google_service_account" "control_plane" {
  project      = var.project_id
  account_id   = "ruv-social-control"
  display_name = "RuV social read only control plane"

  depends_on = [google_project_service.required]
}

resource "google_service_account" "heartbeat" {
  count        = var.enable_heartbeat ? 1 : 0
  project      = var.project_id
  account_id   = "ruv-social-heartbeat"
  display_name = "RuV social Cloud Run heartbeat invoker"

  depends_on = [google_project_service.required]
}

resource "google_cloud_run_v2_service" "control_plane" {
  project             = var.project_id
  name                = var.service_name
  location            = var.region
  ingress             = "INGRESS_TRAFFIC_INTERNAL_ONLY"
  deletion_protection = true

  lifecycle {
    precondition {
      condition     = var.minimum_instances <= var.maximum_instances
      error_message = "minimum_instances cannot exceed maximum_instances."
    }
    precondition {
      condition     = startswith(var.container_image, "${var.region}-docker.pkg.dev/${var.project_id}/")
      error_message = "container_image registry location and project must match region and project_id."
    }
  }

  template {
    annotations = {
      "ruvnet.dev/image-evidence-digest" = var.image_evidence_digest
    }
    service_account = local.control_plane_email
    timeout         = "10s"

    scaling {
      min_instance_count = var.minimum_instances
      max_instance_count = var.maximum_instances
    }

    containers {
      image = var.container_image

      resources {
        cpu_idle = true
        limits = {
          cpu    = "1"
          memory = "512Mi"
        }
      }

      startup_probe {
        initial_delay_seconds = 1
        timeout_seconds       = 2
        period_seconds        = 5
        failure_threshold     = 6
        http_get {
          path = "/healthz"
          port = 8080
        }
      }
    }
  }

  depends_on = [
    google_project_service.required,
    google_service_account.control_plane
  ]
}

resource "google_cloud_run_v2_service_iam_member" "heartbeat_invoker" {
  count    = var.enable_heartbeat ? 1 : 0
  project  = var.project_id
  location = google_cloud_run_v2_service.control_plane.location
  name     = google_cloud_run_v2_service.control_plane.name
  role     = "roles/run.invoker"
  member   = "serviceAccount:${local.heartbeat_email}"

  depends_on = [google_service_account.heartbeat]
}

resource "google_project_service" "scheduler" {
  count              = var.enable_heartbeat ? 1 : 0
  project            = var.project_id
  service            = "cloudscheduler.googleapis.com"
  disable_on_destroy = false
}

resource "google_cloud_scheduler_job" "heartbeat" {
  count       = var.enable_heartbeat ? 1 : 0
  project     = var.project_id
  region      = var.region
  name        = "${var.service_name}-heartbeat"
  description = "Authenticated health check. It does not provide perpetual execution."
  schedule    = "*/5 * * * *"
  time_zone   = "UTC"

  http_target {
    http_method = "GET"
    uri         = "${google_cloud_run_v2_service.control_plane.uri}/healthz"
    oidc_token {
      service_account_email = local.heartbeat_email
      audience              = google_cloud_run_v2_service.control_plane.uri
    }
  }

  depends_on = [
    google_cloud_run_v2_service_iam_member.heartbeat_invoker,
    google_project_service.scheduler
  ]
}
