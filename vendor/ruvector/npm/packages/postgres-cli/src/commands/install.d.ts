/**
 * RuVector PostgreSQL Installation Commands
 *
 * Provides complete installation of RuVector PostgreSQL extension:
 * - Full native installation (PostgreSQL + Rust + pgrx + extension)
 * - Docker-based installation (recommended for quick start)
 * - Extension management (enable, disable, upgrade)
 */
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
export declare class InstallCommands {
    /**
     * Comprehensive system check
     */
    static checkSystem(): Promise<SystemInfo>;
    /**
     * Check system requirements (backward compatible)
     */
    static checkRequirements(): Promise<{
        docker: boolean;
        postgres: boolean;
        pgConfig: string | null;
    }>;
    /**
     * Run command with sudo if needed
     */
    static sudoExec(command: string, options?: {
        silent?: boolean;
    }): string;
    /**
     * Install PostgreSQL
     */
    static installPostgreSQL(pgVersion: string, sys: SystemInfo): Promise<boolean>;
    /**
     * Install Rust
     */
    static installRust(): Promise<boolean>;
    /**
     * Install required build dependencies
     */
    static installBuildDeps(sys: SystemInfo, pgVersion?: string): Promise<boolean>;
    /**
     * Install cargo-pgrx
     */
    static installPgrx(pgVersion: string): Promise<boolean>;
    /**
     * Build and install ruvector-postgres extension
     */
    static buildAndInstallExtension(pgVersion: string): Promise<boolean>;
    /**
     * Configure PostgreSQL for the extension
     */
    static configurePostgreSQL(options: InstallOptions): Promise<boolean>;
    /**
     * Full native installation
     */
    static installNativeFull(options?: InstallOptions): Promise<void>;
    /**
     * Install RuVector PostgreSQL (auto-detect best method)
     */
    static install(options?: InstallOptions): Promise<void>;
    /**
     * Install via Docker
     */
    static installDocker(options?: InstallOptions): Promise<void>;
    /**
     * Install native extension (download pre-built binaries) - Legacy method
     */
    static installNative(options?: InstallOptions): Promise<void>;
    /**
     * Uninstall RuVector PostgreSQL
     */
    static uninstall(options?: {
        name?: string;
        removeData?: boolean;
    }): Promise<void>;
    /**
     * Get installation status
     */
    static status(options?: {
        name?: string;
    }): Promise<StatusInfo>;
    /**
     * Print status information
     */
    static printStatus(options?: {
        name?: string;
    }): Promise<void>;
    /**
     * Start the database
     */
    static start(options?: {
        name?: string;
    }): Promise<void>;
    /**
     * Stop the database
     */
    static stop(options?: {
        name?: string;
    }): Promise<void>;
    /**
     * Show logs
     */
    static logs(options?: {
        name?: string;
        follow?: boolean;
        tail?: number;
    }): Promise<void>;
    /**
     * Execute psql command
     */
    static psql(options?: {
        name?: string;
        command?: string;
    }): Promise<void>;
}
export {};
//# sourceMappingURL=install.d.ts.map