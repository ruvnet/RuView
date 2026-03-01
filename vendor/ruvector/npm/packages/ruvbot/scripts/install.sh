#!/bin/bash
#
# RuvBot Installer
#
# Usage:
#   curl -fsSL https://get.ruvector.dev/ruvbot | bash
#   curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/npm/packages/ruvbot/scripts/install.sh | bash
#
# Options (via environment variables):
#   RUVBOT_VERSION     - Specific version to install (default: latest)
#   RUVBOT_GLOBAL      - Install globally (default: true)
#   RUVBOT_INIT        - Run init after install (default: false)
#   RUVBOT_CHANNEL     - Configure channel: slack, discord, telegram
#   RUVBOT_DEPLOY      - Deploy target: local, docker, cloudrun, k8s
#   RUVBOT_WIZARD      - Run interactive wizard (default: false)
#
# Examples:
#   # Basic install
#   curl -fsSL https://get.ruvector.dev/ruvbot | bash
#
#   # Install specific version
#   RUVBOT_VERSION=0.1.3 curl -fsSL https://get.ruvector.dev/ruvbot | bash
#
#   # Install and initialize
#   RUVBOT_INIT=true curl -fsSL https://get.ruvector.dev/ruvbot | bash
#
#   # Install with Slack configuration
#   RUVBOT_CHANNEL=slack curl -fsSL https://get.ruvector.dev/ruvbot | bash
#
#   # Install and deploy to Cloud Run
#   RUVBOT_DEPLOY=cloudrun curl -fsSL https://get.ruvector.dev/ruvbot | bash
#
#   # Run full interactive wizard
#   RUVBOT_WIZARD=true curl -fsSL https://get.ruvector.dev/ruvbot | bash

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color
BOLD='\033[1m'
DIM='\033[2m'

# Configuration
RUVBOT_VERSION="${RUVBOT_VERSION:-latest}"
RUVBOT_GLOBAL="${RUVBOT_GLOBAL:-true}"
RUVBOT_INIT="${RUVBOT_INIT:-false}"
RUVBOT_CHANNEL="${RUVBOT_CHANNEL:-}"
RUVBOT_DEPLOY="${RUVBOT_DEPLOY:-}"
RUVBOT_WIZARD="${RUVBOT_WIZARD:-false}"

# Feature flags
GCLOUD_AVAILABLE=false
DOCKER_AVAILABLE=false
KUBECTL_AVAILABLE=false

# Banner
print_banner() {
  echo -e "${CYAN}"
  echo '  ____             ____        _   '
  echo ' |  _ \ _   ___   _| __ )  ___ | |_ '
  echo ' | |_) | | | \ \ / /  _ \ / _ \| __|'
  echo ' |  _ <| |_| |\ V /| |_) | (_) | |_ '
  echo ' |_| \_\\__,_| \_/ |____/ \___/ \__|'
  echo -e "${NC}"
  echo -e "${BOLD}Enterprise-Grade Self-Learning AI Assistant${NC}"
  echo -e "${DIM}Military-strength security • 150x faster search • 12+ LLM models${NC}"
  echo ""
}

# Logging functions
info() { echo -e "${BLUE}ℹ${NC} $1"; }
success() { echo -e "${GREEN}✓${NC} $1"; }
warn() { echo -e "${YELLOW}⚠${NC} $1"; }
error() { echo -e "${RED}✗${NC} $1"; exit 1; }
step() { echo -e "\n${MAGENTA}▸${NC} ${BOLD}$1${NC}"; }

# Check dependencies
check_dependencies() {
  step "Checking dependencies"

  # Check Node.js
  if ! command -v node &> /dev/null; then
    error "Node.js is required but not installed. Install from https://nodejs.org"
  fi

  NODE_VERSION=$(node -v | cut -d 'v' -f 2 | cut -d '.' -f 1)
  if [ "$NODE_VERSION" -lt 18 ]; then
    error "Node.js 18+ is required. Current: $(node -v)"
  fi
  success "Node.js $(node -v)"

  # Check npm
  if ! command -v npm &> /dev/null; then
    error "npm is required but not installed"
  fi
  success "npm $(npm -v)"

  # Check optional: gcloud
  if command -v gcloud &> /dev/null; then
    success "gcloud CLI $(gcloud --version 2>/dev/null | head -1 | awk '{print $4}')"
    GCLOUD_AVAILABLE=true
  else
    echo -e "${DIM}  ○ gcloud CLI not found (optional for Cloud Run)${NC}"
  fi

  # Check optional: docker
  if command -v docker &> /dev/null; then
    success "Docker $(docker --version | awk '{print $3}' | tr -d ',')"
    DOCKER_AVAILABLE=true
  else
    echo -e "${DIM}  ○ Docker not found (optional for containerization)${NC}"
  fi

  # Check optional: kubectl
  if command -v kubectl &> /dev/null; then
    success "kubectl $(kubectl version --client -o json 2>/dev/null | grep -o '"gitVersion": "[^"]*"' | cut -d'"' -f4)"
    KUBECTL_AVAILABLE=true
  else
    echo -e "${DIM}  ○ kubectl not found (optional for Kubernetes)${NC}"
  fi
}

# Install RuvBot
install_ruvbot() {
  step "Installing RuvBot"

  PACKAGE="ruvbot"
  if [ "$RUVBOT_VERSION" != "latest" ]; then
    PACKAGE="ruvbot@$RUVBOT_VERSION"
    info "Installing version $RUVBOT_VERSION"
  fi

  if [ "$RUVBOT_GLOBAL" = "true" ]; then
    npm install -g "$PACKAGE" 2>/dev/null || sudo npm install -g "$PACKAGE"
    success "RuvBot installed globally"
  else
    npm install "$PACKAGE"
    success "RuvBot installed locally"
  fi

  # Verify installation
  if command -v ruvbot &> /dev/null; then
    INSTALLED_VERSION=$(ruvbot --version 2>/dev/null || echo "unknown")
    success "RuvBot $INSTALLED_VERSION is ready"
  else
    success "RuvBot installed (use 'npx ruvbot' to run)"
  fi
}

# Install optional dependencies for channels
install_channel_deps() {
  local channel=$1
  step "Installing $channel dependencies"

  case "$channel" in
    slack)
      npm install @slack/bolt @slack/web-api 2>/dev/null
      success "Slack SDK installed (@slack/bolt, @slack/web-api)"
      ;;
    discord)
      npm install discord.js 2>/dev/null
      success "Discord.js installed"
      ;;
    telegram)
      npm install telegraf 2>/dev/null
      success "Telegraf installed"
      ;;
    all)
      npm install @slack/bolt @slack/web-api discord.js telegraf 2>/dev/null
      success "All channel dependencies installed"
      ;;
  esac
}

# Initialize project
init_project() {
  step "Initializing RuvBot project"

  if [ "$RUVBOT_GLOBAL" = "true" ]; then
    ruvbot init --yes
  else
    npx ruvbot init --yes
  fi

  success "Project initialized"
}

# Configure channel interactively
configure_channel() {
  local channel=$1

  step "Configuring $channel"

  case "$channel" in
    slack)
      echo ""
      echo "  To set up Slack, you'll need credentials from:"
      echo -e "  ${CYAN}https://api.slack.com/apps${NC}"
      echo ""
      read -p "  SLACK_BOT_TOKEN (xoxb-...): " SLACK_BOT_TOKEN
      read -p "  SLACK_SIGNING_SECRET: " SLACK_SIGNING_SECRET
      read -p "  SLACK_APP_TOKEN (xapp-...): " SLACK_APP_TOKEN

      {
        echo "SLACK_BOT_TOKEN=$SLACK_BOT_TOKEN"
        echo "SLACK_SIGNING_SECRET=$SLACK_SIGNING_SECRET"
        echo "SLACK_APP_TOKEN=$SLACK_APP_TOKEN"
      } >> .env

      success "Slack configuration saved to .env"
      ;;

    discord)
      echo ""
      echo "  To set up Discord, you'll need credentials from:"
      echo -e "  ${CYAN}https://discord.com/developers/applications${NC}"
      echo ""
      read -p "  DISCORD_TOKEN: " DISCORD_TOKEN
      read -p "  DISCORD_CLIENT_ID: " DISCORD_CLIENT_ID
      read -p "  DISCORD_GUILD_ID (optional): " DISCORD_GUILD_ID

      {
        echo "DISCORD_TOKEN=$DISCORD_TOKEN"
        echo "DISCORD_CLIENT_ID=$DISCORD_CLIENT_ID"
        [ -n "$DISCORD_GUILD_ID" ] && echo "DISCORD_GUILD_ID=$DISCORD_GUILD_ID"
      } >> .env

      success "Discord configuration saved to .env"
      ;;

    telegram)
      echo ""
      echo "  To set up Telegram, get a token from:"
      echo -e "  ${CYAN}@BotFather${NC} on Telegram"
      echo ""
      read -p "  TELEGRAM_BOT_TOKEN: " TELEGRAM_BOT_TOKEN

      echo "TELEGRAM_BOT_TOKEN=$TELEGRAM_BOT_TOKEN" >> .env

      success "Telegram configuration saved to .env"
      ;;
  esac
}

# Deploy to Cloud Run
deploy_cloudrun() {
  step "Deploying to Google Cloud Run"

  if [ "$GCLOUD_AVAILABLE" != "true" ]; then
    error "gcloud CLI is required. Install from https://cloud.google.com/sdk"
  fi

  # Check authentication
  if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" 2>/dev/null | head -1; then
    warn "Not authenticated with gcloud"
    info "Running 'gcloud auth login'..."
    gcloud auth login
  fi

  # Get project
  CURRENT_PROJECT=$(gcloud config get-value project 2>/dev/null || echo "")
  echo ""
  read -p "  GCP Project ID [$CURRENT_PROJECT]: " PROJECT_ID
  PROJECT_ID="${PROJECT_ID:-$CURRENT_PROJECT}"

  if [ -z "$PROJECT_ID" ]; then
    error "Project ID is required"
  fi

  gcloud config set project "$PROJECT_ID" 2>/dev/null

  # Get region
  read -p "  Region [us-central1]: " REGION
  REGION="${REGION:-us-central1}"

  # Get service name
  read -p "  Service name [ruvbot]: " SERVICE_NAME
  SERVICE_NAME="${SERVICE_NAME:-ruvbot}"

  # Get API key
  echo ""
  echo "  LLM Provider:"
  echo "    1. OpenRouter (recommended - Gemini, Claude, GPT)"
  echo "    2. Anthropic (Claude only)"
  read -p "  Choose [1]: " PROVIDER_CHOICE
  PROVIDER_CHOICE="${PROVIDER_CHOICE:-1}"

  if [ "$PROVIDER_CHOICE" = "1" ]; then
    read -p "  OPENROUTER_API_KEY: " API_KEY
    ENV_VARS="OPENROUTER_API_KEY=$API_KEY,DEFAULT_MODEL=google/gemini-2.0-flash-001"
  else
    read -p "  ANTHROPIC_API_KEY: " API_KEY
    ENV_VARS="ANTHROPIC_API_KEY=$API_KEY"
  fi

  # Channel configuration
  echo ""
  read -p "  Configure Slack? [y/N]: " SETUP_SLACK
  if [[ "$SETUP_SLACK" =~ ^[Yy]$ ]]; then
    read -p "    SLACK_BOT_TOKEN: " SLACK_BOT_TOKEN
    read -p "    SLACK_SIGNING_SECRET: " SLACK_SIGNING_SECRET
    ENV_VARS="$ENV_VARS,SLACK_BOT_TOKEN=$SLACK_BOT_TOKEN,SLACK_SIGNING_SECRET=$SLACK_SIGNING_SECRET"
  fi

  read -p "  Configure Telegram? [y/N]: " SETUP_TELEGRAM
  if [[ "$SETUP_TELEGRAM" =~ ^[Yy]$ ]]; then
    read -p "    TELEGRAM_BOT_TOKEN: " TELEGRAM_BOT_TOKEN
    ENV_VARS="$ENV_VARS,TELEGRAM_BOT_TOKEN=$TELEGRAM_BOT_TOKEN"
  fi

  # Enable required APIs
  info "Enabling required GCP APIs..."
  gcloud services enable run.googleapis.com containerregistry.googleapis.com cloudbuild.googleapis.com 2>/dev/null

  # Create Dockerfile if it doesn't exist
  if [ ! -f "Dockerfile" ]; then
    info "Creating Dockerfile..."
    cat > Dockerfile << 'DOCKERFILE'
FROM node:20-slim

WORKDIR /app

# Install curl for health checks
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# Install ruvbot
RUN npm install -g ruvbot

# Create directories
RUN mkdir -p /app/data /app/plugins /app/skills

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:${PORT:-8080}/health || exit 1

# Start command
CMD ["ruvbot", "start", "--port", "8080"]
DOCKERFILE
    success "Dockerfile created"
  fi

  # Deploy
  info "Deploying to Cloud Run (this may take a few minutes)..."
  gcloud run deploy "$SERVICE_NAME" \
    --source . \
    --platform managed \
    --region "$REGION" \
    --allow-unauthenticated \
    --port 8080 \
    --memory 512Mi \
    --min-instances 0 \
    --max-instances 10 \
    --set-env-vars="$ENV_VARS" \
    --quiet

  # Get URL
  SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" --region "$REGION" --format='value(status.url)')

  echo ""
  echo -e "${GREEN}═══════════════════════════════════════${NC}"
  echo -e "${BOLD}🚀 RuvBot deployed successfully!${NC}"
  echo -e "${GREEN}═══════════════════════════════════════${NC}"
  echo ""
  echo -e "  URL:      ${CYAN}$SERVICE_URL${NC}"
  echo -e "  Health:   ${CYAN}$SERVICE_URL/health${NC}"
  echo -e "  API:      ${CYAN}$SERVICE_URL/api/status${NC}"
  echo -e "  Models:   ${CYAN}$SERVICE_URL/api/models${NC}"
  echo ""
  echo "  Quick test:"
  echo -e "    ${DIM}curl $SERVICE_URL/health${NC}"
  echo ""

  # Set Telegram webhook if configured
  if [ -n "$TELEGRAM_BOT_TOKEN" ]; then
    WEBHOOK_URL="$SERVICE_URL/telegram/webhook"
    info "Setting Telegram webhook..."
    curl -s "https://api.telegram.org/bot$TELEGRAM_BOT_TOKEN/setWebhook?url=$WEBHOOK_URL" > /dev/null
    success "Telegram webhook: $WEBHOOK_URL"
  fi
}

# Deploy to Docker
deploy_docker() {
  step "Deploying with Docker"

  if [ "$DOCKER_AVAILABLE" != "true" ]; then
    error "Docker is required. Install from https://docker.com"
  fi

  # Get configuration
  read -p "  Container name [ruvbot]: " CONTAINER_NAME
  CONTAINER_NAME="${CONTAINER_NAME:-ruvbot}"

  read -p "  Port [3000]: " PORT
  PORT="${PORT:-3000}"

  # Create docker-compose.yml
  info "Creating docker-compose.yml..."
  cat > docker-compose.yml << COMPOSE
version: '3.8'
services:
  ruvbot:
    image: node:20-slim
    container_name: $CONTAINER_NAME
    working_dir: /app
    command: sh -c "npm install -g ruvbot && ruvbot start --port 3000"
    ports:
      - "$PORT:3000"
    environment:
      - OPENROUTER_API_KEY=\${OPENROUTER_API_KEY}
      - ANTHROPIC_API_KEY=\${ANTHROPIC_API_KEY}
      - SLACK_BOT_TOKEN=\${SLACK_BOT_TOKEN}
      - SLACK_SIGNING_SECRET=\${SLACK_SIGNING_SECRET}
      - SLACK_APP_TOKEN=\${SLACK_APP_TOKEN}
      - DISCORD_TOKEN=\${DISCORD_TOKEN}
      - DISCORD_CLIENT_ID=\${DISCORD_CLIENT_ID}
      - TELEGRAM_BOT_TOKEN=\${TELEGRAM_BOT_TOKEN}
    volumes:
      - ./data:/app/data
      - ./plugins:/app/plugins
      - ./skills:/app/skills
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    restart: unless-stopped
COMPOSE
  success "docker-compose.yml created"

  # Start containers
  read -p "  Start containers now? [Y/n]: " START_NOW
  START_NOW="${START_NOW:-Y}"

  if [[ "$START_NOW" =~ ^[Yy]$ ]]; then
    info "Starting Docker containers..."
    docker-compose up -d

    echo ""
    echo -e "${GREEN}═══════════════════════════════════════${NC}"
    echo -e "${BOLD}🚀 RuvBot is running!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════${NC}"
    echo ""
    echo -e "  URL:      ${CYAN}http://localhost:$PORT${NC}"
    echo -e "  Health:   ${CYAN}http://localhost:$PORT/health${NC}"
    echo -e "  Logs:     ${DIM}docker-compose logs -f${NC}"
    echo -e "  Stop:     ${DIM}docker-compose down${NC}"
    echo ""
  fi
}

# Deploy to Kubernetes
deploy_k8s() {
  step "Deploying to Kubernetes"

  if [ "$KUBECTL_AVAILABLE" != "true" ]; then
    error "kubectl is required. Install from https://kubernetes.io/docs/tasks/tools/"
  fi

  # Get namespace
  read -p "  Namespace [default]: " NAMESPACE
  NAMESPACE="${NAMESPACE:-default}"

  # Get API key
  read -p "  OPENROUTER_API_KEY: " API_KEY

  info "Creating Kubernetes manifests..."

  mkdir -p k8s

  # Create secret
  cat > k8s/secret.yaml << SECRET
apiVersion: v1
kind: Secret
metadata:
  name: ruvbot-secrets
  namespace: $NAMESPACE
type: Opaque
stringData:
  OPENROUTER_API_KEY: "$API_KEY"
  DEFAULT_MODEL: "google/gemini-2.0-flash-001"
SECRET

  # Create deployment
  cat > k8s/deployment.yaml << DEPLOYMENT
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ruvbot
  namespace: $NAMESPACE
spec:
  replicas: 2
  selector:
    matchLabels:
      app: ruvbot
  template:
    metadata:
      labels:
        app: ruvbot
    spec:
      containers:
      - name: ruvbot
        image: node:20-slim
        command: ["sh", "-c", "npm install -g ruvbot && ruvbot start --port 3000"]
        ports:
        - containerPort: 3000
        envFrom:
        - secretRef:
            name: ruvbot-secrets
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 60
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /ready
            port: 3000
          initialDelaySeconds: 30
          periodSeconds: 10
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: ruvbot
  namespace: $NAMESPACE
spec:
  selector:
    app: ruvbot
  ports:
  - port: 80
    targetPort: 3000
  type: LoadBalancer
DEPLOYMENT

  success "Kubernetes manifests created in k8s/"

  read -p "  Apply manifests now? [Y/n]: " APPLY_NOW
  APPLY_NOW="${APPLY_NOW:-Y}"

  if [[ "$APPLY_NOW" =~ ^[Yy]$ ]]; then
    kubectl apply -f k8s/

    echo ""
    success "Kubernetes resources created"
    echo ""
    echo "  Check status:"
    echo -e "    ${DIM}kubectl get pods -l app=ruvbot${NC}"
    echo ""
    echo "  Get service URL:"
    echo -e "    ${DIM}kubectl get svc ruvbot${NC}"
    echo ""
  fi
}

# Deployment wizard
deployment_wizard() {
  step "Deployment Options"
  echo ""
  echo "  1. Local (development)"
  echo "  2. Docker"
  echo "  3. Google Cloud Run"
  echo "  4. Kubernetes"
  echo "  5. Skip deployment"
  echo ""
  read -p "  Select [5]: " DEPLOY_CHOICE
  DEPLOY_CHOICE="${DEPLOY_CHOICE:-5}"

  case "$DEPLOY_CHOICE" in
    1)
      info "Starting local development server..."
      if [ "$RUVBOT_GLOBAL" = "true" ]; then
        ruvbot start --debug
      else
        npx ruvbot start --debug
      fi
      ;;
    2) deploy_docker ;;
    3) deploy_cloudrun ;;
    4) deploy_k8s ;;
    5) info "Skipping deployment" ;;
    *) warn "Invalid option, skipping deployment" ;;
  esac
}

# Interactive setup wizard
run_wizard() {
  step "RuvBot Setup Wizard"

  # Ensure .env exists
  touch .env 2>/dev/null || true

  # LLM Provider
  echo ""
  echo "  ${BOLD}Step 1: LLM Provider${NC}"
  echo "  ───────────────────"
  echo "    1. OpenRouter (Gemini 2.5, Claude, GPT - recommended)"
  echo "    2. Anthropic (Claude only)"
  echo "    3. Skip (configure later)"
  read -p "  Select [1]: " PROVIDER
  PROVIDER="${PROVIDER:-1}"

  case "$PROVIDER" in
    1)
      read -p "    OPENROUTER_API_KEY: " OPENROUTER_KEY
      {
        echo "OPENROUTER_API_KEY=$OPENROUTER_KEY"
        echo "DEFAULT_MODEL=google/gemini-2.0-flash-001"
      } >> .env
      success "OpenRouter configured"
      ;;
    2)
      read -p "    ANTHROPIC_API_KEY: " ANTHROPIC_KEY
      echo "ANTHROPIC_API_KEY=$ANTHROPIC_KEY" >> .env
      success "Anthropic configured"
      ;;
    3) info "Skipping LLM configuration" ;;
  esac

  # Channel Configuration
  echo ""
  echo "  ${BOLD}Step 2: Channel Integrations${NC}"
  echo "  ────────────────────────────"
  echo "    1. Slack"
  echo "    2. Discord"
  echo "    3. Telegram"
  echo "    4. All channels"
  echo "    5. Skip (configure later)"
  read -p "  Select [5]: " CHANNELS
  CHANNELS="${CHANNELS:-5}"

  case "$CHANNELS" in
    1)
      install_channel_deps "slack"
      configure_channel "slack"
      ;;
    2)
      install_channel_deps "discord"
      configure_channel "discord"
      ;;
    3)
      install_channel_deps "telegram"
      configure_channel "telegram"
      ;;
    4)
      install_channel_deps "all"
      configure_channel "slack"
      configure_channel "discord"
      configure_channel "telegram"
      ;;
    5) info "Skipping channel configuration" ;;
  esac

  # Deployment
  echo ""
  echo "  ${BOLD}Step 3: Deployment${NC}"
  echo "  ──────────────────"
  deployment_wizard
}

# Print next steps
print_next_steps() {
  echo ""
  echo -e "${BOLD}📚 Quick Start${NC}"
  echo "═══════════════════════════════════════"
  echo ""
  echo "  Configure LLM provider:"
  echo -e "    ${CYAN}export OPENROUTER_API_KEY=sk-or-...${NC}"
  echo ""
  echo "  Run diagnostics:"
  echo -e "    ${CYAN}ruvbot doctor${NC}"
  echo ""
  echo "  Start the bot:"
  echo -e "    ${CYAN}ruvbot start${NC}"
  echo ""
  echo "  Channel setup guides:"
  echo -e "    ${CYAN}ruvbot channels setup slack${NC}"
  echo -e "    ${CYAN}ruvbot channels setup discord${NC}"
  echo -e "    ${CYAN}ruvbot channels setup telegram${NC}"
  echo ""
  echo "  Deploy templates:"
  echo -e "    ${CYAN}ruvbot templates list${NC}"
  echo -e "    ${CYAN}ruvbot deploy code-reviewer${NC}"
  echo ""
  echo "  Deploy to Cloud Run:"
  echo -e "    ${CYAN}ruvbot deploy cloudrun${NC}"
  echo ""
  echo -e "${DIM}Docs: https://github.com/ruvnet/ruvector/tree/main/npm/packages/ruvbot${NC}"
  echo ""
}

# Main
main() {
  print_banner
  check_dependencies
  install_ruvbot

  # Handle channel installation
  if [ -n "$RUVBOT_CHANNEL" ]; then
    install_channel_deps "$RUVBOT_CHANNEL"
  fi

  # Handle initialization
  if [ "$RUVBOT_INIT" = "true" ]; then
    init_project
  fi

  # Handle wizard
  if [ "$RUVBOT_WIZARD" = "true" ]; then
    run_wizard
  elif [ -n "$RUVBOT_DEPLOY" ]; then
    # Handle deployment without wizard
    case "$RUVBOT_DEPLOY" in
      cloudrun|cloud-run|gcp) deploy_cloudrun ;;
      docker) deploy_docker ;;
      k8s|kubernetes) deploy_k8s ;;
      *) warn "Unknown deployment target: $RUVBOT_DEPLOY" ;;
    esac
  fi

  print_next_steps
}

main "$@"
