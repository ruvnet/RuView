import { create } from 'zustand';
import type { NetworkStats, NodeInfo, TimeCrystal, CreditBalance } from '../types';
import { edgeNetService } from '../services/edgeNet';
import { storageService } from '../services/storage';
import { relayClient, type TaskAssignment, type NetworkState as RelayNetworkState } from '../services/relayClient';

interface ContributionSettings {
  enabled: boolean;
  cpuLimit: number;
  gpuEnabled: boolean;
  gpuLimit: number;
  memoryLimit: number;
  bandwidthLimit: number;
  respectBattery: boolean;
  onlyWhenIdle: boolean;
  idleThreshold: number;
  consentGiven: boolean;
  consentTimestamp: Date | null;
}

interface NetworkState {
  stats: NetworkStats;
  nodes: NodeInfo[];
  timeCrystal: TimeCrystal;
  credits: CreditBalance;
  isConnected: boolean;
  isRelayConnected: boolean;
  isLoading: boolean;
  error: string | null;
  startTime: number;
  contributionSettings: ContributionSettings;
  isWASMReady: boolean;
  nodeId: string | null;
  // Relay network state
  relayNetworkState: RelayNetworkState | null;
  connectedPeers: string[];
  pendingTasks: TaskAssignment[];
  // Firebase peers (alias for connectedPeers for backward compatibility)
  firebasePeers: string[];
  // Persisted cumulative values from IndexedDB
  persistedCredits: number;
  persistedTasks: number;
  persistedUptime: number;

  setStats: (stats: Partial<NetworkStats>) => void;
  addNode: (node: NodeInfo) => void;
  removeNode: (nodeId: string) => void;
  updateNode: (nodeId: string, updates: Partial<NodeInfo>) => void;
  setTimeCrystal: (crystal: Partial<TimeCrystal>) => void;
  setCredits: (credits: Partial<CreditBalance>) => void;
  setConnected: (connected: boolean) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  updateRealStats: () => void;
  getUptime: () => number;
  setContributionSettings: (settings: Partial<ContributionSettings>) => void;
  giveConsent: () => void;
  revokeConsent: () => void;
  initializeEdgeNet: () => Promise<void>;
  startContributing: () => void;
  stopContributing: () => void;
  saveToIndexedDB: () => Promise<void>;
  loadFromIndexedDB: () => Promise<void>;
  connectToRelay: () => Promise<boolean>;
  disconnectFromRelay: () => void;
  processAssignedTask: (task: TaskAssignment) => Promise<void>;
  clearLocalData: () => Promise<void>;
}

const initialStats: NetworkStats = {
  totalNodes: 0,
  activeNodes: 0,
  totalCompute: 0,
  creditsEarned: 0,
  tasksCompleted: 0,
  uptime: 0,
  latency: 0,
  bandwidth: 0,
};

const initialTimeCrystal: TimeCrystal = {
  phase: 0,
  frequency: 1.618,
  coherence: 0,
  entropy: 1.0,
  synchronizedNodes: 0,
};

const initialCredits: CreditBalance = {
  available: 0,
  pending: 0,
  earned: 0,
  spent: 0,
};

const defaultContributionSettings: ContributionSettings = {
  enabled: false,
  cpuLimit: 50,
  gpuEnabled: false,
  gpuLimit: 30,
  memoryLimit: 512,
  bandwidthLimit: 10,
  respectBattery: true,
  onlyWhenIdle: true,
  idleThreshold: 30,
  consentGiven: false,
  consentTimestamp: null,
};

export const useNetworkStore = create<NetworkState>()((set, get) => ({
  stats: initialStats,
  nodes: [],
  timeCrystal: initialTimeCrystal,
  credits: initialCredits,
  isConnected: false,
  isRelayConnected: false,
  isLoading: true,
  error: null,
  startTime: Date.now(),
  contributionSettings: defaultContributionSettings,
  isWASMReady: false,
  nodeId: null,
  relayNetworkState: null,
  connectedPeers: [],
  pendingTasks: [],
  firebasePeers: [], // Kept in sync with connectedPeers for backward compatibility
  persistedCredits: 0,
  persistedTasks: 0,
  persistedUptime: 0,

  setStats: (stats) =>
    set((state) => ({ stats: { ...state.stats, ...stats } })),

  addNode: (node) =>
    set((state) => {
      const newNodes = [...state.nodes, node];
      return {
        nodes: newNodes,
        stats: {
          ...state.stats,
          totalNodes: newNodes.length,
          activeNodes: newNodes.filter((n) => n.status === 'active').length,
        },
      };
    }),

  removeNode: (nodeId) =>
    set((state) => {
      const newNodes = state.nodes.filter((n) => n.id !== nodeId);
      return {
        nodes: newNodes,
        stats: {
          ...state.stats,
          totalNodes: newNodes.length,
          activeNodes: newNodes.filter((n) => n.status === 'active').length,
        },
      };
    }),

  updateNode: (nodeId, updates) =>
    set((state) => ({
      nodes: state.nodes.map((n) =>
        n.id === nodeId ? { ...n, ...updates } : n
      ),
    })),

  setTimeCrystal: (crystal) =>
    set((state) => ({
      timeCrystal: { ...state.timeCrystal, ...crystal },
    })),

  setCredits: (credits) =>
    set((state) => ({
      credits: { ...state.credits, ...credits },
    })),

  setConnected: (connected) =>
    set({ isConnected: connected, isLoading: false }),

  setLoading: (loading) => set({ isLoading: loading }),

  setError: (error) => set({ error, isLoading: false }),

  getUptime: () => {
    const state = get();
    return (Date.now() - state.startTime) / 1000;
  },

  setContributionSettings: (settings) =>
    set((state) => ({
      contributionSettings: { ...state.contributionSettings, ...settings },
    })),

  loadFromIndexedDB: async () => {
    try {
      const savedState = await storageService.loadState();
      if (savedState) {
        set({
          persistedCredits: savedState.creditsEarned,
          persistedTasks: savedState.tasksCompleted,
          persistedUptime: savedState.totalUptime,
          nodeId: savedState.nodeId,
          contributionSettings: {
            ...defaultContributionSettings,
            consentGiven: savedState.consentGiven,
            consentTimestamp: savedState.consentTimestamp
              ? new Date(savedState.consentTimestamp)
              : null,
            cpuLimit: savedState.cpuLimit,
            gpuEnabled: savedState.gpuEnabled,
            gpuLimit: savedState.gpuLimit,
            respectBattery: savedState.respectBattery,
            onlyWhenIdle: savedState.onlyWhenIdle,
          },
          credits: {
            earned: savedState.creditsEarned,
            spent: savedState.creditsSpent,
            available: savedState.creditsEarned - savedState.creditsSpent,
            pending: 0,
          },
          stats: {
            ...initialStats,
            creditsEarned: savedState.creditsEarned,
            tasksCompleted: savedState.tasksCompleted,
          },
        });
        console.log('[EdgeNet] Loaded persisted state:', savedState.creditsEarned, 'rUv');
      }
    } catch (error) {
      console.error('[EdgeNet] Failed to load from IndexedDB:', error);
    }
  },

  saveToIndexedDB: async () => {
    const state = get();
    try {
      await storageService.saveState({
        id: 'primary',
        nodeId: state.nodeId,
        creditsEarned: state.credits.earned,
        creditsSpent: state.credits.spent,
        tasksCompleted: state.stats.tasksCompleted,
        tasksSubmitted: 0,
        totalUptime: state.stats.uptime + state.persistedUptime,
        lastActiveTimestamp: Date.now(),
        consentGiven: state.contributionSettings.consentGiven,
        consentTimestamp: state.contributionSettings.consentTimestamp?.getTime() || null,
        cpuLimit: state.contributionSettings.cpuLimit,
        gpuEnabled: state.contributionSettings.gpuEnabled,
        gpuLimit: state.contributionSettings.gpuLimit,
        respectBattery: state.contributionSettings.respectBattery,
        onlyWhenIdle: state.contributionSettings.onlyWhenIdle,
      });
    } catch (error) {
      console.error('[EdgeNet] Failed to save to IndexedDB:', error);
    }
  },

  giveConsent: () => {
    set((state) => ({
      contributionSettings: {
        ...state.contributionSettings,
        consentGiven: true,
        consentTimestamp: new Date(),
      },
    }));
    get().saveToIndexedDB();
    console.log('[EdgeNet] User consent given for contribution');
  },

  revokeConsent: async () => {
    const { stopContributing } = get();
    stopContributing();
    set((state) => ({
      contributionSettings: {
        ...state.contributionSettings,
        consentGiven: false,
        consentTimestamp: null,
        enabled: false,
      },
    }));
    await storageService.clear();
    console.log('[EdgeNet] User consent revoked, data cleared');
  },

  initializeEdgeNet: async () => {
    try {
      set({ isLoading: true, error: null });
      console.log('[EdgeNet] Initializing...');

      // Load persisted state from IndexedDB first
      await get().loadFromIndexedDB();

      // Initialize WASM module
      await edgeNetService.init();
      const isWASMReady = edgeNetService.isWASMAvailable();
      set({ isWASMReady });

      if (isWASMReady) {
        console.log('[EdgeNet] WASM module ready');
        const node = await edgeNetService.createNode();
        if (node) {
          const nodeId = node.nodeId();
          set({ nodeId });
          console.log('[EdgeNet] Node created:', nodeId);
          edgeNetService.enableTimeCrystal(8);

          // Auto-start if consent was previously given
          const state = get();
          if (state.contributionSettings.consentGiven) {
            edgeNetService.startNode();
            set((s) => ({
              contributionSettings: { ...s.contributionSettings, enabled: true },
            }));
            console.log('[EdgeNet] Auto-started from previous session');

            // Auto-connect to relay
            setTimeout(() => {
              get().connectToRelay();
            }, 1000);
          }
        }
      }

      set({ isConnected: true, isLoading: false });
      console.log('[EdgeNet] Initialization complete');
    } catch (error) {
      console.error('[EdgeNet] Initialization failed:', error);
      set({
        error: error instanceof Error ? error.message : 'Failed to initialize',
        isLoading: false,
      });
    }
  },

  startContributing: async () => {
    const { contributionSettings, isWASMReady, nodeId } = get();
    if (!contributionSettings.consentGiven) {
      console.warn('[EdgeNet] Cannot start without consent');
      return;
    }

    // Start WASM node
    if (isWASMReady) {
      edgeNetService.startNode();
      console.log('[EdgeNet] Started WASM node');
    }

    set((state) => ({
      contributionSettings: { ...state.contributionSettings, enabled: true },
    }));

    // Connect to relay for distributed network
    if (nodeId) {
      const connected = await get().connectToRelay();
      if (connected) {
        console.log('[EdgeNet] Connected to distributed network');
      }
    }

    get().saveToIndexedDB();
    console.log('[EdgeNet] Started contributing');
  },

  stopContributing: () => {
    // Pause WASM node
    edgeNetService.pauseNode();

    // Disconnect from relay
    get().disconnectFromRelay();

    set((state) => ({
      contributionSettings: { ...state.contributionSettings, enabled: false },
    }));
    get().saveToIndexedDB();
    console.log('[EdgeNet] Stopped contributing');
  },

  updateRealStats: () => {
    const state = get();
    const sessionUptime = (Date.now() - state.startTime) / 1000;
    const totalUptime = sessionUptime + state.persistedUptime;
    const { isWASMReady, contributionSettings } = state;

    // Process epoch if contributing (advances WASM state)
    if (isWASMReady && contributionSettings.enabled) {
      edgeNetService.processEpoch();
      edgeNetService.stepCapabilities(1.0);
      edgeNetService.recordPerformance(0.95, 100);

      // Submit demo tasks periodically (every ~5 seconds) and process them
      if (Math.floor(sessionUptime) % 5 === 0) {
        edgeNetService.submitDemoTask();
      }
      // Process any queued tasks to earn credits
      edgeNetService.processNextTask().catch(() => {
        // No tasks available is normal
      });
    }

    // Get REAL stats from WASM node
    const realStats = edgeNetService.getStats();
    const timeCrystalSync = edgeNetService.getTimeCrystalSync();
    const networkFitness = edgeNetService.getNetworkFitness();

    // Debug: Log raw stats periodically
    if (realStats && Math.floor(sessionUptime) % 10 === 0) {
      console.log('[EdgeNet] Raw WASM stats:', {
        ruv_earned: realStats.ruv_earned?.toString(),
        tasks_completed: realStats.tasks_completed?.toString(),
        multiplier: realStats.multiplier,
        reputation: realStats.reputation,
        timeCrystalSync,
        networkFitness,
      });
    }

    if (realStats) {
      // Convert from nanoRuv (1e9) to Ruv
      const sessionRuvEarned = Number(realStats.ruv_earned) / 1e9;
      const sessionRuvSpent = Number(realStats.ruv_spent) / 1e9;
      const sessionTasks = Number(realStats.tasks_completed);

      // Add persisted values for cumulative totals
      const totalRuvEarned = state.persistedCredits + sessionRuvEarned;
      const totalTasks = state.persistedTasks + sessionTasks;

      set({
        stats: {
          totalNodes: contributionSettings.enabled ? 1 : 0,
          activeNodes: contributionSettings.enabled ? 1 : 0,
          totalCompute: Math.round(networkFitness * (contributionSettings.cpuLimit / 100) * 100) / 100,
          creditsEarned: Math.round(totalRuvEarned * 100) / 100,
          tasksCompleted: totalTasks,
          uptime: Math.round(totalUptime * 10) / 10,
          latency: Math.round((1 - timeCrystalSync) * 100),
          bandwidth: Math.round(contributionSettings.bandwidthLimit * 10) / 10,
        },
        timeCrystal: {
          ...state.timeCrystal,
          phase: (state.timeCrystal.phase + 0.01) % 1,
          coherence: Math.round(timeCrystalSync * 1000) / 1000,
          entropy: Math.round((1 - timeCrystalSync * 0.8) * 1000) / 1000,
          synchronizedNodes: contributionSettings.enabled ? 1 : 0,
        },
        credits: {
          available: Math.round((totalRuvEarned - sessionRuvSpent - state.credits.spent) * 100) / 100,
          pending: 0,
          earned: Math.round(totalRuvEarned * 100) / 100,
          spent: Math.round((sessionRuvSpent + state.credits.spent) * 100) / 100,
        },
        isConnected: isWASMReady || get().isRelayConnected,
        isLoading: false,
      });

      // Save to IndexedDB periodically (every 10 seconds worth of updates)
      if (Math.floor(sessionUptime) % 10 === 0) {
        get().saveToIndexedDB();
      }
    } else {
      // WASM not ready - show zeros but keep persisted values
      set({
        stats: {
          ...state.stats,
          totalNodes: 0,
          activeNodes: 0,
          totalCompute: 0,
          uptime: Math.round(totalUptime * 10) / 10,
          creditsEarned: state.persistedCredits,
          tasksCompleted: state.persistedTasks,
        },
        credits: {
          ...state.credits,
          earned: state.persistedCredits,
        },
        isConnected: false,
        isLoading: !isWASMReady,
      });
    }
  },

  connectToRelay: async () => {
    const state = get();
    if (!state.nodeId) {
      console.warn('[EdgeNet] Cannot connect to relay without node ID');
      return false;
    }

    // Set up relay event handlers
    relayClient.setHandlers({
      onConnected: (_nodeId, networkState, peers) => {
        console.log('[EdgeNet] Connected to relay, peers:', peers.length);
        set({
          isRelayConnected: true,
          relayNetworkState: networkState,
          connectedPeers: peers,
          firebasePeers: peers,
          stats: {
            ...get().stats,
            activeNodes: networkState.activeNodes + 1, // Include ourselves
            totalNodes: networkState.totalNodes + 1,
          },
          timeCrystal: {
            ...get().timeCrystal,
            phase: networkState.timeCrystalPhase,
            synchronizedNodes: networkState.activeNodes + 1,
          },
        });
      },

      onDisconnected: () => {
        console.log('[EdgeNet] Disconnected from relay');
        set({
          isRelayConnected: false,
          connectedPeers: [],
          firebasePeers: [],
        });
      },

      onNodeJoined: (nodeId, totalNodes) => {
        console.log('[EdgeNet] Peer joined:', nodeId);
        set((s) => ({
          connectedPeers: [...s.connectedPeers, nodeId],
          firebasePeers: [...s.firebasePeers, nodeId],
          stats: { ...s.stats, activeNodes: totalNodes, totalNodes },
          timeCrystal: { ...s.timeCrystal, synchronizedNodes: totalNodes },
        }));
      },

      onNodeLeft: (nodeId, totalNodes) => {
        console.log('[EdgeNet] Peer left:', nodeId);
        set((s) => ({
          connectedPeers: s.connectedPeers.filter((id) => id !== nodeId),
          firebasePeers: s.firebasePeers.filter((id) => id !== nodeId),
          stats: { ...s.stats, activeNodes: totalNodes, totalNodes },
          timeCrystal: { ...s.timeCrystal, synchronizedNodes: totalNodes },
        }));
      },

      onTaskAssigned: (task) => {
        console.log('[EdgeNet] Task assigned:', task.id);
        set((s) => ({
          pendingTasks: [...s.pendingTasks, task],
        }));
        // Auto-process the task
        get().processAssignedTask(task);
      },

      onCreditEarned: (amount, taskId) => {
        const ruvAmount = Number(amount) / 1e9; // Convert from nanoRuv
        console.log('[EdgeNet] Credit earned:', ruvAmount, 'rUv for task', taskId);
        set((s) => ({
          credits: {
            ...s.credits,
            earned: s.credits.earned + ruvAmount,
            available: s.credits.available + ruvAmount,
          },
          stats: {
            ...s.stats,
            creditsEarned: s.stats.creditsEarned + ruvAmount,
            tasksCompleted: s.stats.tasksCompleted + 1,
          },
        }));
        get().saveToIndexedDB();
      },

      onTimeCrystalSync: (phase, _timestamp, activeNodes) => {
        set((s) => ({
          timeCrystal: {
            ...s.timeCrystal,
            phase,
            synchronizedNodes: activeNodes,
            coherence: Math.min(1, activeNodes / 10), // Coherence increases with more nodes
          },
        }));
      },

      onError: (error) => {
        console.error('[EdgeNet] Relay error:', error);
        set({ error: error.message });
      },
    });

    // Connect to the relay
    const connected = await relayClient.connect(state.nodeId);
    if (connected) {
      console.log('[EdgeNet] Relay connection established');
    } else {
      console.warn('[EdgeNet] Failed to connect to relay');
    }
    return connected;
  },

  disconnectFromRelay: () => {
    relayClient.disconnect();
    set({
      isRelayConnected: false,
      connectedPeers: [],
      firebasePeers: [],
      pendingTasks: [],
    });
  },

  processAssignedTask: async (task) => {
    const state = get();
    if (!state.isWASMReady) {
      console.warn('[EdgeNet] Cannot process task - WASM not ready');
      return;
    }

    try {
      console.log('[EdgeNet] Processing task:', task.id, task.taskType);

      // Process the task using WASM
      const result = await edgeNetService.submitTask(
        task.taskType,
        task.payload,
        task.maxCredits
      );

      // Process the task in WASM node
      await edgeNetService.processNextTask();

      // Report completion to relay
      const reward = task.maxCredits / BigInt(2); // Earn half the max credits
      relayClient.completeTask(task.id, task.submitter, result, reward);

      // Remove from pending
      set((s) => ({
        pendingTasks: s.pendingTasks.filter((t) => t.id !== task.id),
      }));

      console.log('[EdgeNet] Task completed:', task.id);
    } catch (error) {
      console.error('[EdgeNet] Task processing failed:', error);
    }
  },

  clearLocalData: async () => {
    // Disconnect from relay
    get().disconnectFromRelay();
    // Stop contributing
    get().stopContributing();
    // Clear IndexedDB
    await storageService.clear();
    // Reset state to defaults
    set({
      stats: initialStats,
      nodes: [],
      timeCrystal: initialTimeCrystal,
      credits: initialCredits,
      isConnected: false,
      isRelayConnected: false,
      isLoading: false,
      error: null,
      startTime: Date.now(),
      contributionSettings: defaultContributionSettings,
      isWASMReady: false,
      nodeId: null,
      relayNetworkState: null,
      connectedPeers: [],
      pendingTasks: [],
      firebasePeers: [],
      persistedCredits: 0,
      persistedTasks: 0,
      persistedUptime: 0,
    });
    console.log('[EdgeNet] Local data cleared');
  },
}));
