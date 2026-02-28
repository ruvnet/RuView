import type { DebugLog } from '../types';

// Debug state
let debugLogs: DebugLog[] = [];
let logListeners: ((logs: DebugLog[]) => void)[] = [];
let isConsoleOverridden = false;

const MAX_LOGS = 500;

// Generate unique ID
const generateId = () => Math.random().toString(36).substr(2, 9);

// Add log entry
const addLog = (level: DebugLog['level'], message: string, data?: unknown, source = 'app') => {
  const log: DebugLog = {
    id: generateId(),
    level,
    message,
    data,
    timestamp: new Date(),
    source,
  };

  debugLogs = [log, ...debugLogs].slice(0, MAX_LOGS);
  logListeners.forEach((listener) => listener(debugLogs));
};

// Initialize debug console
export const initDebugConsole = () => {
  if (isConsoleOverridden) return;
  isConsoleOverridden = true;

  const originalConsole = {
    log: console.log.bind(console),
    warn: console.warn.bind(console),
    error: console.error.bind(console),
    info: console.info.bind(console),
    debug: console.debug.bind(console),
  };

  // Override console methods
  console.log = (...args: unknown[]) => {
    originalConsole.log(...args);
    addLog('info', formatArgs(args), args.length > 1 ? args : undefined);
  };

  console.warn = (...args: unknown[]) => {
    originalConsole.warn(...args);
    addLog('warn', formatArgs(args), args.length > 1 ? args : undefined);
  };

  console.error = (...args: unknown[]) => {
    originalConsole.error(...args);
    addLog('error', formatArgs(args), args.length > 1 ? args : undefined);
  };

  console.info = (...args: unknown[]) => {
    originalConsole.info(...args);
    addLog('info', formatArgs(args), args.length > 1 ? args : undefined);
  };

  console.debug = (...args: unknown[]) => {
    originalConsole.debug(...args);
    addLog('debug', formatArgs(args), args.length > 1 ? args : undefined);
  };

  // Add global debug utilities
  (window as any).edgeNet = {
    logs: () => debugLogs,
    clear: () => {
      debugLogs = [];
      logListeners.forEach((listener) => listener(debugLogs));
    },
    export: () => JSON.stringify(debugLogs, null, 2),
    stats: () => ({
      total: debugLogs.length,
      byLevel: debugLogs.reduce((acc, log) => {
        acc[log.level] = (acc[log.level] || 0) + 1;
        return acc;
      }, {} as Record<string, number>),
      bySource: debugLogs.reduce((acc, log) => {
        acc[log.source] = (acc[log.source] || 0) + 1;
        return acc;
      }, {} as Record<string, number>),
    }),
  };

  // Log initialization
  console.log('[Debug] Console debug utilities initialized');
  console.log('[Debug] Access debug tools via window.edgeNet');
};

// Format console arguments
const formatArgs = (args: unknown[]): string => {
  return args
    .map((arg) => {
      if (typeof arg === 'string') return arg;
      if (arg instanceof Error) return `${arg.name}: ${arg.message}`;
      try {
        return JSON.stringify(arg);
      } catch {
        return String(arg);
      }
    })
    .join(' ');
};

// Subscribe to log updates
export const subscribeToLogs = (listener: (logs: DebugLog[]) => void) => {
  logListeners.push(listener);
  listener(debugLogs);

  return () => {
    logListeners = logListeners.filter((l) => l !== listener);
  };
};

// Get current logs
export const getLogs = () => debugLogs;

// Clear logs
export const clearLogs = () => {
  debugLogs = [];
  logListeners.forEach((listener) => listener(debugLogs));
};

// Manual log functions
export const debug = {
  log: (message: string, data?: unknown, source?: string) =>
    addLog('info', message, data, source),
  warn: (message: string, data?: unknown, source?: string) =>
    addLog('warn', message, data, source),
  error: (message: string, data?: unknown, source?: string) =>
    addLog('error', message, data, source),
  debug: (message: string, data?: unknown, source?: string) =>
    addLog('debug', message, data, source),
  info: (message: string, data?: unknown, source?: string) =>
    addLog('info', message, data, source),
};

// Performance timing utilities
export const timing = {
  marks: new Map<string, number>(),

  start: (label: string) => {
    timing.marks.set(label, performance.now());
    console.debug(`[Timing] Started: ${label}`);
  },

  end: (label: string) => {
    const start = timing.marks.get(label);
    if (start) {
      const duration = performance.now() - start;
      timing.marks.delete(label);
      console.debug(`[Timing] ${label}: ${duration.toFixed(2)}ms`);
      return duration;
    }
    return 0;
  },

  measure: async <T>(label: string, fn: () => Promise<T>): Promise<T> => {
    timing.start(label);
    try {
      return await fn();
    } finally {
      timing.end(label);
    }
  },
};
