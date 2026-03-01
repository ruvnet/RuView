/**
 * RuvBot CLI - Deploy Command
 *
 * Deploy RuvBot to various cloud platforms with interactive wizards.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import { execSync, spawn } from 'child_process';

export function createDeploymentCommand(): Command {
  const deploy = new Command('deploy-cloud')
    .alias('cloud')
    .description('Deploy RuvBot to cloud platforms');

  // Cloud Run deployment
  deploy
    .command('cloudrun')
    .alias('gcp')
    .description('Deploy to Google Cloud Run')
    .option('--project <project>', 'GCP project ID')
    .option('--region <region>', 'Cloud Run region', 'us-central1')
    .option('--service <name>', 'Service name', 'ruvbot')
    .option('--memory <size>', 'Memory allocation', '512Mi')
    .option('--min-instances <n>', 'Minimum instances', '0')
    .option('--max-instances <n>', 'Maximum instances', '10')
    .option('--env-file <path>', 'Path to .env file')
    .option('--yes', 'Skip confirmation prompts')
    .action(async (options) => {
      await deployToCloudRun(options);
    });

  // Docker deployment
  deploy
    .command('docker')
    .description('Deploy with Docker/Docker Compose')
    .option('--name <name>', 'Container name', 'ruvbot')
    .option('--port <port>', 'Host port', '3000')
    .option('--detach', 'Run in background', true)
    .option('--env-file <path>', 'Path to .env file')
    .action(async (options) => {
      await deployToDocker(options);
    });

  // Kubernetes deployment
  deploy
    .command('k8s')
    .alias('kubernetes')
    .description('Deploy to Kubernetes cluster')
    .option('--namespace <ns>', 'Kubernetes namespace', 'default')
    .option('--replicas <n>', 'Number of replicas', '2')
    .option('--env-file <path>', 'Path to .env file')
    .action(async (options) => {
      await deployToK8s(options);
    });

  // Deployment wizard
  deploy
    .command('wizard')
    .description('Interactive deployment wizard')
    .action(async () => {
      await runDeploymentWizard();
    });

  // Status check
  deploy
    .command('status')
    .description('Check deployment status')
    .option('--platform <platform>', 'Platform: cloudrun, docker, k8s')
    .action(async (options) => {
      await checkDeploymentStatus(options);
    });

  return deploy;
}

async function deployToCloudRun(options: Record<string, unknown>): Promise<void> {
  console.log(chalk.bold('\n☁️  Google Cloud Run Deployment\n'));
  console.log('═'.repeat(50));

  // Check gcloud
  if (!commandExists('gcloud')) {
    console.error(chalk.red('\n✗ gcloud CLI is required'));
    console.log(chalk.gray('  Install from: https://cloud.google.com/sdk'));
    process.exit(1);
  }

  const spinner = ora('Checking gcloud authentication...').start();

  try {
    // Check authentication
    const account = execSync('gcloud auth list --filter=status:ACTIVE --format="value(account)"', {
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();

    if (!account) {
      spinner.fail('Not authenticated with gcloud');
      console.log(chalk.yellow('\nRun: gcloud auth login'));
      process.exit(1);
    }

    spinner.succeed(`Authenticated as ${account}`);

    // Get or prompt for project
    let projectId = options.project as string;
    if (!projectId) {
      projectId = execSync('gcloud config get-value project', {
        encoding: 'utf-8',
        stdio: ['pipe', 'pipe', 'pipe'],
      }).trim();

      if (!projectId) {
        console.error(chalk.red('\n✗ No project ID specified'));
        console.log(chalk.gray('  Use --project <id> or run: gcloud config set project <id>'));
        process.exit(1);
      }
    }

    console.log(chalk.cyan(`  Project: ${projectId}`));
    console.log(chalk.cyan(`  Region:  ${options.region}`));
    console.log(chalk.cyan(`  Service: ${options.service}`));

    // Enable APIs
    spinner.start('Enabling required APIs...');
    execSync('gcloud services enable run.googleapis.com containerregistry.googleapis.com cloudbuild.googleapis.com', {
      stdio: 'pipe',
    });
    spinner.succeed('APIs enabled');

    // Build environment variables
    let envVars = '';
    if (options.envFile) {
      const fs = await import('fs/promises');
      const envContent = await fs.readFile(options.envFile as string, 'utf-8');
      const vars = envContent
        .split('\n')
        .filter((line) => line.trim() && !line.startsWith('#'))
        .map((line) => line.trim())
        .join(',');
      envVars = `--set-env-vars="${vars}"`;
    }

    // Check for Dockerfile
    const fs = await import('fs/promises');
    let hasDockerfile = false;
    try {
      await fs.access('Dockerfile');
      hasDockerfile = true;
    } catch {
      // Create Dockerfile
      spinner.start('Creating Dockerfile...');
      await fs.writeFile(
        'Dockerfile',
        `FROM node:20-slim
WORKDIR /app
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
RUN npm install -g ruvbot
RUN mkdir -p /app/data /app/plugins /app/skills
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \\
  CMD curl -f http://localhost:\${PORT:-8080}/health || exit 1
CMD ["ruvbot", "start", "--port", "8080"]
`
      );
      spinner.succeed('Dockerfile created');
    }

    // Deploy
    spinner.start('Deploying to Cloud Run (this may take a few minutes)...');

    const deployCmd = [
      'gcloud run deploy',
      options.service,
      '--source .',
      '--platform managed',
      `--region ${options.region}`,
      '--allow-unauthenticated',
      '--port 8080',
      `--memory ${options.memory}`,
      `--min-instances ${options.minInstances}`,
      `--max-instances ${options.maxInstances}`,
      envVars,
      '--quiet',
    ]
      .filter(Boolean)
      .join(' ');

    execSync(deployCmd, { stdio: 'inherit' });

    // Get URL
    const serviceUrl = execSync(
      `gcloud run services describe ${options.service} --region ${options.region} --format='value(status.url)'`,
      { encoding: 'utf-8' }
    ).trim();

    console.log('\n' + chalk.green('═'.repeat(50)));
    console.log(chalk.bold.green('🚀 Deployment successful!'));
    console.log(chalk.green('═'.repeat(50)));
    console.log(`\n  URL:      ${chalk.cyan(serviceUrl)}`);
    console.log(`  Health:   ${chalk.cyan(serviceUrl + '/health')}`);
    console.log(`  API:      ${chalk.cyan(serviceUrl + '/api/status')}`);
    console.log(`\n  Test: ${chalk.gray(`curl ${serviceUrl}/health`)}`);
    console.log();
  } catch (error) {
    spinner.fail('Deployment failed');
    console.error(chalk.red(`\nError: ${error instanceof Error ? error.message : 'Unknown error'}`));
    process.exit(1);
  }
}

async function deployToDocker(options: Record<string, unknown>): Promise<void> {
  console.log(chalk.bold('\n🐳 Docker Deployment\n'));
  console.log('═'.repeat(50));

  if (!commandExists('docker')) {
    console.error(chalk.red('\n✗ Docker is required'));
    console.log(chalk.gray('  Install from: https://docker.com'));
    process.exit(1);
  }

  const fs = await import('fs/promises');
  const spinner = ora('Creating docker-compose.yml...').start();

  try {
    const envFileMapping = options.envFile ? `env_file:\n      - ${options.envFile}` : '';

    const composeContent = `version: '3.8'
services:
  ruvbot:
    image: node:20-slim
    container_name: ${options.name}
    working_dir: /app
    command: sh -c "npm install -g ruvbot && ruvbot start --port 3000"
    ports:
      - "${options.port}:3000"
    ${envFileMapping}
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
`;

    await fs.writeFile('docker-compose.yml', composeContent);
    spinner.succeed('docker-compose.yml created');

    // Create directories
    await fs.mkdir('data', { recursive: true });
    await fs.mkdir('plugins', { recursive: true });
    await fs.mkdir('skills', { recursive: true });

    if (options.detach) {
      spinner.start('Starting containers...');
      execSync('docker-compose up -d', { stdio: 'pipe' });
      spinner.succeed('Containers started');

      console.log('\n' + chalk.green('═'.repeat(50)));
      console.log(chalk.bold.green('🚀 RuvBot is running!'));
      console.log(chalk.green('═'.repeat(50)));
      console.log(`\n  URL:      ${chalk.cyan(`http://localhost:${options.port}`)}`);
      console.log(`  Health:   ${chalk.cyan(`http://localhost:${options.port}/health`)}`);
      console.log(`\n  Logs:     ${chalk.gray('docker-compose logs -f')}`);
      console.log(`  Stop:     ${chalk.gray('docker-compose down')}`);
      console.log();
    } else {
      console.log(chalk.cyan('\nRun: docker-compose up'));
    }
  } catch (error) {
    spinner.fail('Docker deployment failed');
    console.error(chalk.red(`\nError: ${error instanceof Error ? error.message : 'Unknown error'}`));
    process.exit(1);
  }
}

async function deployToK8s(options: Record<string, unknown>): Promise<void> {
  console.log(chalk.bold('\n☸️  Kubernetes Deployment\n'));
  console.log('═'.repeat(50));

  if (!commandExists('kubectl')) {
    console.error(chalk.red('\n✗ kubectl is required'));
    console.log(chalk.gray('  Install from: https://kubernetes.io/docs/tasks/tools/'));
    process.exit(1);
  }

  const fs = await import('fs/promises');
  const spinner = ora('Creating Kubernetes manifests...').start();

  try {
    await fs.mkdir('k8s', { recursive: true });

    // Deployment manifest
    const deployment = `apiVersion: apps/v1
kind: Deployment
metadata:
  name: ruvbot
  namespace: ${options.namespace}
spec:
  replicas: ${options.replicas}
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
  namespace: ${options.namespace}
spec:
  selector:
    app: ruvbot
  ports:
  - port: 80
    targetPort: 3000
  type: LoadBalancer
`;

    await fs.writeFile('k8s/deployment.yaml', deployment);

    // Secret template
    const secret = `apiVersion: v1
kind: Secret
metadata:
  name: ruvbot-secrets
  namespace: ${options.namespace}
type: Opaque
stringData:
  OPENROUTER_API_KEY: "YOUR_API_KEY"
  DEFAULT_MODEL: "google/gemini-2.0-flash-001"
`;

    await fs.writeFile('k8s/secret.yaml', secret);

    spinner.succeed('Kubernetes manifests created in k8s/');

    console.log('\n' + chalk.yellow('⚠️  Before applying:'));
    console.log(chalk.gray('   1. Edit k8s/secret.yaml with your API keys'));
    console.log(chalk.gray('   2. Review k8s/deployment.yaml'));
    console.log('\n  Apply with:');
    console.log(chalk.cyan('    kubectl apply -f k8s/'));
    console.log('\n  Check status:');
    console.log(chalk.cyan('    kubectl get pods -l app=ruvbot'));
    console.log();
  } catch (error) {
    spinner.fail('Kubernetes manifest creation failed');
    console.error(chalk.red(`\nError: ${error instanceof Error ? error.message : 'Unknown error'}`));
    process.exit(1);
  }
}

async function runDeploymentWizard(): Promise<void> {
  console.log(chalk.bold('\n🧙 RuvBot Deployment Wizard\n'));
  console.log('═'.repeat(50));

  // This would use inquirer or similar for interactive prompts
  // For now, provide instructions
  console.log('\nSelect a deployment target:\n');
  console.log('  1. Google Cloud Run (serverless, auto-scaling)');
  console.log('     ' + chalk.cyan('ruvbot deploy-cloud cloudrun'));
  console.log();
  console.log('  2. Docker (local or server deployment)');
  console.log('     ' + chalk.cyan('ruvbot deploy-cloud docker'));
  console.log();
  console.log('  3. Kubernetes (production cluster)');
  console.log('     ' + chalk.cyan('ruvbot deploy-cloud k8s'));
  console.log();
  console.log('For interactive setup, use the install script:');
  console.log(chalk.cyan('  RUVBOT_WIZARD=true curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/npm/packages/ruvbot/scripts/install.sh | bash'));
  console.log();
}

async function checkDeploymentStatus(options: Record<string, unknown>): Promise<void> {
  const platform = options.platform as string;

  console.log(chalk.bold('\n📊 Deployment Status\n'));

  if (!platform || platform === 'cloudrun') {
    console.log(chalk.cyan('Cloud Run:'));
    if (commandExists('gcloud')) {
      try {
        const services = execSync(
          'gcloud run services list --format="table(metadata.name,status.url,status.conditions[0].status)" 2>/dev/null',
          { encoding: 'utf-8' }
        );
        console.log(services || '  No services found');
      } catch {
        console.log(chalk.gray('  Not configured or no services'));
      }
    } else {
      console.log(chalk.gray('  gcloud CLI not installed'));
    }
    console.log();
  }

  if (!platform || platform === 'docker') {
    console.log(chalk.cyan('Docker:'));
    if (commandExists('docker')) {
      try {
        const containers = execSync(
          'docker ps --filter "name=ruvbot" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" 2>/dev/null',
          { encoding: 'utf-8' }
        );
        console.log(containers || '  No containers running');
      } catch {
        console.log(chalk.gray('  No containers found'));
      }
    } else {
      console.log(chalk.gray('  Docker not installed'));
    }
    console.log();
  }

  if (!platform || platform === 'k8s') {
    console.log(chalk.cyan('Kubernetes:'));
    if (commandExists('kubectl')) {
      try {
        const pods = execSync(
          'kubectl get pods -l app=ruvbot -o wide 2>/dev/null',
          { encoding: 'utf-8' }
        );
        console.log(pods || '  No pods found');
      } catch {
        console.log(chalk.gray('  No pods found or not configured'));
      }
    } else {
      console.log(chalk.gray('  kubectl not installed'));
    }
    console.log();
  }
}

function commandExists(cmd: string): boolean {
  try {
    execSync(`which ${cmd}`, { stdio: 'pipe' });
    return true;
  } catch {
    return false;
  }
}

export default createDeploymentCommand;
