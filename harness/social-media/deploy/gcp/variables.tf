variable "project_id" {
  description = "Google Cloud project dedicated to the social metaharness."
  type        = string
}

variable "region" {
  description = "Single region used by the optional Phase G1 control plane."
  type        = string
  default     = "us-central1"
}

variable "container_image" {
  description = "Immutable Artifact Registry image reference. Use a digest, not a mutable tag."
  type        = string
  validation {
    condition     = can(regex("^[a-z0-9-]+-docker\\.pkg\\.dev/[a-z0-9._:-]+/[a-z0-9._-]+/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$", var.container_image))
    error_message = "container_image must be a full Artifact Registry URI pinned by sha256 digest."
  }
}

variable "image_evidence_digest" {
  description = "Canonical SocialImageEvidenceV1 digest reviewed with the application image."
  type        = string
  validation {
    condition     = can(regex("^sha256:[0-9a-f]{64}$", var.image_evidence_digest))
    error_message = "image_evidence_digest must be a lowercase sha256 digest."
  }
}

variable "service_name" {
  description = "Cloud Run service name."
  type        = string
  default     = "ruv-social-control"
}

variable "minimum_instances" {
  description = "Warm control plane instances. Zero is the cost controlled default."
  type        = number
  default     = 0
  validation {
    condition     = var.minimum_instances >= 0 && var.minimum_instances <= 3 && floor(var.minimum_instances) == var.minimum_instances
    error_message = "minimum_instances must be between zero and three."
  }
}

variable "maximum_instances" {
  description = "Hard scale ceiling for the read only control plane."
  type        = number
  default     = 3
  validation {
    condition     = var.maximum_instances >= 1 && var.maximum_instances <= 3 && floor(var.maximum_instances) == var.maximum_instances
    error_message = "maximum_instances must be between one and three."
  }
}

variable "enable_heartbeat" {
  description = "Create an authenticated five minute health check. This improves detection, not perpetual availability."
  type        = bool
  default     = false
}
