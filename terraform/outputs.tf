# WiFi-DensePose Terraform Outputs
# This file defines outputs that can be used by other Terraform configurations or external systems

# VPC Outputs
output "vpc_id" {
  description = "ID of the VPC"
  value       = aws_vpc.main.id
}

output "vpc_cidr_block" {
  description = "CIDR block of the VPC"
  value       = aws_vpc.main.cidr_block
}

output "public_subnet_ids" {
  description = "IDs of the public subnets"
  value       = aws_subnet.public[*].id
}

output "private_subnet_ids" {
  description = "IDs of the private subnets"
  value       = aws_subnet.private[*].id
}

output "internet_gateway_id" {
  description = "ID of the Internet Gateway"
  value       = aws_internet_gateway.main.id
}

output "nat_gateway_ids" {
  description = "IDs of the NAT Gateways"
  value       = aws_nat_gateway.main[*].id
}

# EKS Cluster Outputs
output "cluster_id" {
  description = "EKS cluster ID"
  value       = aws_eks_cluster.main.id
}

output "cluster_arn" {
  description = "EKS cluster ARN"
  value       = aws_eks_cluster.main.arn
}

output "cluster_endpoint" {
  description = "Endpoint for EKS control plane"
  value       = aws_eks_cluster.main.endpoint
}

output "cluster_security_group_id" {
  description = "Security group ID attached to the EKS cluster"
  value       = aws_eks_cluster.main.vpc_config[0].cluster_security_group_id
}

output "cluster_iam_role_name" {
  description = "IAM role name associated with EKS cluster"
  value       = aws_iam_role.eks_cluster.name
}

output "cluster_iam_role_arn" {
  description = "IAM role ARN associated with EKS cluster"
  value       = aws_iam_role.eks_cluster.arn
}

output "cluster_certificate_authority_data" {
  description = "Base64 encoded certificate data required to communicate with the cluster"
  value       = aws_eks_cluster.main.certificate_authority[0].data
}

output "cluster_primary_security_group_id" {
  description = "The cluster primary security group ID created by the EKS cluster"
  value       = aws_eks_cluster.main.vpc_config[0].cluster_security_group_id
}

output "cluster_service_cidr" {
  description = "The CIDR block that Kubernetes pod and service IP addresses are assigned from"
  value       = aws_eks_cluster.main.kubernetes_network_config[0].service_ipv4_cidr
}

# EKS Node Group Outputs
output "node_groups" {
  description = "EKS node groups"
  value = {
    main = {
      arn           = aws_eks_node_group.main.arn
      status        = aws_eks_node_group.main.status
      capacity_type = aws_eks_node_group.main.capacity_type
      instance_types = aws_eks_node_group.main.instance_types
      scaling_config = aws_eks_node_group.main.scaling_config
    }
  }
}

output "node_security_group_id" {
  description = "ID of the EKS node shared security group"
  value       = aws_security_group.eks_nodes.id
}

output "node_iam_role_name" {
  description = "IAM role name associated with EKS node group"
  value       = aws_iam_role.eks_nodes.name
}

output "node_iam_role_arn" {
  description = "IAM role ARN associated with EKS node group"
  value       = aws_iam_role.eks_nodes.arn
}

# Database Outputs
output "db_instance_endpoint" {
  description = "RDS instance endpoint"
  value       = aws_db_instance.main.endpoint
  sensitive   = true
}

output "db_instance_name" {
  description = "RDS instance name"
  value       = aws_db_instance.main.db_name
}

output "db_instance_username" {
  description = "RDS instance root username"
  value       = aws_db_instance.main.username
  sensitive   = true
}

output "db_instance_port" {
  description = "RDS instance port"
  value       = aws_db_instance.main.port
}

output "db_subnet_group_id" {
  description = "RDS subnet group name"
  value       = aws_db_subnet_group.main.id
}

output "db_subnet_group_arn" {
  description = "RDS subnet group ARN"
  value       = aws_db_subnet_group.main.arn
}

output "db_instance_resource_id" {
  description = "RDS instance resource ID"
  value       = aws_db_instance.main.resource_id
}

output "db_instance_status" {
  description = "RDS instance status"
  value       = aws_db_instance.main.status
}

output "db_instance_availability_zone" {
  description = "RDS instance availability zone"
  value       = aws_db_instance.main.availability_zone
}

output "db_instance_backup_retention_period" {
  description = "RDS instance backup retention period"
  value       = aws_db_instance.main.backup_retention_period
}

# Redis Outputs
output "redis_cluster_id" {
  description = "ElastiCache Redis cluster identifier"
  value       = aws_elasticache_replication_group.main.id
}

output "redis_primary_endpoint_address" {
  description = "Address of the endpoint for the primary node in the replication group"
  value       = aws_elasticache_replication_group.main.primary_endpoint_address
  sensitive   = true
}

output "redis_reader_endpoint_address" {
  description = "Address of the endpoint for the reader node in the replication group"
  value       = aws_elasticache_replication_group.main.reader_endpoint_address
  sensitive   = true
}

output "redis_port" {
  description = "Redis port"
  value       = aws_elasticache_replication_group.main.port
}

output "redis_subnet_group_name" {
  description = "ElastiCache subnet group name"
  value       = aws_elasticache_subnet_group.main.name
}

# S3 Outputs
output "s3_bucket_id" {
  description = "S3 bucket ID for application data"
  value       = aws_s3_bucket.app_data.id
}

output "s3_bucket_arn" {
  description = "S3 bucket ARN for application data"
  value       = aws_s3_bucket.app_data.arn
}

output "s3_bucket_domain_name" {
  description = "S3 bucket domain name"
  value       = aws_s3_bucket.app_data.bucket_domain_name
}

output "s3_bucket_regional_domain_name" {
  description = "S3 bucket region-specific domain name"
  value       = aws_s3_bucket.app_data.bucket_regional_domain_name
}

output "alb_logs_bucket_id" {
  description = "S3 bucket ID for ALB logs"
  value       = aws_s3_bucket.alb_logs.id
}

output "alb_logs_bucket_arn" {
  description = "S3 bucket ARN for ALB logs"
  value       = aws_s3_bucket.alb_logs.arn
}

# Load Balancer Outputs
output "alb_id" {
  description = "Application Load Balancer ID"
  value       = aws_lb.main.id
}

output "alb_arn" {
  description = "Application Load Balancer ARN"
  value       = aws_lb.main.arn
}

output "alb_dns_name" {
  description = "Application Load Balancer DNS name"
  value       = aws_lb.main.dns_name
}

output "alb_zone_id" {
  description = "Application Load Balancer zone ID"
  value       = aws_lb.main.zone_id
}

output "alb_security_group_id" {
  description = "Application Load Balancer security group ID"
  value       = aws_security_group.alb.id
}

# Security Group Outputs
output "security_groups" {
  description = "Security groups created"
  value = {
    eks_cluster = aws_security_group.eks_cluster.id
    eks_nodes   = aws_security_group.eks_nodes.id
    rds         = aws_security_group.rds.id
    redis       = aws_security_group.redis.id
    alb         = aws_security_group.alb.id
  }
}

# KMS Key Outputs
output "kms_key_ids" {
  description = "KMS Key IDs"
  value = {
    eks        = aws_kms_key.eks.id
    rds        = aws_kms_key.rds.id
    s3         = aws_kms_key.s3.id
    cloudwatch = aws_kms_key.cloudwatch.id
    secrets    = aws_kms_key.secrets.id
  }
}

output "kms_key_arns" {
  description = "KMS Key ARNs"
  value = {
    eks        = aws_kms_key.eks.arn
    rds        = aws_kms_key.rds.arn
    s3         = aws_kms_key.s3.arn
    cloudwatch = aws_kms_key.cloudwatch.arn
    secrets    = aws_kms_key.secrets.arn
  }
}

# Secrets Manager Outputs
output "secrets_manager_secret_id" {
  description = "Secrets Manager secret ID"
  value       = aws_secretsmanager_secret.app_secrets.id
}

output "secrets_manager_secret_arn" {
  description = "Secrets Manager secret ARN"
  value       = aws_secretsmanager_secret.app_secrets.arn
}

# CloudWatch Outputs
output "cloudwatch_log_group_name" {
  description = "CloudWatch log group name for EKS cluster"
  value       = aws_cloudwatch_log_group.eks_cluster.name
}

output "cloudwatch_log_group_arn" {
  description = "CloudWatch log group ARN for EKS cluster"
  value       = aws_cloudwatch_log_group.eks_cluster.arn
}

# IAM Role Outputs
output "iam_roles" {
  description = "IAM roles created"
  value = {
    eks_cluster     = aws_iam_role.eks_cluster.arn
    eks_nodes       = aws_iam_role.eks_nodes.arn
    rds_monitoring  = aws_iam_role.rds_monitoring.arn
  }
}

# Region and Account Information
output "aws_region" {
  description = "AWS region"
  value       = var.aws_region
}

output "aws_account_id" {
  description = "AWS account ID"
  value       = data.aws_caller_identity.current.account_id
}

# Kubernetes Configuration
output "kubeconfig" {
  description = "kubectl config as generated by the module"
  value = {
    apiVersion      = "v1"
    kind            = "Config"
    current_context = "terraform"
    contexts = [{
      name = "terraform"
      context = {
        cluster = "terraform"
        user    = "terraform"
      }
    }]
    clusters = [{
      name = "terraform"
      cluster = {
        certificate_authority_data = aws_eks_cluster.main.certificate_authority[0].data
        server                     = aws_eks_cluster.main.endpoint
      }
    }]
    users = [{
      name = "terraform"
      user = {
        exec = {
          apiVersion = "client.authentication.k8s.io/v1beta1"
          command    = "aws"
          args = [
            "eks",
            "get-token",
            "--cluster-name",
            aws_eks_cluster.main.name,
            "--region",
            var.aws_region,
          ]
        }
      }
    }]
  }
  sensitive = true
}

# Connection Strings (Sensitive)
output "database_url" {
  description = "Database connection URL"
  value       = "postgresql://${aws_db_instance.main.username}:${random_password.db_password.result}@${aws_db_instance.main.endpoint}/${aws_db_instance.main.db_name}"
  sensitive   = true
}

output "redis_url" {
  description = "Redis connection URL"
  value       = "redis://:${random_password.redis_auth_token.result}@${aws_elasticache_replication_group.main.primary_endpoint_address}:6379"
  sensitive   = true
}

# Application Configuration
output "app_config" {
  description = "Application configuration values"
  value = {
    environment = var.environment
    region      = var.aws_region
    vpc_id      = aws_vpc.main.id
    cluster_name = aws_eks_cluster.main.name
    namespace   = "wifi-densepose"
  }
}

# Monitoring Configuration
output "monitoring_config" {
  description = "Monitoring configuration"
  value = {
    log_group_name = aws_cloudwatch_log_group.eks_cluster.name
    log_retention  = var.log_retention_days
    kms_key_id     = aws_kms_key.cloudwatch.id
  }
}

# Network Configuration Summary
output "network_config" {
  description = "Network configuration summary"
  value = {
    vpc_id              = aws_vpc.main.id
    vpc_cidr            = aws_vpc.main.cidr_block
    public_subnets      = aws_subnet.public[*].id
    private_subnets     = aws_subnet.private[*].id
    availability_zones  = aws_subnet.public[*].availability_zone
    nat_gateways        = aws_nat_gateway.main[*].id
    internet_gateway    = aws_internet_gateway.main.id
  }
}

# Security Configuration Summary
output "security_config" {
  description = "Security configuration summary"
  value = {
    kms_keys = {
      eks        = aws_kms_key.eks.arn
      rds        = aws_kms_key.rds.arn
      s3         = aws_kms_key.s3.arn
      cloudwatch = aws_kms_key.cloudwatch.arn
      secrets    = aws_kms_key.secrets.arn
    }
    security_groups = {
      eks_cluster = aws_security_group.eks_cluster.id
      eks_nodes   = aws_security_group.eks_nodes.id
      rds         = aws_security_group.rds.id
      redis       = aws_security_group.redis.id
      alb         = aws_security_group.alb.id
    }
    secrets_manager = aws_secretsmanager_secret.app_secrets.arn
  }
}

# Resource Tags
output "common_tags" {
  description = "Common tags applied to resources"
  value = {
    Project     = var.project_name
    Environment = var.environment
    ManagedBy   = "Terraform"
    Owner       = var.owner
  }
}

# Deployment Information
output "deployment_info" {
  description = "Deployment information"
  value = {
    timestamp    = timestamp()
    terraform_version = ">=1.0"
    aws_region   = var.aws_region
    environment  = var.environment
    project_name = var.project_name
  }
}