"use strict";
/**
 * Logger utility
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.createLogger = createLogger;
const LOG_LEVELS = {
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
class SimpleLogger {
    constructor(options = {}) {
        this.level = LOG_LEVELS[options.level ?? 'info'];
        this.name = options.name ?? 'ruvbot';
        this.pretty = options.pretty ?? true;
    }
    log(level, message, ...args) {
        if (LOG_LEVELS[level] < this.level)
            return;
        const timestamp = new Date().toISOString();
        const prefix = this.pretty
            ? `[${timestamp}] ${level.toUpperCase().padEnd(5)} [${this.name}]`
            : JSON.stringify({ timestamp, level, name: this.name });
        if (this.pretty) {
            console.log(`${prefix} ${message}`, ...args);
        }
        else {
            console.log(JSON.stringify({
                timestamp,
                level,
                name: this.name,
                msg: message,
                ...(args.length > 0 ? { args } : {}),
            }));
        }
    }
    trace(message, ...args) {
        this.log('trace', message, ...args);
    }
    debug(message, ...args) {
        this.log('debug', message, ...args);
    }
    info(message, ...args) {
        this.log('info', message, ...args);
    }
    warn(message, ...args) {
        this.log('warn', message, ...args);
    }
    error(message, ...args) {
        this.log('error', message, ...args);
    }
    fatal(message, ...args) {
        this.log('fatal', message, ...args);
    }
    child(bindings) {
        return new SimpleLogger({
            level: Object.entries(LOG_LEVELS).find(([_, v]) => v === this.level)?.[0],
            pretty: this.pretty,
            name: bindings.name ? `${this.name}:${bindings.name}` : this.name,
        });
    }
}
/**
 * Create a logger instance
 */
function createLogger(options) {
    return new SimpleLogger(options);
}
//# sourceMappingURL=logger.js.map