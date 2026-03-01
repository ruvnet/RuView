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

const LOG_LEVELS: Record<LogLevel, number> = {
  trace: 10,
  debug: 20,
  info: 30,
  warn: 40,
  error: 50,
  fatal: 60,
};

/**
 * Simple logger implementation
 * In production, this would use pino or similar
 */
class SimpleLogger implements Logger {
  private level: number;
  private name: string;
  private pretty: boolean;

  constructor(options: LoggerOptions = {}) {
    this.level = LOG_LEVELS[options.level ?? 'info'];
    this.name = options.name ?? 'ruvbot';
    this.pretty = options.pretty ?? true;
  }

  private log(level: LogLevel, message: string, ...args: unknown[]): void {
    if (LOG_LEVELS[level] < this.level) return;

    const timestamp = new Date().toISOString();
    const prefix = this.pretty
      ? `[${timestamp}] ${level.toUpperCase().padEnd(5)} [${this.name}]`
      : JSON.stringify({ timestamp, level, name: this.name });

    if (this.pretty) {
      console.log(`${prefix} ${message}`, ...args);
    } else {
      console.log(
        JSON.stringify({
          timestamp,
          level,
          name: this.name,
          msg: message,
          ...(args.length > 0 ? { args } : {}),
        })
      );
    }
  }

  trace(message: string, ...args: unknown[]): void {
    this.log('trace', message, ...args);
  }

  debug(message: string, ...args: unknown[]): void {
    this.log('debug', message, ...args);
  }

  info(message: string, ...args: unknown[]): void {
    this.log('info', message, ...args);
  }

  warn(message: string, ...args: unknown[]): void {
    this.log('warn', message, ...args);
  }

  error(message: string, ...args: unknown[]): void {
    this.log('error', message, ...args);
  }

  fatal(message: string, ...args: unknown[]): void {
    this.log('fatal', message, ...args);
  }

  child(bindings: Record<string, unknown>): Logger {
    return new SimpleLogger({
      level: Object.entries(LOG_LEVELS).find(([_, v]) => v === this.level)?.[0] as LogLevel,
      pretty: this.pretty,
      name: bindings.name ? `${this.name}:${bindings.name}` : this.name,
    });
  }
}

/**
 * Create a logger instance
 */
export function createLogger(options?: LoggerOptions): Logger {
  return new SimpleLogger(options);
}
