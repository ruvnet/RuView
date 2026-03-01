/**
 * RuVector PostgreSQL Installation Commands
 *
 * Provides complete installation of RuVector PostgreSQL extension:
 * - Full native installation (PostgreSQL + Rust + pgrx + extension)
 * - Docker-based installation (recommended for quick start)
 * - Extension management (enable, disable, upgrade)
 */

import { execSync, spawn, spawnSync } from 'child_process';
import { promisify } from 'util';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import chalk from 'chalk';
import ora from 'ora';

// Constants
const DOCKER_IMAGE = 'ruvnet/ruvector-postgres';
const DOCKER_IMAGE_VERSION = '0.2.5';
const RUVECTOR_CRATE_VERSION = '0.2.5';
const PGRX_VERSION = '0.12.6';
const DEFAULT_PG_VERSION = '16';
const SUPPORTED_PG_VERSIONS = ['14', '15', '16', '17'];
const DEFAULT_PORT = 5432;
const DEFAULT_USER = 'ruvector';
const DEFAULT_PASSWORD = 'ruvector';
const DEFAULT_DB = 'ruvector';

interface InstallOptions {
  method?: 'docker' | 'native' | 'auto';
  port?: number;
  user?: string;
  password?: string;
  database?: string;
  dataDir?: string;
  version?: string;
  pgVersion?: string;
  detach?: boolean;
  name?: string;
  skipPostgres?: boolean;
  skipRust?: boolean;
}

interface StatusInfo {
  installed: boolean;
  running: boolean;
  method: 'docker' | 'native' | 'none';
  version?: string;
  containerId?: string;
  port?: number;
  connectionString?: string;
}

interface SystemInfo {
  platform: NodeJS.Platform;
  arch: string;
  docker: boolean;
  postgres: boolean;
  pgVersion: string | null;
  pgConfig: string | null;
  rust: boolean;
  rustVersion: string | null;
  cargo: boolean;
  pgrx: boolean;
  pgrxVersion: string | null;
  sudo: boolean;
  packageManager: 'apt' | 'yum' | 'dnf' | 'brew' | 'pacman' | 'unknown';
}

export class InstallCommands {

  /**
   * Comprehensive system check
   */
  static async checkSystem(): Promise<SystemInfo> {
    const info: SystemInfo = {
      platform: os.platform(),
      arch: os.arch(),
      docker: false,
      postgres: false,
      pgVersion: null,
      pgConfig: null,
      rust: false,
      rustVersion: null,
      cargo: false,
      pgrx: false,
      pgrxVersion: null,
      sudo: false,
      packageManager: 'unknown',
    };

    // Check Docker
    try {
      execSync('docker --version', { stdio: 'pipe' });
      info.docker = true;
    } catch { /* not available */ }

    // Check PostgreSQL
    try {
      const pgVersion = execSync('psql --version', { stdio: 'pipe', encoding: 'utf-8' });
      info.postgres = true;
      const match = pgVersion.match(/(\d+)/);
      if (match) info.pgVersion = match[1];
    } catch { /* not available */ }

    // Check pg_config
    try {
      info.pgConfig = execSync('pg_config --libdir', { stdio: 'pipe', encoding: 'utf-8' }).trim();
    } catch { /* not available */ }

    // Check Rust
    try {
      const rustVersion = execSync('rustc --version', { stdio: 'pipe', encoding: 'utf-8' });
      info.rust = true;
      const match = rustVersion.match(/rustc (\d+\.\d+\.\d+)/);
      if (match) info.rustVersion = match[1];
    } catch { /* not available */ }

    // Check Cargo
    try {
      execSync('cargo --version', { stdio: 'pipe' });
      info.cargo = true;
    } catch { /* not available */ }

    // Check pgrx
    try {
      const pgrxVersion = execSync('cargo pgrx --version', { stdio: 'pipe', encoding: 'utf-8' });
      info.pgrx = true;
      const match = pgrxVersion.match(/cargo-pgrx (\d+\.\d+\.\d+)/);
      if (match) info.pgrxVersion = match[1];
    } catch { /* not available */ }

    // Check sudo
    try {
      execSync('sudo -n true', { stdio: 'pipe' });
      info.sudo = true;
    } catch { /* not available or needs password */ }

    // Detect package manager
    if (info.platform === 'darwin') {
      try {
        execSync('brew --version', { stdio: 'pipe' });
        info.packageManager = 'brew';
      } catch { /* not available */ }
    } else if (info.platform === 'linux') {
      if (fs.existsSync('/usr/bin/apt-get')) {
        info.packageManager = 'apt';
      } else if (fs.existsSync('/usr/bin/dnf')) {
        info.packageManager = 'dnf';
      } else if (fs.existsSync('/usr/bin/yum')) {
        info.packageManager = 'yum';
      } else if (fs.existsSync('/usr/bin/pacman')) {
        info.packageManager = 'pacman';
      }
    }

    return info;
  }

  /**
   * Check system requirements (backward compatible)
   */
  static async checkRequirements(): Promise<{ docker: boolean; postgres: boolean; pgConfig: string | null }> {
    const sys = await this.checkSystem();
    return {
      docker: sys.docker,
      postgres: sys.postgres,
      pgConfig: sys.pgConfig,
    };
  }

  /**
   * Run command with sudo if needed
   */
  static sudoExec(command: string, options: { silent?: boolean } = {}): string {
    const needsSudo = process.getuid?.() !== 0;
    const fullCommand = needsSudo ? `sudo ${command}` : command;

    return execSync(fullCommand, {
      stdio: options.silent ? 'pipe' : 'inherit',
      encoding: 'utf-8',
    });
  }

  /**
   * Install PostgreSQL
   */
  static async installPostgreSQL(pgVersion: string, sys: SystemInfo): Promise<boolean> {
    const spinner = ora(`Installing PostgreSQL ${pgVersion}...`).start();

    try {
      if (sys.platform === 'darwin') {
        if (sys.packageManager !== 'brew') {
          spinner.fail('Homebrew not found. Please install it first: https://brew.sh');
          return false;
        }
        execSync(`brew install postgresql@${pgVersion}`, { stdio: 'inherit' });
        execSync(`brew services start postgresql@${pgVersion}`, { stdio: 'inherit' });

        // Add to PATH
        const brewPrefix = execSync('brew --prefix', { encoding: 'utf-8' }).trim();
        process.env.PATH = `${brewPrefix}/opt/postgresql@${pgVersion}/bin:${process.env.PATH}`;

        spinner.succeed(`PostgreSQL ${pgVersion} installed via Homebrew`);
        return true;
      }

      if (sys.platform === 'linux') {
        switch (sys.packageManager) {
          case 'apt':
            // Add PostgreSQL APT repository
            spinner.text = 'Adding PostgreSQL APT repository...';
            this.sudoExec('apt-get update');
            this.sudoExec('apt-get install -y wget gnupg2 lsb-release');
            this.sudoExec('sh -c \'echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list\'');
            this.sudoExec('wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -');
            this.sudoExec('apt-get update');

            // Install PostgreSQL and dev files
            spinner.text = `Installing PostgreSQL ${pgVersion} and development files...`;
            this.sudoExec(`apt-get install -y postgresql-${pgVersion} postgresql-server-dev-${pgVersion}`);

            // Start service
            this.sudoExec(`systemctl start postgresql`);
            this.sudoExec(`systemctl enable postgresql`);

            spinner.succeed(`PostgreSQL ${pgVersion} installed via APT`);
            return true;

          case 'dnf':
          case 'yum':
            const pkg = sys.packageManager;
            spinner.text = 'Adding PostgreSQL repository...';
            this.sudoExec(`${pkg} install -y https://download.postgresql.org/pub/repos/yum/reporpms/EL-$(rpm -E %{rhel})-x86_64/pgdg-redhat-repo-latest.noarch.rpm`);
            this.sudoExec(`${pkg} install -y postgresql${pgVersion}-server postgresql${pgVersion}-devel`);
            this.sudoExec(`/usr/pgsql-${pgVersion}/bin/postgresql-${pgVersion}-setup initdb`);
            this.sudoExec(`systemctl start postgresql-${pgVersion}`);
            this.sudoExec(`systemctl enable postgresql-${pgVersion}`);

            spinner.succeed(`PostgreSQL ${pgVersion} installed via ${pkg.toUpperCase()}`);
            return true;

          case 'pacman':
            this.sudoExec(`pacman -S --noconfirm postgresql`);
            this.sudoExec(`su - postgres -c "initdb -D /var/lib/postgres/data"`);
            this.sudoExec(`systemctl start postgresql`);
            this.sudoExec(`systemctl enable postgresql`);

            spinner.succeed('PostgreSQL installed via Pacman');
            return true;

          default:
            spinner.fail('Unknown package manager. Please install PostgreSQL manually.');
            return false;
        }
      }

      spinner.fail(`Unsupported platform: ${sys.platform}`);
      return false;

    } catch (error) {
      spinner.fail('Failed to install PostgreSQL');
      console.error(chalk.red((error as Error).message));
      return false;
    }
  }

  /**
   * Install Rust
   */
  static async installRust(): Promise<boolean> {
    const spinner = ora('Installing Rust...').start();

    try {
      // Use rustup to install Rust
      execSync('curl --proto \'=https\' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y', {
        stdio: 'inherit',
        shell: '/bin/bash',
      });

      // Source cargo env
      const cargoEnv = path.join(os.homedir(), '.cargo', 'env');
      if (fs.existsSync(cargoEnv)) {
        process.env.PATH = `${path.join(os.homedir(), '.cargo', 'bin')}:${process.env.PATH}`;
      }

      spinner.succeed('Rust installed via rustup');
      return true;
    } catch (error) {
      spinner.fail('Failed to install Rust');
      console.error(chalk.red((error as Error).message));
      return false;
    }
  }

  /**
   * Install required build dependencies
   */
  static async installBuildDeps(sys: SystemInfo, pgVersion?: string): Promise<boolean> {
    const spinner = ora('Installing build dependencies...').start();
    const pg = pgVersion || sys.pgVersion || DEFAULT_PG_VERSION;

    try {
      if (sys.platform === 'darwin') {
        execSync('brew install llvm pkg-config openssl cmake', { stdio: 'inherit' });
      } else if (sys.platform === 'linux') {
        switch (sys.packageManager) {
          case 'apt':
            // Update package lists first, then install PostgreSQL server dev headers for pgrx
            this.sudoExec('apt-get update');
            this.sudoExec(`apt-get install -y build-essential libclang-dev clang pkg-config libssl-dev cmake postgresql-server-dev-${pg}`);
            break;
          case 'dnf':
          case 'yum':
            this.sudoExec(`${sys.packageManager} install -y gcc gcc-c++ clang clang-devel openssl-devel cmake make postgresql${pg}-devel`);
            break;
          case 'pacman':
            this.sudoExec('pacman -S --noconfirm base-devel clang openssl cmake postgresql-libs');
            break;
          default:
            spinner.warn('Please install: gcc, clang, libclang-dev, pkg-config, libssl-dev, cmake, postgresql-server-dev');
            return true;
        }
      }

      spinner.succeed('Build dependencies installed');
      return true;
    } catch (error) {
      spinner.fail('Failed to install build dependencies');
      console.error(chalk.red((error as Error).message));
      return false;
    }
  }

  /**
   * Install cargo-pgrx
   */
  static async installPgrx(pgVersion: string): Promise<boolean> {
    const spinner = ora(`Installing cargo-pgrx ${PGRX_VERSION}...`).start();

    try {
      execSync(`cargo install cargo-pgrx --version ${PGRX_VERSION} --locked`, { stdio: 'inherit' });
      spinner.succeed(`cargo-pgrx ${PGRX_VERSION} installed`);

      // Initialize pgrx
      spinner.start(`Initializing pgrx for PostgreSQL ${pgVersion}...`);

      // Find pg_config
      let pgConfigPath: string;
      try {
        pgConfigPath = execSync(`which pg_config`, { encoding: 'utf-8' }).trim();
      } catch {
        // Try common paths
        const commonPaths = [
          `/usr/lib/postgresql/${pgVersion}/bin/pg_config`,
          `/usr/pgsql-${pgVersion}/bin/pg_config`,
          `/opt/homebrew/opt/postgresql@${pgVersion}/bin/pg_config`,
          `/usr/local/opt/postgresql@${pgVersion}/bin/pg_config`,
        ];
        pgConfigPath = commonPaths.find(p => fs.existsSync(p)) || 'pg_config';
      }

      execSync(`cargo pgrx init --pg${pgVersion}=${pgConfigPath}`, { stdio: 'inherit' });
      spinner.succeed(`pgrx initialized for PostgreSQL ${pgVersion}`);

      return true;
    } catch (error) {
      spinner.fail('Failed to install/initialize pgrx');
      console.error(chalk.red((error as Error).message));
      return false;
    }
  }

  /**
   * Build and install ruvector-postgres extension
   */
  static async buildAndInstallExtension(pgVersion: string): Promise<boolean> {
    const spinner = ora('Building ruvector-postgres extension...').start();

    try {
      // Create temporary directory
      const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ruvector-'));

      spinner.text = 'Cloning ruvector repository...';

      // Clone the actual repository (pgrx needs .control file and proper structure)
      execSync(`git clone --depth 1 https://github.com/ruvnet/ruvector.git ${tmpDir}/ruvector`, {
        stdio: 'pipe',
      });

      const projectDir = path.join(tmpDir, 'ruvector', 'crates', 'ruvector-postgres');

      // Verify the extension directory exists
      if (!fs.existsSync(projectDir)) {
        throw new Error('ruvector-postgres crate not found in repository');
      }

      spinner.text = 'Building extension (this may take 5-10 minutes)...';

      // Build and install using pgrx
      execSync(`cargo pgrx install --features pg${pgVersion} --release`, {
        cwd: projectDir,
        stdio: 'inherit',
        env: {
          ...process.env,
          CARGO_NET_GIT_FETCH_WITH_CLI: 'true',
        },
      });

      // Cleanup
      spinner.text = 'Cleaning up...';
      fs.rmSync(tmpDir, { recursive: true, force: true });

      spinner.succeed('ruvector-postgres extension installed');
      return true;
    } catch (error) {
      spinner.fail('Failed to build extension');
      console.error(chalk.red((error as Error).message));
      return false;
    }
  }

  /**
   * Configure PostgreSQL for the extension
   */
  static async configurePostgreSQL(options: InstallOptions): Promise<boolean> {
    const spinner = ora('Configuring PostgreSQL...').start();

    const user = options.user || DEFAULT_USER;
    const password = options.password || DEFAULT_PASSWORD;
    const database = options.database || DEFAULT_DB;

    try {
      // Create user and database
      const commands = [
        `CREATE USER ${user} WITH PASSWORD '${password}' SUPERUSER;`,
        `CREATE DATABASE ${database} OWNER ${user};`,
        `\\c ${database}`,
        `CREATE EXTENSION IF NOT EXISTS ruvector;`,
      ];

      for (const cmd of commands) {
        try {
          execSync(`sudo -u postgres psql -c "${cmd}"`, { stdio: 'pipe' });
        } catch {
          // User/DB might already exist, that's OK
        }
      }

      spinner.succeed('PostgreSQL configured');
      return true;
    } catch (error) {
      spinner.fail('Failed to configure PostgreSQL');
      console.error(chalk.red((error as Error).message));
      return false;
    }
  }

  /**
   * Full native installation
   */
  static async installNativeFull(options: InstallOptions = {}): Promise<void> {
    const pgVersion = options.pgVersion || DEFAULT_PG_VERSION;

    console.log(chalk.bold.blue('\n🚀 RuVector PostgreSQL Native Installation\n'));
    console.log(chalk.gray('This will install PostgreSQL, Rust, and the RuVector extension.\n'));

    // Check system
    let sys = await this.checkSystem();

    console.log(chalk.bold('📋 System Check:'));
    console.log(`  Platform:    ${chalk.cyan(sys.platform)} ${chalk.cyan(sys.arch)}`);
    console.log(`  PostgreSQL:  ${sys.postgres ? chalk.green(`✓ ${sys.pgVersion}`) : chalk.yellow('✗ Not installed')}`);
    console.log(`  Rust:        ${sys.rust ? chalk.green(`✓ ${sys.rustVersion}`) : chalk.yellow('✗ Not installed')}`);
    console.log(`  cargo-pgrx:  ${sys.pgrx ? chalk.green(`✓ ${sys.pgrxVersion}`) : chalk.yellow('✗ Not installed')}`);
    console.log(`  Pkg Manager: ${chalk.cyan(sys.packageManager)}`);
    console.log();

    // Install PostgreSQL if needed
    if (!sys.postgres && !options.skipPostgres) {
      console.log(chalk.bold(`\n📦 Step 1: Installing PostgreSQL ${pgVersion}`));
      const installed = await this.installPostgreSQL(pgVersion, sys);
      if (!installed) {
        throw new Error('Failed to install PostgreSQL');
      }
      sys = await this.checkSystem(); // Refresh
    } else if (sys.postgres) {
      console.log(chalk.green(`✓ PostgreSQL ${sys.pgVersion} already installed`));
    }

    // Install build dependencies (including PostgreSQL dev headers)
    const targetPgVersion = options.pgVersion || sys.pgVersion || DEFAULT_PG_VERSION;
    console.log(chalk.bold('\n🔧 Step 2: Installing build dependencies'));
    await this.installBuildDeps(sys, targetPgVersion);

    // Install Rust if needed
    if (!sys.rust && !options.skipRust) {
      console.log(chalk.bold('\n🦀 Step 3: Installing Rust'));
      const installed = await this.installRust();
      if (!installed) {
        throw new Error('Failed to install Rust');
      }
      sys = await this.checkSystem(); // Refresh
    } else if (sys.rust) {
      console.log(chalk.green(`✓ Rust ${sys.rustVersion} already installed`));
    }

    // Install pgrx if needed
    if (!sys.pgrx || sys.pgrxVersion !== PGRX_VERSION) {
      console.log(chalk.bold('\n🔌 Step 4: Installing cargo-pgrx'));
      const installed = await this.installPgrx(targetPgVersion);
      if (!installed) {
        throw new Error('Failed to install pgrx');
      }
    } else {
      console.log(chalk.green(`✓ cargo-pgrx ${sys.pgrxVersion} already installed`));
    }

    // Build and install extension
    console.log(chalk.bold('\n🏗️  Step 5: Building RuVector extension'));
    const built = await this.buildAndInstallExtension(targetPgVersion);
    if (!built) {
      throw new Error('Failed to build extension');
    }

    // Configure PostgreSQL
    console.log(chalk.bold('\n⚙️  Step 6: Configuring PostgreSQL'));
    await this.configurePostgreSQL(options);

    // Success!
    const port = options.port || DEFAULT_PORT;
    const user = options.user || DEFAULT_USER;
    const password = options.password || DEFAULT_PASSWORD;
    const database = options.database || DEFAULT_DB;
    const connString = `postgresql://${user}:${password}@localhost:${port}/${database}`;

    console.log(chalk.green.bold('\n✅ RuVector PostgreSQL installed successfully!\n'));

    console.log(chalk.bold('Connection Details:'));
    console.log(`  Host:     ${chalk.cyan('localhost')}`);
    console.log(`  Port:     ${chalk.cyan(port.toString())}`);
    console.log(`  User:     ${chalk.cyan(user)}`);
    console.log(`  Password: ${chalk.cyan(password)}`);
    console.log(`  Database: ${chalk.cyan(database)}`);

    console.log(chalk.bold('\nConnection String:'));
    console.log(`  ${chalk.cyan(connString)}`);

    console.log(chalk.bold('\nQuick Test:'));
    console.log(chalk.gray(`  psql "${connString}" -c "SELECT ruvector_version();"`));

    console.log(chalk.bold('\nExample Usage:'));
    console.log(chalk.gray('  CREATE TABLE embeddings (id serial, vec real[384]);'));
    console.log(chalk.gray('  CREATE INDEX ON embeddings USING hnsw (vec);'));
    console.log(chalk.gray('  INSERT INTO embeddings (vec) VALUES (ARRAY[0.1, 0.2, ...]);'));
  }

  /**
   * Install RuVector PostgreSQL (auto-detect best method)
   */
  static async install(options: InstallOptions = {}): Promise<void> {
    const spinner = ora('Checking system requirements...').start();

    try {
      const sys = await this.checkSystem();
      spinner.succeed('System check complete');

      console.log(chalk.bold('\n📋 System Status:'));
      console.log(`  Docker:     ${sys.docker ? chalk.green('✓ Available') : chalk.yellow('✗ Not found')}`);
      console.log(`  PostgreSQL: ${sys.postgres ? chalk.green(`✓ ${sys.pgVersion}`) : chalk.yellow('✗ Not found')}`);
      console.log(`  Rust:       ${sys.rust ? chalk.green(`✓ ${sys.rustVersion}`) : chalk.yellow('✗ Not found')}`);

      const method = options.method || 'auto';

      if (method === 'auto') {
        // Prefer Docker for simplicity, fall back to native
        if (sys.docker) {
          console.log(chalk.cyan('\n→ Using Docker installation (fastest)\n'));
          await this.installDocker(options);
        } else {
          console.log(chalk.cyan('\n→ Using native installation (will install all dependencies)\n'));
          await this.installNativeFull(options);
        }
      } else if (method === 'docker') {
        if (!sys.docker) {
          throw new Error('Docker not found. Please install Docker first: https://docs.docker.com/get-docker/');
        }
        await this.installDocker(options);
      } else if (method === 'native') {
        await this.installNativeFull(options);
      }
    } catch (error) {
      spinner.fail('Installation failed');
      throw error;
    }
  }

  /**
   * Install via Docker
   */
  static async installDocker(options: InstallOptions = {}): Promise<void> {
    const port = options.port || DEFAULT_PORT;
    const user = options.user || DEFAULT_USER;
    const password = options.password || DEFAULT_PASSWORD;
    const database = options.database || DEFAULT_DB;
    const version = options.version || DOCKER_IMAGE_VERSION;
    const containerName = options.name || 'ruvector-postgres';
    const dataDir = options.dataDir;

    // Check if container already exists
    const existingSpinner = ora('Checking for existing installation...').start();
    try {
      const existing = execSync(`docker ps -a --filter name=^${containerName}$ --format "{{.ID}}"`, { encoding: 'utf-8' }).trim();
      if (existing) {
        existingSpinner.warn(`Container '${containerName}' already exists`);
        console.log(chalk.yellow(`  Run 'ruvector-pg uninstall' first or use a different --name`));
        return;
      }
      existingSpinner.succeed('No existing installation found');
    } catch {
      existingSpinner.succeed('No existing installation found');
    }

    // Check for local image first, then try to pull from Docker Hub
    const pullSpinner = ora(`Checking for ${DOCKER_IMAGE}:${version}...`).start();
    try {
      // Check if image exists locally
      execSync(`docker image inspect ${DOCKER_IMAGE}:${version}`, { stdio: 'pipe' });
      pullSpinner.succeed(`Found local image ${DOCKER_IMAGE}:${version}`);
    } catch {
      // Try pulling from Docker Hub (ruvnet/ruvector-postgres)
      pullSpinner.text = `Pulling ${DOCKER_IMAGE}:${version} from Docker Hub...`;
      try {
        execSync(`docker pull ${DOCKER_IMAGE}:${version}`, { stdio: 'pipe' });
        pullSpinner.succeed(`Pulled ${DOCKER_IMAGE}:${version}`);
      } catch {
        pullSpinner.fail('Image not found locally or on Docker Hub');
        console.log(chalk.yellow('\n📦 To build the image locally, run:'));
        console.log(chalk.gray('   git clone https://github.com/ruvnet/ruvector.git'));
        console.log(chalk.gray('   cd ruvector'));
        console.log(chalk.gray(`   docker build -f crates/ruvector-postgres/docker/Dockerfile -t ${DOCKER_IMAGE}:${version} .`));
        console.log(chalk.yellow('\n   Then run this install command again.'));
        console.log(chalk.yellow('\n💡 Or use native installation:'));
        console.log(chalk.gray('   npx @ruvector/postgres-cli install --method native\n'));
        throw new Error(`RuVector Docker image not available. Build it first or use native installation.`);
      }
    }

    // Build run command
    let runCmd = `docker run -d --name ${containerName}`;
    runCmd += ` -p ${port}:5432`;
    runCmd += ` -e POSTGRES_USER=${user}`;
    runCmd += ` -e POSTGRES_PASSWORD=${password}`;
    runCmd += ` -e POSTGRES_DB=${database}`;

    if (dataDir) {
      const absDataDir = path.resolve(dataDir);
      if (!fs.existsSync(absDataDir)) {
        fs.mkdirSync(absDataDir, { recursive: true });
      }
      runCmd += ` -v ${absDataDir}:/var/lib/postgresql/data`;
    }

    runCmd += ` ${DOCKER_IMAGE}:${version}`;

    // Run container
    const runSpinner = ora('Starting RuVector PostgreSQL...').start();
    try {
      execSync(runCmd, { encoding: 'utf-8' });
      runSpinner.succeed('Container started');

      // Wait for PostgreSQL to be ready
      const readySpinner = ora('Waiting for PostgreSQL to be ready...').start();
      let ready = false;
      for (let i = 0; i < 30; i++) {
        try {
          execSync(`docker exec ${containerName} pg_isready -U ${user}`, { stdio: 'pipe' });
          ready = true;
          break;
        } catch {
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
      }

      if (ready) {
        readySpinner.succeed('PostgreSQL is ready');
      } else {
        readySpinner.warn('PostgreSQL may still be starting...');
      }

      // Verify extension
      const verifySpinner = ora('Verifying RuVector extension...').start();
      try {
        const extCheck = execSync(
          `docker exec ${containerName} psql -U ${user} -d ${database} -c "SELECT extname, extversion FROM pg_extension WHERE extname = 'ruvector';"`,
          { encoding: 'utf-8' }
        );
        if (extCheck.includes('ruvector')) {
          verifySpinner.succeed('RuVector extension verified');
        } else {
          verifySpinner.warn('Extension may need manual activation');
        }
      } catch {
        verifySpinner.warn('Could not verify extension (database may still be initializing)');
      }

      // Print success message
      console.log(chalk.green.bold('\n✅ RuVector PostgreSQL installed successfully!\n'));
      console.log(chalk.bold('Connection Details:'));
      console.log(`  Host:     ${chalk.cyan('localhost')}`);
      console.log(`  Port:     ${chalk.cyan(port.toString())}`);
      console.log(`  User:     ${chalk.cyan(user)}`);
      console.log(`  Password: ${chalk.cyan(password)}`);
      console.log(`  Database: ${chalk.cyan(database)}`);
      console.log(`  Container: ${chalk.cyan(containerName)}`);

      const connString = `postgresql://${user}:${password}@localhost:${port}/${database}`;
      console.log(chalk.bold('\nConnection String:'));
      console.log(`  ${chalk.cyan(connString)}`);

      console.log(chalk.bold('\nQuick Start:'));
      console.log(`  ${chalk.gray('# Connect with psql')}`);
      console.log(`  psql "${connString}"`);
      console.log(`  ${chalk.gray('# Or use docker')}`);
      console.log(`  docker exec -it ${containerName} psql -U ${user} -d ${database}`);

      console.log(chalk.bold('\nTest HNSW Index:'));
      console.log(chalk.gray(`  CREATE TABLE items (id serial, embedding real[]);`));
      console.log(chalk.gray(`  CREATE INDEX ON items USING hnsw (embedding);`));

    } catch (error) {
      runSpinner.fail('Failed to start container');
      throw error;
    }
  }

  /**
   * Install native extension (download pre-built binaries) - Legacy method
   */
  static async installNative(options: InstallOptions = {}): Promise<void> {
    // Redirect to full native installation
    await this.installNativeFull(options);
  }

  /**
   * Uninstall RuVector PostgreSQL
   */
  static async uninstall(options: { name?: string; removeData?: boolean } = {}): Promise<void> {
    const containerName = options.name || 'ruvector-postgres';

    const spinner = ora(`Stopping container '${containerName}'...`).start();

    try {
      // Stop container
      try {
        execSync(`docker stop ${containerName}`, { stdio: 'pipe' });
        spinner.succeed('Container stopped');
      } catch {
        spinner.info('Container was not running');
      }

      // Remove container
      const removeSpinner = ora('Removing container...').start();
      try {
        execSync(`docker rm ${containerName}`, { stdio: 'pipe' });
        removeSpinner.succeed('Container removed');
      } catch {
        removeSpinner.info('Container already removed');
      }

      if (options.removeData) {
        console.log(chalk.yellow('\n⚠️  Data volumes were not removed (manual cleanup required)'));
      }

      console.log(chalk.green.bold('\n✅ RuVector PostgreSQL uninstalled\n'));

    } catch (error) {
      spinner.fail('Uninstall failed');
      throw error;
    }
  }

  /**
   * Get installation status
   */
  static async status(options: { name?: string } = {}): Promise<StatusInfo> {
    const containerName = options.name || 'ruvector-postgres';

    const info: StatusInfo = {
      installed: false,
      running: false,
      method: 'none',
    };

    // Check Docker installation
    try {
      const containerInfo = execSync(
        `docker inspect ${containerName} --format '{{.State.Running}} {{.Config.Image}} {{.NetworkSettings.Ports}}'`,
        { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'pipe'] }
      ).trim();

      const [running, image] = containerInfo.split(' ');
      info.installed = true;
      info.running = running === 'true';
      info.method = 'docker';
      info.version = image.split(':')[1] || 'latest';
      info.containerId = execSync(`docker inspect ${containerName} --format '{{.Id}}'`, { encoding: 'utf-8' }).trim().substring(0, 12);

      // Get port mapping
      try {
        const portMapping = execSync(
          `docker port ${containerName} 5432`,
          { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'pipe'] }
        ).trim();
        const portMatch = portMapping.match(/:(\d+)$/);
        if (portMatch) {
          info.port = parseInt(portMatch[1]);
          info.connectionString = `postgresql://ruvector:ruvector@localhost:${info.port}/ruvector`;
        }
      } catch { /* port not mapped */ }

    } catch {
      // No Docker installation found, check native
      try {
        execSync('psql -c "SELECT 1 FROM pg_extension WHERE extname = \'ruvector\'" 2>/dev/null', { stdio: 'pipe' });
        info.installed = true;
        info.running = true;
        info.method = 'native';
      } catch { /* not installed */ }
    }

    return info;
  }

  /**
   * Print status information
   */
  static async printStatus(options: { name?: string } = {}): Promise<void> {
    const spinner = ora('Checking installation status...').start();

    const status = await this.status(options);
    spinner.stop();

    console.log(chalk.bold('\n📊 RuVector PostgreSQL Status\n'));

    if (!status.installed) {
      console.log(`  Status: ${chalk.yellow('Not installed')}`);
      console.log(chalk.gray('\n  Run `ruvector-pg install` to install'));
      return;
    }

    console.log(`  Installed: ${chalk.green('Yes')}`);
    console.log(`  Method: ${chalk.cyan(status.method)}`);
    console.log(`  Version: ${chalk.cyan(status.version || 'unknown')}`);
    console.log(`  Running: ${status.running ? chalk.green('Yes') : chalk.red('No')}`);

    if (status.method === 'docker') {
      console.log(`  Container: ${chalk.cyan(status.containerId)}`);
    }

    if (status.port) {
      console.log(`  Port: ${chalk.cyan(status.port.toString())}`);
    }

    if (status.connectionString) {
      console.log(`\n  Connection: ${chalk.cyan(status.connectionString)}`);
    }

    if (!status.running) {
      console.log(chalk.gray('\n  Run `ruvector-pg start` to start the database'));
    }
  }

  /**
   * Start the database
   */
  static async start(options: { name?: string } = {}): Promise<void> {
    const containerName = options.name || 'ruvector-postgres';
    const spinner = ora('Starting RuVector PostgreSQL...').start();

    try {
      execSync(`docker start ${containerName}`, { stdio: 'pipe' });

      // Wait for ready
      for (let i = 0; i < 30; i++) {
        try {
          execSync(`docker exec ${containerName} pg_isready`, { stdio: 'pipe' });
          spinner.succeed('RuVector PostgreSQL started');
          return;
        } catch {
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
      }

      spinner.warn('Started but may not be ready yet');
    } catch (error) {
      spinner.fail('Failed to start');
      throw error;
    }
  }

  /**
   * Stop the database
   */
  static async stop(options: { name?: string } = {}): Promise<void> {
    const containerName = options.name || 'ruvector-postgres';
    const spinner = ora('Stopping RuVector PostgreSQL...').start();

    try {
      execSync(`docker stop ${containerName}`, { stdio: 'pipe' });
      spinner.succeed('RuVector PostgreSQL stopped');
    } catch (error) {
      spinner.fail('Failed to stop');
      throw error;
    }
  }

  /**
   * Show logs
   */
  static async logs(options: { name?: string; follow?: boolean; tail?: number } = {}): Promise<void> {
    const containerName = options.name || 'ruvector-postgres';
    const tail = options.tail || 100;

    try {
      if (options.follow) {
        const child = spawn('docker', ['logs', containerName, '--tail', tail.toString(), '-f'], {
          stdio: 'inherit'
        });
        child.on('error', (err) => {
          console.error(chalk.red(`Error: ${err.message}`));
        });
      } else {
        const output = execSync(`docker logs ${containerName} --tail ${tail}`, { encoding: 'utf-8' });
        console.log(output);
      }
    } catch (error) {
      console.error(chalk.red('Failed to get logs'));
      throw error;
    }
  }

  /**
   * Execute psql command
   */
  static async psql(options: { name?: string; command?: string } = {}): Promise<void> {
    const containerName = options.name || 'ruvector-postgres';

    if (options.command) {
      try {
        const output = execSync(
          `docker exec ${containerName} psql -U ruvector -d ruvector -c "${options.command}"`,
          { encoding: 'utf-8' }
        );
        console.log(output);
      } catch (error) {
        console.error(chalk.red('Failed to execute command'));
        throw error;
      }
    } else {
      // Interactive mode
      const child = spawn('docker', ['exec', '-it', containerName, 'psql', '-U', 'ruvector', '-d', 'ruvector'], {
        stdio: 'inherit'
      });
      child.on('error', (err) => {
        console.error(chalk.red(`Error: ${err.message}`));
      });
    }
  }
}
