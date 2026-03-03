/**
 * Telemetry and Logging Module
 * Centralized logging and metrics collection
 */

import winston from 'winston';
import { Logger } from '../utils/logger';

/**
 * Create and configure the main application logger
 */
export const logger = new Logger('AIMDS');

/**
 * Winston logger instance for backwards compatibility
 */
export const winstonLogger = winston.createLogger({
  level: process.env.LOG_LEVEL || 'info',
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.errors({ stack: true }),
    winston.format.json()
  ),
  transports: [
    new winston.transports.Console({
      format: winston.format.combine(
        winston.format.colorize(),
        winston.format.simple()
      )
    })
  ]
});

/**
 * Log levels
 */
export enum LogLevel {
  DEBUG = 'debug',
  INFO = 'info',
  WARN = 'warn',
  ERROR = 'error'
}

/**
 * Telemetry event types
 */
export interface TelemetryEvent {
  type: string;
  timestamp: number;
  data?: Record<string, any>;
  level?: LogLevel;
}

/**
 * Telemetry collector for application-wide events
 */
export class TelemetryCollector {
  private events: TelemetryEvent[] = [];
  private maxEvents: number = 10000;

  /**
   * Record a telemetry event
   */
  record(event: TelemetryEvent): void {
    this.events.push({
      ...event,
      timestamp: event.timestamp || Date.now()
    });

    // Keep only the most recent events
    if (this.events.length > this.maxEvents) {
      this.events.shift();
    }

    // Also log to winston
    const level = event.level || LogLevel.INFO;
    winstonLogger.log(level, `Telemetry: ${event.type}`, event.data);
  }

  /**
   * Get recent events
   */
  getEvents(limit: number = 100): TelemetryEvent[] {
    return this.events.slice(-limit);
  }

  /**
   * Clear all events
   */
  clear(): void {
    this.events = [];
  }

  /**
   * Get event statistics
   */
  getStats(): {
    total: number;
    byType: Record<string, number>;
  } {
    const byType: Record<string, number> = {};

    for (const event of this.events) {
      byType[event.type] = (byType[event.type] || 0) + 1;
    }

    return {
      total: this.events.length,
      byType
    };
  }
}

/**
 * Global telemetry collector instance
 */
export const telemetry = new TelemetryCollector();

/**
 * Helper function to log and record telemetry
 */
export function logTelemetry(
  type: string,
  data?: Record<string, any>,
  level: LogLevel = LogLevel.INFO
): void {
  telemetry.record({ type, data, level, timestamp: Date.now() });
}
