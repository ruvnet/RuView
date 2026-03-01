#!/bin/bash
# Deploy Edge-Net Relay to Google Cloud Run

set -e

PROJECT_ID="${GCP_PROJECT:-$(gcloud config get-value project)}"
REGION="${GCP_REGION:-us-central1}"
SERVICE_NAME="edge-net-relay"

echo "🚀 Deploying Edge-Net Relay to Cloud Run"
echo "   Project: $PROJECT_ID"
echo "   Region: $REGION"

# Enable required APIs
echo "📦 Enabling Cloud Run API..."
gcloud services enable run.googleapis.com --project=$PROJECT_ID

# Build and deploy
echo "🏗️ Building and deploying..."
gcloud run deploy $SERVICE_NAME \
  --source . \
  --project=$PROJECT_ID \
  --region=$REGION \
  --platform=managed \
  --allow-unauthenticated \
  --memory=256Mi \
  --cpu=1 \
  --min-instances=1 \
  --max-instances=10 \
  --timeout=3600 \
  --session-affinity

# Get URL
URL=$(gcloud run services describe $SERVICE_NAME --region=$REGION --project=$PROJECT_ID --format='value(status.url)')

echo ""
echo "✅ Edge-Net Relay deployed successfully!"
echo "🌐 Relay URL: $URL"
echo ""
echo "📝 Add this to your dashboard's edgeNet.ts:"
echo ""
echo "   const config = new this.module.EdgeNetConfig(id)"
echo "     .addRelay('${URL/https/wss}')"
echo "     .cpuLimit(0.5)"
echo "     .build();"
