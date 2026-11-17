output "service_name" {
  value       = google_cloud_run_v2_service.control_plane.name
  description = "Internal only Cloud Run control plane name."
}

output "service_uri" {
  value       = google_cloud_run_v2_service.control_plane.uri
  description = "Internal Cloud Run URI. IAM and ingress still apply."
}

output "control_plane_service_account" {
  value       = google_service_account.control_plane.email
  description = "Least authority runtime identity. No platform role is granted by this module."
}
