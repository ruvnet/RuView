// Network Stats Types
export interface NetworkStats {
  totalNodes: number;
  activeNodes: number;
  totalCompute: number; // TFLOPS
  creditsEarned: number;
  tasksCompleted: number;
  uptime: number; // percentage
  latency: number; // ms
  bandwidth: number; // Mbps
}

export interface NodeInfo {
  id: string;
  status: 'online' | 'offline' | 'busy' | 'idle' | 'active';
  computePower: number;
  creditsEarned: number;
  tasksCompleted: number;
  location?: string;
  lastSeen: Date;
}

// CDN Configuration
export interface CDNScript {
  id: string;
  name: string;
  description: string;
  url: string;
  size: string;
  category: 'wasm' | 'ai' | 'crypto' | 'network' | 'utility';
  enabled: boolean;
  loaded: boolean;
}

export interface CDNConfig {
  scripts: CDNScript[];
  autoLoad: boolean;
  cacheEnabled: boolean;
}

// MCP Tool Types
export interface MCPTool {
  id: string;
  name: string;
  description: string;
  category: 'swarm' | 'agent' | 'memory' | 'neural' | 'task' | 'github';
  status: 'ready' | 'running' | 'error' | 'disabled';
  lastRun?: Date;
  parameters?: Record<string, unknown>;
}

export interface MCPResult {
  toolId: string;
  success: boolean;
  data?: unknown;
  error?: string;
  timestamp: Date;
  duration: number;
}

// WASM Module Types
export interface WASMModule {
  id: string;
  name: string;
  version: string;
  loaded: boolean;
  size: number;
  features: string[];
  status: 'loading' | 'ready' | 'error' | 'unloaded';
  error?: string;
  loadTime?: number; // ms to load
}

export interface WASMBenchmark {
  moduleId: string;
  operation: string;
  iterations: number;
  avgTime: number;
  minTime: number;
  maxTime: number;
  throughput: number;
}

// Dashboard State
export interface DashboardTab {
  id: string;
  label: string;
  icon: string;
  badge?: number;
}

export interface ModalConfig {
  id: string;
  title: string;
  isOpen: boolean;
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full';
}

// Time Crystal Types
export interface TimeCrystal {
  phase: number;
  frequency: number;
  coherence: number;
  entropy: number;
  synchronizedNodes: number;
}

export interface TemporalMetrics {
  crystalPhase: number;
  driftCorrection: number;
  consensusLatency: number;
  epochNumber: number;
}

// Specialized Networks
export interface SpecializedNetwork {
  id: string;
  name: string;
  description: string;
  category: 'science' | 'finance' | 'healthcare' | 'ai' | 'gaming' | 'social' | 'compute';
  icon: string;
  color: string;
  stats: {
    nodes: number;
    compute: number; // TFLOPS
    tasks: number;
    uptime: number; // percentage
  };
  requirements: {
    minCompute: number;
    minBandwidth: number;
    capabilities: string[];
  };
  rewards: {
    baseRate: number; // credits per hour
    bonusMultiplier: number;
  };
  status: 'active' | 'maintenance' | 'launching' | 'closed';
  joined: boolean;
  joinedAt?: Date;
}

// Credit Economy
export interface CreditBalance {
  available: number;
  pending: number;
  earned: number;
  spent: number;
}

export interface CreditTransaction {
  id: string;
  type: 'earn' | 'spend' | 'transfer';
  amount: number;
  description: string;
  timestamp: Date;
}

// Debug Console
export interface DebugLog {
  id: string;
  level: 'info' | 'warn' | 'error' | 'debug';
  message: string;
  data?: unknown;
  timestamp: Date;
  source: string;
}

export interface DebugState {
  logs: DebugLog[];
  isVisible: boolean;
  filter: string;
  level: 'all' | 'info' | 'warn' | 'error' | 'debug';
}
