#!/bin/bash
# =============================================================================
# RuvBot - Google Cloud Platform Deployment Script
# =============================================================================
# Quick deployment for RuvBot to Google Cloud Run
#
# Usage:
#   ./deploy.sh [options]
#
# Options:
#   --project-id ID     GCP Project ID (required)
#   --region REGION     GCP Region (default: us-central1)
#   --env ENV           Environment: dev, staging, prod (default: prod)
#   --no-sql            Skip Cloud SQL setup (use in-memory)
#   --terraform         Use Terraform instead of gcloud
#   --destroy           Destroy all resources
#
# Environment Variables:
#   ANTHROPIC_API_KEY   Required: Anthropic API key
#   OPENROUTER_API_KEY  Optional: OpenRouter API key
# =============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Defaults
REGION="us-central1"
ENVIRONMENT="prod"
USE_TERRAFORM=false
ENABLE_SQL=true
DESTROY=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --project-id)
            PROJECT_ID="$2"
            shift 2
            ;;
        --region)
            REGION="$2"
            shift 2
            ;;
        --env)
            ENVIRONMENT="$2"
            shift 2
            ;;
        --no-sql)
            ENABLE_SQL=false
            shift
            ;;
        --terraform)
            USE_TERRAFORM=true
            shift
            ;;
        --destroy)
            DESTROY=true
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Validate required variables
if [ -z "$PROJECT_ID" ]; then
    echo -e "${RED}Error: --project-id is required${NC}"
    exit 1
fi

if [ -z "$ANTHROPIC_API_KEY" ]; then
    echo -e "${RED}Error: ANTHROPIC_API_KEY environment variable is required${NC}"
    exit 1
fi

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}  RuvBot GCP Deployment${NC}"
echo -e "${BLUE}================================================${NC}"
echo -e "Project:     ${GREEN}$PROJECT_ID${NC}"
echo -e "Region:      ${GREEN}$REGION${NC}"
echo -e "Environment: ${GREEN}$ENVIRONMENT${NC}"
echo -e "Cloud SQL:   ${GREEN}$ENABLE_SQL${NC}"
echo -e "Method:      ${GREEN}$([ "$USE_TERRAFORM" = true ] && echo "Terraform" || echo "gcloud")${NC}"
echo -e "${BLUE}================================================${NC}"

# Confirm deployment
read -p "Continue with deployment? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Deployment cancelled."
    exit 0
fi

# -----------------------------------------------------------------------------
# Terraform Deployment
# -----------------------------------------------------------------------------
if [ "$USE_TERRAFORM" = true ]; then
    cd "$(dirname "$0")/terraform"

    if [ "$DESTROY" = true ]; then
        echo -e "${YELLOW}Destroying infrastructure...${NC}"
        terraform destroy \
            -var="project_id=$PROJECT_ID" \
            -var="region=$REGION" \
            -var="environment=$ENVIRONMENT" \
            -var="enable_cloud_sql=$ENABLE_SQL" \
            -var="anthropic_api_key=$ANTHROPIC_API_KEY" \
            -var="openrouter_api_key=${OPENROUTER_API_KEY:-}"
        exit 0
    fi

    echo -e "${YELLOW}Initializing Terraform...${NC}"
    terraform init

    echo -e "${YELLOW}Planning deployment...${NC}"
    terraform plan \
        -var="project_id=$PROJECT_ID" \
        -var="region=$REGION" \
        -var="environment=$ENVIRONMENT" \
        -var="enable_cloud_sql=$ENABLE_SQL" \
        -var="anthropic_api_key=$ANTHROPIC_API_KEY" \
        -var="openrouter_api_key=${OPENROUTER_API_KEY:-}" \
        -out=tfplan

    echo -e "${YELLOW}Applying deployment...${NC}"
    terraform apply tfplan

    echo -e "${GREEN}Deployment complete!${NC}"
    terraform output
    exit 0
fi

# -----------------------------------------------------------------------------
# gcloud Deployment
# -----------------------------------------------------------------------------

echo -e "${YELLOW}Setting project...${NC}"
gcloud config set project "$PROJECT_ID"

echo -e "${YELLOW}Enabling APIs...${NC}"
gcloud services enable \
    run.googleapis.com \
    cloudbuild.googleapis.com \
    secretmanager.googleapis.com \
    sqladmin.googleapis.com \
    storage.googleapis.com

# Create secrets
echo -e "${YELLOW}Creating secrets...${NC}"
echo -n "$ANTHROPIC_API_KEY" | gcloud secrets create anthropic-api-key \
    --data-file=- --replication-policy=automatic 2>/dev/null || \
    echo -n "$ANTHROPIC_API_KEY" | gcloud secrets versions add anthropic-api-key --data-file=-

if [ -n "$OPENROUTER_API_KEY" ]; then
    echo -n "$OPENROUTER_API_KEY" | gcloud secrets create openrouter-api-key \
        --data-file=- --replication-policy=automatic 2>/dev/null || \
        echo -n "$OPENROUTER_API_KEY" | gcloud secrets versions add openrouter-api-key --data-file=-
fi

# Create service account
echo -e "${YELLOW}Creating service account...${NC}"
gcloud iam service-accounts create ruvbot-runner \
    --display-name="RuvBot Cloud Run" 2>/dev/null || true

SA_EMAIL="ruvbot-runner@$PROJECT_ID.iam.gserviceaccount.com"

gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/secretmanager.secretAccessor" --quiet

gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/cloudsql.client" --quiet

# Create Cloud SQL (if enabled)
if [ "$ENABLE_SQL" = true ]; then
    echo -e "${YELLOW}Creating Cloud SQL instance...${NC}"

    if ! gcloud sql instances describe "ruvbot-$ENVIRONMENT" --quiet 2>/dev/null; then
        gcloud sql instances create "ruvbot-$ENVIRONMENT" \
            --database-version=POSTGRES_16 \
            --tier=db-f1-micro \
            --region="$REGION" \
            --storage-size=10GB \
            --storage-auto-increase \
            --availability-type=zonal

        # Generate password
        DB_PASSWORD=$(openssl rand -base64 24 | tr -d '/+=' | head -c 24)

        # Create database and user
        gcloud sql databases create ruvbot --instance="ruvbot-$ENVIRONMENT"
        gcloud sql users create ruvbot --instance="ruvbot-$ENVIRONMENT" --password="$DB_PASSWORD"

        # Get instance IP
        INSTANCE_IP=$(gcloud sql instances describe "ruvbot-$ENVIRONMENT" --format='get(ipAddresses[0].ipAddress)')

        # Store connection string in secrets
        DATABASE_URL="postgresql://ruvbot:$DB_PASSWORD@$INSTANCE_IP:5432/ruvbot"
        echo -n "$DATABASE_URL" | gcloud secrets create database-url \
            --data-file=- --replication-policy=automatic 2>/dev/null || \
            echo -n "$DATABASE_URL" | gcloud secrets versions add database-url --data-file=-
    fi
fi

# Build and push image
echo -e "${YELLOW}Building container image...${NC}"
cd "$(dirname "$0")/../.."
gcloud builds submit --tag "gcr.io/$PROJECT_ID/ruvbot:latest" .

# Deploy to Cloud Run
echo -e "${YELLOW}Deploying to Cloud Run...${NC}"
SECRETS="ANTHROPIC_API_KEY=anthropic-api-key:latest"
if [ -n "$OPENROUTER_API_KEY" ]; then
    SECRETS="$SECRETS,OPENROUTER_API_KEY=openrouter-api-key:latest"
fi
if [ "$ENABLE_SQL" = true ]; then
    SECRETS="$SECRETS,DATABASE_URL=database-url:latest"
fi

gcloud run deploy ruvbot \
    --image="gcr.io/$PROJECT_ID/ruvbot:latest" \
    --region="$REGION" \
    --platform=managed \
    --allow-unauthenticated \
    --port=8080 \
    --memory=512Mi \
    --cpu=1 \
    --min-instances=0 \
    --max-instances=10 \
    --timeout=300 \
    --concurrency=80 \
    --set-env-vars="NODE_ENV=production" \
    --set-secrets="$SECRETS" \
    --service-account="$SA_EMAIL"

# Get URL
SERVICE_URL=$(gcloud run services describe ruvbot --region="$REGION" --format='get(status.url)')

echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Deployment Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo -e "Service URL: ${BLUE}$SERVICE_URL${NC}"
echo -e "Health Check: ${BLUE}$SERVICE_URL/health${NC}"
echo ""
echo -e "${YELLOW}Estimated Monthly Cost:${NC}"
echo "  - Cloud Run: ~\$0 (free tier)"
if [ "$ENABLE_SQL" = true ]; then
    echo "  - Cloud SQL: ~\$10-15/month"
fi
echo "  - Secrets: ~\$0.18/month"
echo "  - Total: ~\$$([ "$ENABLE_SQL" = true ] && echo "15-20" || echo "5")/month"
echo ""
echo -e "${GREEN}Done!${NC}"
