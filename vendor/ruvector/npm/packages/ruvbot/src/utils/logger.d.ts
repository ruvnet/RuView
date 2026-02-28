/**
 * Logger utility
 */
export type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'fatal';
export interface LoggerOptions {
    level?: LogLevel;
    pretty?: boolean;
    name?: string;
}
export interface Logger {
    trace(message: string, ...args: unknown[]): void;
    debug(message: string, ...args: unknown[]): void;
    info(message: string, ...args: unknown[]): void;
    warn(message: string, ...args: unknown[]): void;
    error(message: string, ...args: unknown[]): void;
    fatal(message: string, ...args: unknown[]): void;
    child(bindings: Record<string, unknown>): Logger;
}
/**
 * Create a logger instance
 */
export declare function createLogger(options?: LoggerOptions): Logger;
//# sourceMappingURL=logger.d.ts.map