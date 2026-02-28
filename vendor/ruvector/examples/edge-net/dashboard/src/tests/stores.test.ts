import { describe, it, expect, beforeEach } from 'vitest';
import { useNetworkStore } from '../stores/networkStore';
import { useWASMStore } from '../stores/wasmStore';
import { useMCPStore } from '../stores/mcpStore';
import { useCDNStore } from '../stores/cdnStore';

describe('Network Store', () => {
  beforeEach(() => {
    // Reset to initial state (real data starts at 0)
    useNetworkStore.setState({
      stats: {
        totalNodes: 0,
        activeNodes: 0,
        totalCompute: 0,
        creditsEarned: 0,
        tasksCompleted: 0,
        uptime: 0,
        latency: 0,
        bandwidth: 0,
      },
      isConnected: false,
      isLoading: true,
      error: null,
      startTime: Date.now(),
    });
  });

  it('should start with empty network (real data)', () => {
    const { stats } = useNetworkStore.getState();
    expect(stats.totalNodes).toBe(0);
    expect(stats.activeNodes).toBe(0);
  });

  it('should update stats', () => {
    const { setStats } = useNetworkStore.getState();
    setStats({ activeNodes: 5, totalNodes: 10 });

    const { stats } = useNetworkStore.getState();
    expect(stats.activeNodes).toBe(5);
    expect(stats.totalNodes).toBe(10);
  });

  it('should update real stats and track network', () => {
    // Run multiple ticks to ensure stats update
    for (let i = 0; i < 50; i++) {
      useNetworkStore.getState().updateRealStats();
    }

    const { stats, isConnected } = useNetworkStore.getState();
    // Network should be connected after updates
    expect(isConnected).toBe(true);
    // Some metrics should have updated
    expect(typeof stats.totalCompute).toBe('number');
    expect(typeof stats.uptime).toBe('number');
  });

  it('should track connection status', () => {
    const { setConnected } = useNetworkStore.getState();

    setConnected(false);
    expect(useNetworkStore.getState().isConnected).toBe(false);
    expect(useNetworkStore.getState().isLoading).toBe(false);

    setConnected(true);
    expect(useNetworkStore.getState().isConnected).toBe(true);
  });

  it('should calculate uptime', () => {
    const { getUptime } = useNetworkStore.getState();
    const uptime = getUptime();
    expect(typeof uptime).toBe('number');
    expect(uptime).toBeGreaterThanOrEqual(0);
  });
});

describe('WASM Store', () => {
  it('should have default modules', () => {
    const { modules } = useWASMStore.getState();
    expect(modules.length).toBeGreaterThan(0);
    expect(modules[0].id).toBe('edge-net');
    expect(modules[0].version).toBe('0.1.1');
  });

  it('should start with unloaded modules', () => {
    const { modules } = useWASMStore.getState();
    const edgeNet = modules.find(m => m.id === 'edge-net');
    expect(edgeNet?.loaded).toBe(false);
    expect(edgeNet?.status).toBe('unloaded');
    expect(edgeNet?.size).toBe(0); // Size unknown until loaded
  });

  it('should update module status', () => {
    const { updateModule } = useWASMStore.getState();

    updateModule('edge-net', { status: 'loading' });

    const updatedModules = useWASMStore.getState().modules;
    const edgeNet = updatedModules.find((m) => m.id === 'edge-net');
    expect(edgeNet?.status).toBe('loading');
  });

  it('should track benchmarks', () => {
    const { addBenchmark, benchmarks } = useWASMStore.getState();
    const initialCount = benchmarks.length;

    addBenchmark({
      moduleId: 'edge-net',
      operation: 'vector_ops_256',
      iterations: 1000,
      avgTime: 0.05,
      minTime: 0.01,
      maxTime: 0.15,
      throughput: 20000,
    });

    expect(useWASMStore.getState().benchmarks.length).toBe(initialCount + 1);
  });

  it('should clear benchmarks', () => {
    const { addBenchmark, clearBenchmarks } = useWASMStore.getState();

    addBenchmark({
      moduleId: 'edge-net',
      operation: 'test',
      iterations: 100,
      avgTime: 1,
      minTime: 0.5,
      maxTime: 2,
      throughput: 100,
    });

    clearBenchmarks();
    expect(useWASMStore.getState().benchmarks.length).toBe(0);
  });
});

describe('MCP Store', () => {
  it('should have default tools', () => {
    const { tools } = useMCPStore.getState();
    expect(tools.length).toBeGreaterThan(0);
    expect(tools.some((t) => t.category === 'swarm')).toBe(true);
  });

  it('should update tool status', () => {
    const { updateTool } = useMCPStore.getState();

    updateTool('swarm_init', { status: 'running' });

    const updatedTools = useMCPStore.getState().tools;
    const tool = updatedTools.find((t) => t.id === 'swarm_init');
    expect(tool?.status).toBe('running');
  });

  it('should add results', () => {
    const { addResult } = useMCPStore.getState();

    addResult({
      toolId: 'swarm_init',
      success: true,
      data: { test: true },
      timestamp: new Date(),
      duration: 100,
    });

    const { results } = useMCPStore.getState();
    expect(results.length).toBeGreaterThan(0);
  });
});

describe('CDN Store', () => {
  it('should have default scripts', () => {
    const { scripts } = useCDNStore.getState();
    expect(scripts.length).toBeGreaterThan(0);
    expect(scripts.some((s) => s.category === 'wasm')).toBe(true);
  });

  it('should toggle script enabled state', () => {
    const { toggleScript, scripts } = useCDNStore.getState();
    const initialEnabled = scripts[0].enabled;

    toggleScript(scripts[0].id);

    const updatedScripts = useCDNStore.getState().scripts;
    expect(updatedScripts[0].enabled).toBe(!initialEnabled);
  });

  it('should track auto-load setting', () => {
    const { setAutoLoad } = useCDNStore.getState();

    setAutoLoad(true);
    expect(useCDNStore.getState().autoLoad).toBe(true);

    setAutoLoad(false);
    expect(useCDNStore.getState().autoLoad).toBe(false);
  });
});
