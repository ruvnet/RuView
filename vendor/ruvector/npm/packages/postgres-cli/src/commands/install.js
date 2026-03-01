"use strict";
/**
 * RuVector PostgreSQL Installation Commands
 *
 * Provides complete installation of RuVector PostgreSQL extension:
 * - Full native installation (PostgreSQL + Rust + pgrx + extension)
 * - Docker-based installation (recommended for quick start)
 * - Extension management (enable, disable, upgrade)
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.InstallCommands = void 0;
const child_process_1 = require("child_process");
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const os = __importStar(require("os"));
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
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
class InstallCommands {
    /**
     * Comprehensive system check
     */
    static async checkSystem() {
        const info = {
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
            (0, child_process_1.execSync)('docker --version', { stdio: 'pipe' });
            info.docker = true;
        }
        catch { /* not available */ }
        // Check PostgreSQL
        try {
            const pgVersion = (0, child_process_1.execSync)('psql --version', { stdio: 'pipe', encoding: 'utf-8' });
            info.postgres = true;
            const match = pgVersion.match(/(\d+)/);
            if (match)
                info.pgVersion = match[1];
        }
        catch { /* not available */ }
        // Check pg_config
        try {
            info.pgConfig = (0, child_process_1.execSync)('pg_config --libdir', { stdio: 'pipe', encoding: 'utf-8' }).trim();
        }
        catch { /* not available */ }
        // Check Rust
        try {
            const rustVersion = (0, child_process_1.execSync)('rustc --version', { stdio: 'pipe', encoding: 'utf-8' });
            info.rust = true;
            const match = rustVersion.match(/rustc (\d+\.\d+\.\d+)/);
            if (match)
                info.rustVersion = match[1];
        }
        catch { /* not available */ }
        // Check Cargo
        try {
            (0, child_process_1.execSync)('cargo --version', { stdio: 'pipe' });
            info.cargo = true;
        }
        catch { /* not available */ }
        // Check pgrx
        try {
            const pgrxVersion = (0, child_process_1.execSync)('cargo pgrx --version', { stdio: 'pipe', encoding: 'utf-8' });
            info.pgrx = true;
            const match = pgrxVersion.match(/cargo-pgrx (\d+\.\d+\.\d+)/);
            if (match)
                info.pgrxVersion = match[1];
        }
        catch { /* not available */ }
        // Check sudo
        try {
            (0, child_process_1.execSync)('sudo -n true', { stdio: 'pipe' });
            info.sudo = true;
        }
        catch { /* not available or needs password */ }
        // Detect package manager
        if (info.platform === 'darwin') {
            try {
                (0, child_process_1.execSync)('brew --version', { stdio: 'pipe' });
                info.packageManager = 'brew';
            }
            catch { /* not available */ }
        }
        else if (info.platform === 'linux') {
            if (fs.existsSync('/usr/bin/apt-get')) {
                info.packageManager = 'apt';
            }
            else if (fs.existsSync('/usr/bin/dnf')) {
                info.packageManager = 'dnf';
            }
            else if (fs.existsSync('/usr/bin/yum')) {
                info.packageManager = 'yum';
            }
            else if (fs.existsSync('/usr/bin/pacman')) {
                info.packageManager = 'pacman';
            }
        }
        return info;
    }
    /**
     * Check system requirements (backward compatible)
     */
    static async checkRequirements() {
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
    static sudoExec(command, options = {}) {
        const needsSudo = process.getuid?.() !== 0;
        const fullCommand = needsSudo ? `sudo ${command}` : command;
        return (0, child_process_1.execSync)(fullCommand, {
            stdio: options.silent ? 'pipe' : 'inherit',
            encoding: 'utf-8',
        });
    }
    /**
     * Install PostgreSQL
     */
    static async installPostgreSQL(pgVersion, sys) {
        const spinner = (0, ora_1.default)(`Installing PostgreSQL ${pgVersion}...`).start();
        try {
            if (sys.platform === 'darwin') {
                if (sys.packageManager !== 'brew') {
                    spinner.fail('Homebrew not found. Please install it first: https://brew.sh');
                    return false;
                }
                (0, child_process_1.execSync)(`brew install postgresql@${pgVersion}`, { stdio: 'inherit' });
                (0, child_process_1.execSync)(`brew services start postgresql@${pgVersion}`, { stdio: 'inherit' });
                // Add to PATH
                const brewPrefix = (0, child_process_1.execSync)('brew --prefix', { encoding: 'utf-8' }).trim();
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
        }
        catch (error) {
            spinner.fail('Failed to install PostgreSQL');
            console.error(chalk_1.default.red(error.message));
            return false;
        }
    }
    /**
     * Install Rust
     */
    static async installRust() {
        const spinner = (0, ora_1.default)('Installing Rust...').start();
        try {
            // Use rustup to install Rust
            (0, child_process_1.execSync)('curl --proto \'=https\' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y', {
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
        }
        catch (error) {
            spinner.fail('Failed to install Rust');
            console.error(chalk_1.default.red(error.message));
            return false;
        }
    }
    /**
     * Install required build dependencies
     */
    static async installBuildDeps(sys, pgVersion) {
        const spinner = (0, ora_1.default)('Installing build dependencies...').start();
        const pg = pgVersion || sys.pgVersion || DEFAULT_PG_VERSION;
        try {
            if (sys.platform === 'darwin') {
                (0, child_process_1.execSync)('brew install llvm pkg-config openssl cmake', { stdio: 'inherit' });
            }
            else if (sys.platform === 'linux') {
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
        }
        catch (error) {
            spinner.fail('Failed to install build dependencies');
            console.error(chalk_1.default.red(error.message));
            return false;
        }
    }
    /**
     * Install cargo-pgrx
     */
    static async installPgrx(pgVersion) {
        const spinner = (0, ora_1.default)(`Installing cargo-pgrx ${PGRX_VERSION}...`).start();
        try {
            (0, child_process_1.execSync)(`cargo install cargo-pgrx --version ${PGRX_VERSION} --locked`, { stdio: 'inherit' });
            spinner.succeed(`cargo-pgrx ${PGRX_VERSION} installed`);
            // Initialize pgrx
            spinner.start(`Initializing pgrx for PostgreSQL ${pgVersion}...`);
            // Find pg_config
            let pgConfigPath;
            try {
                pgConfigPath = (0, child_process_1.execSync)(`which pg_config`, { encoding: 'utf-8' }).trim();
            }
            catch {
                // Try common paths
                const commonPaths = [
                    `/usr/lib/postgresql/${pgVersion}/bin/pg_config`,
                    `/usr/pgsql-${pgVersion}/bin/pg_config`,
                    `/opt/homebrew/opt/postgresql@${pgVersion}/bin/pg_config`,
                    `/usr/local/opt/postgresql@${pgVersion}/bin/pg_config`,
                ];
                pgConfigPath = commonPaths.find(p => fs.existsSync(p)) || 'pg_config';
            }
            (0, child_process_1.execSync)(`cargo pgrx init --pg${pgVersion}=${pgConfigPath}`, { stdio: 'inherit' });
            spinner.succeed(`pgrx initialized for PostgreSQL ${pgVersion}`);
            return true;
        }
        catch (error) {
            spinner.fail('Failed to install/initialize pgrx');
            console.error(chalk_1.default.red(error.message));
            return false;
        }
    }
    /**
     * Build and install ruvector-postgres extension
     */
    static async buildAndInstallExtension(pgVersion) {
        const spinner = (0, ora_1.default)('Building ruvector-postgres extension...').start();
        try {
            // Create temporary directory
            const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ruvector-'));
            spinner.text = 'Cloning ruvector repository...';
            // Clone the actual repository (pgrx needs .control file and proper structure)
            (0, child_process_1.execSync)(`git clone --depth 1 https://github.com/ruvnet/ruvector.git ${tmpDir}/ruvector`, {
                stdio: 'pipe',
            });
            const projectDir = path.join(tmpDir, 'ruvector', 'crates', 'ruvector-postgres');
            // Verify the extension directory exists
            if (!fs.existsSync(projectDir)) {
                throw new Error('ruvector-postgres crate not found in repository');
            }
            spinner.text = 'Building extension (this may take 5-10 minutes)...';
            // Build and install using pgrx
            (0, child_process_1.execSync)(`cargo pgrx install --features pg${pgVersion} --release`, {
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
        }
        catch (error) {
            spinner.fail('Failed to build extension');
            console.error(chalk_1.default.red(error.message));
            return false;
        }
    }
    /**
     * Configure PostgreSQL for the extension
     */
    static async configurePostgreSQL(options) {
        const spinner = (0, ora_1.default)('Configuring PostgreSQL...').start();
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
                    (0, child_process_1.execSync)(`sudo -u postgres psql -c "${cmd}"`, { stdio: 'pipe' });
                }
                catch {
                    // User/DB might already exist, that's OK
                }
            }
            spinner.succeed('PostgreSQL configured');
            return true;
        }
        catch (error) {
            spinner.fail('Failed to configure PostgreSQL');
            console.error(chalk_1.default.red(error.message));
            return false;
        }
    }
    /**
     * Full native installation
     */
    static async installNativeFull(options = {}) {
        const pgVersion = options.pgVersion || DEFAULT_PG_VERSION;
        console.log(chalk_1.default.bold.blue('\n🚀 RuVector PostgreSQL Native Installation\n'));
        console.log(chalk_1.default.gray('This will install PostgreSQL, Rust, and the RuVector extension.\n'));
        // Check system
        let sys = await this.checkSystem();
        console.log(chalk_1.default.bold('📋 System Check:'));
        console.log(`  Platform:    ${chalk_1.default.cyan(sys.platform)} ${chalk_1.default.cyan(sys.arch)}`);
        console.log(`  PostgreSQL:  ${sys.postgres ? chalk_1.default.green(`✓ ${sys.pgVersion}`) : chalk_1.default.yellow('✗ Not installed')}`);
        console.log(`  Rust:        ${sys.rust ? chalk_1.default.green(`✓ ${sys.rustVersion}`) : chalk_1.default.yellow('✗ Not installed')}`);
        console.log(`  cargo-pgrx:  ${sys.pgrx ? chalk_1.default.green(`✓ ${sys.pgrxVersion}`) : chalk_1.default.yellow('✗ Not installed')}`);
        console.log(`  Pkg Manager: ${chalk_1.default.cyan(sys.packageManager)}`);
        console.log();
        // Install PostgreSQL if needed
        if (!sys.postgres && !options.skipPostgres) {
            console.log(chalk_1.default.bold(`\n📦 Step 1: Installing PostgreSQL ${pgVersion}`));
            const installed = await this.installPostgreSQL(pgVersion, sys);
            if (!installed) {
                throw new Error('Failed to install PostgreSQL');
            }
            sys = await this.checkSystem(); // Refresh
        }
        else if (sys.postgres) {
            console.log(chalk_1.default.green(`✓ PostgreSQL ${sys.pgVersion} already installed`));
        }
        // Install build dependencies (including PostgreSQL dev headers)
        const targetPgVersion = options.pgVersion || sys.pgVersion || DEFAULT_PG_VERSION;
        console.log(chalk_1.default.bold('\n🔧 Step 2: Installing build dependencies'));
        await this.installBuildDeps(sys, targetPgVersion);
        // Install Rust if needed
        if (!sys.rust && !options.skipRust) {
            console.log(chalk_1.default.bold('\n🦀 Step 3: Installing Rust'));
            const installed = await this.installRust();
            if (!installed) {
                throw new Error('Failed to install Rust');
            }
            sys = await this.checkSystem(); // Refresh
        }
        else if (sys.rust) {
            console.log(chalk_1.default.green(`✓ Rust ${sys.rustVersion} already installed`));
        }
        // Install pgrx if needed
        if (!sys.pgrx || sys.pgrxVersion !== PGRX_VERSION) {
            console.log(chalk_1.default.bold('\n🔌 Step 4: Installing cargo-pgrx'));
            const installed = await this.installPgrx(targetPgVersion);
            if (!installed) {
                throw new Error('Failed to install pgrx');
            }
        }
        else {
            console.log(chalk_1.default.green(`✓ cargo-pgrx ${sys.pgrxVersion} already installed`));
        }
        // Build and install extension
        console.log(chalk_1.default.bold('\n🏗️  Step 5: Building RuVector extension'));
        const built = await this.buildAndInstallExtension(targetPgVersion);
        if (!built) {
            throw new Error('Failed to build extension');
        }
        // Configure PostgreSQL
        console.log(chalk_1.default.bold('\n⚙️  Step 6: Configuring PostgreSQL'));
        await this.configurePostgreSQL(options);
        // Success!
        const port = options.port || DEFAULT_PORT;
        const user = options.user || DEFAULT_USER;
        const password = options.password || DEFAULT_PASSWORD;
        const database = options.database || DEFAULT_DB;
        const connString = `postgresql://${user}:${password}@localhost:${port}/${database}`;
        console.log(chalk_1.default.green.bold('\n✅ RuVector PostgreSQL installed successfully!\n'));
        console.log(chalk_1.default.bold('Connection Details:'));
        console.log(`  Host:     ${chalk_1.default.cyan('localhost')}`);
        console.log(`  Port:     ${chalk_1.default.cyan(port.toString())}`);
        console.log(`  User:     ${chalk_1.default.cyan(user)}`);
        console.log(`  Password: ${chalk_1.default.cyan(password)}`);
        console.log(`  Database: ${chalk_1.default.cyan(database)}`);
        console.log(chalk_1.default.bold('\nConnection String:'));
        console.log(`  ${chalk_1.default.cyan(connString)}`);
        console.log(chalk_1.default.bold('\nQuick Test:'));
        console.log(chalk_1.default.gray(`  psql "${connString}" -c "SELECT ruvector_version();"`));
        console.log(chalk_1.default.bold('\nExample Usage:'));
        console.log(chalk_1.default.gray('  CREATE TABLE embeddings (id serial, vec real[384]);'));
        console.log(chalk_1.default.gray('  CREATE INDEX ON embeddings USING hnsw (vec);'));
        console.log(chalk_1.default.gray('  INSERT INTO embeddings (vec) VALUES (ARRAY[0.1, 0.2, ...]);'));
    }
    /**
     * Install RuVector PostgreSQL (auto-detect best method)
     */
    static async install(options = {}) {
        const spinner = (0, ora_1.default)('Checking system requirements...').start();
        try {
            const sys = await this.checkSystem();
            spinner.succeed('System check complete');
            console.log(chalk_1.default.bold('\n📋 System Status:'));
            console.log(`  Docker:     ${sys.docker ? chalk_1.default.green('✓ Available') : chalk_1.default.yellow('✗ Not found')}`);
            console.log(`  PostgreSQL: ${sys.postgres ? chalk_1.default.green(`✓ ${sys.pgVersion}`) : chalk_1.default.yellow('✗ Not found')}`);
            console.log(`  Rust:       ${sys.rust ? chalk_1.default.green(`✓ ${sys.rustVersion}`) : chalk_1.default.yellow('✗ Not found')}`);
            const method = options.method || 'auto';
            if (method === 'auto') {
                // Prefer Docker for simplicity, fall back to native
                if (sys.docker) {
                    console.log(chalk_1.default.cyan('\n→ Using Docker installation (fastest)\n'));
                    await this.installDocker(options);
                }
                else {
                    console.log(chalk_1.default.cyan('\n→ Using native installation (will install all dependencies)\n'));
                    await this.installNativeFull(options);
                }
            }
            else if (method === 'docker') {
                if (!sys.docker) {
                    throw new Error('Docker not found. Please install Docker first: https://docs.docker.com/get-docker/');
                }
                await this.installDocker(options);
            }
            else if (method === 'native') {
                await this.installNativeFull(options);
            }
        }
        catch (error) {
            spinner.fail('Installation failed');
            throw error;
        }
    }
    /**
     * Install via Docker
     */
    static async installDocker(options = {}) {
        const port = options.port || DEFAULT_PORT;
        const user = options.user || DEFAULT_USER;
        const password = options.password || DEFAULT_PASSWORD;
        const database = options.database || DEFAULT_DB;
        const version = options.version || DOCKER_IMAGE_VERSION;
        const containerName = options.name || 'ruvector-postgres';
        const dataDir = options.dataDir;
        // Check if container already exists
        const existingSpinner = (0, ora_1.default)('Checking for existing installation...').start();
        try {
            const existing = (0, child_process_1.execSync)(`docker ps -a --filter name=^${containerName}$ --format "{{.ID}}"`, { encoding: 'utf-8' }).trim();
            if (existing) {
                existingSpinner.warn(`Container '${containerName}' already exists`);
                console.log(chalk_1.default.yellow(`  Run 'ruvector-pg uninstall' first or use a different --name`));
                return;
            }
            existingSpinner.succeed('No existing installation found');
        }
        catch {
            existingSpinner.succeed('No existing installation found');
        }
        // Check for local image first, then try to pull from Docker Hub
        const pullSpinner = (0, ora_1.default)(`Checking for ${DOCKER_IMAGE}:${version}...`).start();
        try {
            // Check if image exists locally
            (0, child_process_1.execSync)(`docker image inspect ${DOCKER_IMAGE}:${version}`, { stdio: 'pipe' });
            pullSpinner.succeed(`Found local image ${DOCKER_IMAGE}:${version}`);
        }
        catch {
            // Try pulling from Docker Hub (ruvnet/ruvector-postgres)
            pullSpinner.text = `Pulling ${DOCKER_IMAGE}:${version} from Docker Hub...`;
            try {
                (0, child_process_1.execSync)(`docker pull ${DOCKER_IMAGE}:${version}`, { stdio: 'pipe' });
                pullSpinner.succeed(`Pulled ${DOCKER_IMAGE}:${version}`);
            }
            catch {
                pullSpinner.fail('Image not found locally or on Docker Hub');
                console.log(chalk_1.default.yellow('\n📦 To build the image locally, run:'));
                console.log(chalk_1.default.gray('   git clone https://github.com/ruvnet/ruvector.git'));
                console.log(chalk_1.default.gray('   cd ruvector'));
                console.log(chalk_1.default.gray(`   docker build -f crates/ruvector-postgres/docker/Dockerfile -t ${DOCKER_IMAGE}:${version} .`));
                console.log(chalk_1.default.yellow('\n   Then run this install command again.'));
                console.log(chalk_1.default.yellow('\n💡 Or use native installation:'));
                console.log(chalk_1.default.gray('   npx @ruvector/postgres-cli install --method native\n'));
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
        const runSpinner = (0, ora_1.default)('Starting RuVector PostgreSQL...').start();
        try {
            (0, child_process_1.execSync)(runCmd, { encoding: 'utf-8' });
            runSpinner.succeed('Container started');
            // Wait for PostgreSQL to be ready
            const readySpinner = (0, ora_1.default)('Waiting for PostgreSQL to be ready...').start();
            let ready = false;
            for (let i = 0; i < 30; i++) {
                try {
                    (0, child_process_1.execSync)(`docker exec ${containerName} pg_isready -U ${user}`, { stdio: 'pipe' });
                    ready = true;
                    break;
                }
                catch {
                    await new Promise(resolve => setTimeout(resolve, 1000));
                }
            }
            if (ready) {
                readySpinner.succeed('PostgreSQL is ready');
            }
            else {
                readySpinner.warn('PostgreSQL may still be starting...');
            }
            // Verify extension
            const verifySpinner = (0, ora_1.default)('Verifying RuVector extension...').start();
            try {
                const extCheck = (0, child_process_1.execSync)(`docker exec ${containerName} psql -U ${user} -d ${database} -c "SELECT extname, extversion FROM pg_extension WHERE extname = 'ruvector';"`, { encoding: 'utf-8' });
                if (extCheck.includes('ruvector')) {
                    verifySpinner.succeed('RuVector extension verified');
                }
                else {
                    verifySpinner.warn('Extension may need manual activation');
                }
            }
            catch {
                verifySpinner.warn('Could not verify extension (database may still be initializing)');
            }
            // Print success message
            console.log(chalk_1.default.green.bold('\n✅ RuVector PostgreSQL installed successfully!\n'));
            console.log(chalk_1.default.bold('Connection Details:'));
            console.log(`  Host:     ${chalk_1.default.cyan('localhost')}`);
            console.log(`  Port:     ${chalk_1.default.cyan(port.toString())}`);
            console.log(`  User:     ${chalk_1.default.cyan(user)}`);
            console.log(`  Password: ${chalk_1.default.cyan(password)}`);
            console.log(`  Database: ${chalk_1.default.cyan(database)}`);
            console.log(`  Container: ${chalk_1.default.cyan(containerName)}`);
            const connString = `postgresql://${user}:${password}@localhost:${port}/${database}`;
            console.log(chalk_1.default.bold('\nConnection String:'));
            console.log(`  ${chalk_1.default.cyan(connString)}`);
            console.log(chalk_1.default.bold('\nQuick Start:'));
            console.log(`  ${chalk_1.default.gray('# Connect with psql')}`);
            console.log(`  psql "${connString}"`);
            console.log(`  ${chalk_1.default.gray('# Or use docker')}`);
            console.log(`  docker exec -it ${containerName} psql -U ${user} -d ${database}`);
            console.log(chalk_1.default.bold('\nTest HNSW Index:'));
            console.log(chalk_1.default.gray(`  CREATE TABLE items (id serial, embedding real[]);`));
            console.log(chalk_1.default.gray(`  CREATE INDEX ON items USING hnsw (embedding);`));
        }
        catch (error) {
            runSpinner.fail('Failed to start container');
            throw error;
        }
    }
    /**
     * Install native extension (download pre-built binaries) - Legacy method
     */
    static async installNative(options = {}) {
        // Redirect to full native installation
        await this.installNativeFull(options);
    }
    /**
     * Uninstall RuVector PostgreSQL
     */
    static async uninstall(options = {}) {
        const containerName = options.name || 'ruvector-postgres';
        const spinner = (0, ora_1.default)(`Stopping container '${containerName}'...`).start();
        try {
            // Stop container
            try {
                (0, child_process_1.execSync)(`docker stop ${containerName}`, { stdio: 'pipe' });
                spinner.succeed('Container stopped');
            }
            catch {
                spinner.info('Container was not running');
            }
            // Remove container
            const removeSpinner = (0, ora_1.default)('Removing container...').start();
            try {
                (0, child_process_1.execSync)(`docker rm ${containerName}`, { stdio: 'pipe' });
                removeSpinner.succeed('Container removed');
            }
            catch {
                removeSpinner.info('Container already removed');
            }
            if (options.removeData) {
                console.log(chalk_1.default.yellow('\n⚠️  Data volumes were not removed (manual cleanup required)'));
            }
            console.log(chalk_1.default.green.bold('\n✅ RuVector PostgreSQL uninstalled\n'));
        }
        catch (error) {
            spinner.fail('Uninstall failed');
            throw error;
        }
    }
    /**
     * Get installation status
     */
    static async status(options = {}) {
        const containerName = options.name || 'ruvector-postgres';
        const info = {
            installed: false,
            running: false,
            method: 'none',
        };
        // Check Docker installation
        try {
            const containerInfo = (0, child_process_1.execSync)(`docker inspect ${containerName} --format '{{.State.Running}} {{.Config.Image}} {{.NetworkSettings.Ports}}'`, { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'pipe'] }).trim();
            const [running, image] = containerInfo.split(' ');
            info.installed = true;
            info.running = running === 'true';
            info.method = 'docker';
            info.version = image.split(':')[1] || 'latest';
            info.containerId = (0, child_process_1.execSync)(`docker inspect ${containerName} --format '{{.Id}}'`, { encoding: 'utf-8' }).trim().substring(0, 12);
            // Get port mapping
            try {
                const portMapping = (0, child_process_1.execSync)(`docker port ${containerName} 5432`, { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'pipe'] }).trim();
                const portMatch = portMapping.match(/:(\d+)$/);
                if (portMatch) {
                    info.port = parseInt(portMatch[1]);
                    info.connectionString = `postgresql://ruvector:ruvector@localhost:${info.port}/ruvector`;
                }
            }
            catch { /* port not mapped */ }
        }
        catch {
            // No Docker installation found, check native
            try {
                (0, child_process_1.execSync)('psql -c "SELECT 1 FROM pg_extension WHERE extname = \'ruvector\'" 2>/dev/null', { stdio: 'pipe' });
                info.installed = true;
                info.running = true;
                info.method = 'native';
            }
            catch { /* not installed */ }
        }
        return info;
    }
    /**
     * Print status information
     */
    static async printStatus(options = {}) {
        const spinner = (0, ora_1.default)('Checking installation status...').start();
        const status = await this.status(options);
        spinner.stop();
        console.log(chalk_1.default.bold('\n📊 RuVector PostgreSQL Status\n'));
        if (!status.installed) {
            console.log(`  Status: ${chalk_1.default.yellow('Not installed')}`);
            console.log(chalk_1.default.gray('\n  Run `ruvector-pg install` to install'));
            return;
        }
        console.log(`  Installed: ${chalk_1.default.green('Yes')}`);
        console.log(`  Method: ${chalk_1.default.cyan(status.method)}`);
        console.log(`  Version: ${chalk_1.default.cyan(status.version || 'unknown')}`);
        console.log(`  Running: ${status.running ? chalk_1.default.green('Yes') : chalk_1.default.red('No')}`);
        if (status.method === 'docker') {
            console.log(`  Container: ${chalk_1.default.cyan(status.containerId)}`);
        }
        if (status.port) {
            console.log(`  Port: ${chalk_1.default.cyan(status.port.toString())}`);
        }
        if (status.connectionString) {
            console.log(`\n  Connection: ${chalk_1.default.cyan(status.connectionString)}`);
        }
        if (!status.running) {
            console.log(chalk_1.default.gray('\n  Run `ruvector-pg start` to start the database'));
        }
    }
    /**
     * Start the database
     */
    static async start(options = {}) {
        const containerName = options.name || 'ruvector-postgres';
        const spinner = (0, ora_1.default)('Starting RuVector PostgreSQL...').start();
        try {
            (0, child_process_1.execSync)(`docker start ${containerName}`, { stdio: 'pipe' });
            // Wait for ready
            for (let i = 0; i < 30; i++) {
                try {
                    (0, child_process_1.execSync)(`docker exec ${containerName} pg_isready`, { stdio: 'pipe' });
                    spinner.succeed('RuVector PostgreSQL started');
                    return;
                }
                catch {
                    await new Promise(resolve => setTimeout(resolve, 1000));
                }
            }
            spinner.warn('Started but may not be ready yet');
        }
        catch (error) {
            spinner.fail('Failed to start');
            throw error;
        }
    }
    /**
     * Stop the database
     */
    static async stop(options = {}) {
        const containerName = options.name || 'ruvector-postgres';
        const spinner = (0, ora_1.default)('Stopping RuVector PostgreSQL...').start();
        try {
            (0, child_process_1.execSync)(`docker stop ${containerName}`, { stdio: 'pipe' });
            spinner.succeed('RuVector PostgreSQL stopped');
        }
        catch (error) {
            spinner.fail('Failed to stop');
            throw error;
        }
    }
    /**
     * Show logs
     */
    static async logs(options = {}) {
        const containerName = options.name || 'ruvector-postgres';
        const tail = options.tail || 100;
        try {
            if (options.follow) {
                const child = (0, child_process_1.spawn)('docker', ['logs', containerName, '--tail', tail.toString(), '-f'], {
                    stdio: 'inherit'
                });
                child.on('error', (err) => {
                    console.error(chalk_1.default.red(`Error: ${err.message}`));
                });
            }
            else {
                const output = (0, child_process_1.execSync)(`docker logs ${containerName} --tail ${tail}`, { encoding: 'utf-8' });
                console.log(output);
            }
        }
        catch (error) {
            console.error(chalk_1.default.red('Failed to get logs'));
            throw error;
        }
    }
    /**
     * Execute psql command
     */
    static async psql(options = {}) {
        const containerName = options.name || 'ruvector-postgres';
        if (options.command) {
            try {
                const output = (0, child_process_1.execSync)(`docker exec ${containerName} psql -U ruvector -d ruvector -c "${options.command}"`, { encoding: 'utf-8' });
                console.log(output);
            }
            catch (error) {
                console.error(chalk_1.default.red('Failed to execute command'));
                throw error;
            }
        }
        else {
            // Interactive mode
            const child = (0, child_process_1.spawn)('docker', ['exec', '-it', containerName, 'psql', '-U', 'ruvector', '-d', 'ruvector'], {
                stdio: 'inherit'
            });
            child.on('error', (err) => {
                console.error(chalk_1.default.red(`Error: ${err.message}`));
            });
        }
    }
}
exports.InstallCommands = InstallCommands;
//# sourceMappingURL=install.js.map