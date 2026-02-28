import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Activity, Cpu, Users, Zap, Clock, Gauge } from 'lucide-react';
import { useNetworkStore } from '../../stores/networkStore';
import { StatCard } from '../common/StatCard';

// Format uptime seconds to human readable
function formatUptime(seconds: number): string {
  if (seconds < 60) return `${Math.floor(seconds)}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${Math.floor(seconds % 60)}s`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${mins}m`;
}

// Session start time - only tracks current browser session
const sessionStart = Date.now();

export function NetworkStats() {
  const { stats, timeCrystal, isRelayConnected, connectedPeers, contributionSettings } = useNetworkStore();

  // Use React state for session-only uptime
  const [sessionUptime, setSessionUptime] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setSessionUptime((Date.now() - sessionStart) / 1000);
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const statItems = [
    {
      title: 'Active Nodes',
      value: stats.activeNodes,
      icon: <Users size={24} />,
      color: 'crystal' as const,
    },
    {
      title: 'Total Compute',
      value: `${stats.totalCompute.toFixed(1)} TFLOPS`,
      icon: <Cpu size={24} />,
      color: 'temporal' as const,
    },
    {
      title: 'Tasks Completed',
      value: stats.tasksCompleted,
      icon: <Activity size={24} />,
      color: 'quantum' as const,
    },
    {
      title: 'Credits Earned',
      value: `${stats.creditsEarned.toLocaleString()}`,
      icon: <Zap size={24} />,
      color: 'success' as const,
    },
    {
      title: 'Network Latency',
      value: `${stats.latency.toFixed(0)}ms`,
      icon: <Clock size={24} />,
      color: stats.latency < 50 ? 'success' as const : 'warning' as const,
    },
    {
      title: 'This Session',
      value: formatUptime(sessionUptime),
      icon: <Gauge size={24} />,
      color: 'success' as const,
    },
  ];

  return (
    <div className="space-y-6">
      {/* Connection Status Banner */}
      {contributionSettings.enabled && (
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          className={`p-3 rounded-lg border flex items-center justify-between ${
            isRelayConnected
              ? 'bg-emerald-500/10 border-emerald-500/30'
              : 'bg-amber-500/10 border-amber-500/30'
          }`}
        >
          <div className="flex items-center gap-3">
            <div
              className={`w-2 h-2 rounded-full ${
                isRelayConnected ? 'bg-emerald-400 animate-pulse' : 'bg-amber-400'
              }`}
            />
            <span className={isRelayConnected ? 'text-emerald-400' : 'text-amber-400'}>
              {isRelayConnected
                ? `Connected to Edge-Net (${connectedPeers.length + 1} nodes)`
                : 'Connecting to relay...'}
            </span>
          </div>
          {isRelayConnected && (
            <span className="text-xs text-zinc-500">
              wss://edge-net-relay-...us-central1.run.app
            </span>
          )}
        </motion.div>
      )}

      {/* Main Stats Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {statItems.map((stat, index) => (
          <motion.div
            key={stat.title}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: index * 0.1 }}
          >
            <StatCard {...stat} />
          </motion.div>
        ))}
      </div>

      {/* Time Crystal Status */}
      <motion.div
        className="crystal-card p-6"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.6 }}
      >
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <motion.div
            className={`w-3 h-3 rounded-full ${
              isRelayConnected
                ? 'bg-gradient-to-r from-sky-400 to-violet-400'
                : 'bg-zinc-500'
            }`}
            animate={isRelayConnected ? { scale: [1, 1.2, 1] } : {}}
            transition={{ duration: 2, repeat: Infinity }}
          />
          Time Crystal Synchronization
          {!isRelayConnected && contributionSettings.enabled && (
            <span className="text-xs text-amber-400 ml-2">(waiting for relay)</span>
          )}
        </h3>

        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="text-center p-4 rounded-lg bg-sky-500/10 border border-sky-500/20">
            <p className="text-2xl font-bold text-sky-400">
              {(timeCrystal.phase * 100).toFixed(0)}%
            </p>
            <p className="text-xs text-zinc-400 mt-1">Phase</p>
          </div>

          <div className="text-center p-4 rounded-lg bg-violet-500/10 border border-violet-500/20">
            <p className="text-2xl font-bold text-violet-400">
              {timeCrystal.frequency.toFixed(3)}
            </p>
            <p className="text-xs text-zinc-400 mt-1">Frequency (φ)</p>
          </div>

          <div className="text-center p-4 rounded-lg bg-cyan-500/10 border border-cyan-500/20">
            <p className="text-2xl font-bold text-cyan-400">
              {(timeCrystal.coherence * 100).toFixed(1)}%
            </p>
            <p className="text-xs text-zinc-400 mt-1">Coherence</p>
          </div>

          <div className="text-center p-4 rounded-lg bg-emerald-500/10 border border-emerald-500/20">
            <p className="text-2xl font-bold text-emerald-400">
              {timeCrystal.synchronizedNodes}
            </p>
            <p className="text-xs text-zinc-400 mt-1">Synced Nodes</p>
          </div>
        </div>

        {/* Crystal Animation */}
        <div className="mt-6 h-2 bg-zinc-800 rounded-full overflow-hidden">
          <motion.div
            className="h-full bg-gradient-to-r from-sky-500 via-violet-500 to-cyan-500"
            style={{ width: `${timeCrystal.coherence * 100}%` }}
            animate={{
              opacity: [0.7, 1, 0.7],
            }}
            transition={{ duration: 2, repeat: Infinity }}
          />
        </div>
      </motion.div>
    </div>
  );
}
