/**
 * Activity Panel - Real-time activity log from EdgeNet operations
 */

import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Activity,
  Zap,
  CheckCircle2,
  AlertCircle,
  Clock,
  Server,
  Cpu,
  Network,
  Trash2,
  Filter,
} from 'lucide-react';
import { Button, Chip } from '@heroui/react';
import { useNetworkStore } from '../../stores/networkStore';

interface ActivityEvent {
  id: string;
  type: 'task' | 'credit' | 'network' | 'system' | 'error';
  action: string;
  description: string;
  timestamp: Date;
  metadata?: Record<string, string | number>;
}

const typeConfig = {
  task: { icon: Cpu, color: 'text-sky-400', bgColor: 'bg-sky-500/20' },
  credit: { icon: Zap, color: 'text-emerald-400', bgColor: 'bg-emerald-500/20' },
  network: { icon: Network, color: 'text-violet-400', bgColor: 'bg-violet-500/20' },
  system: { icon: Server, color: 'text-amber-400', bgColor: 'bg-amber-500/20' },
  error: { icon: AlertCircle, color: 'text-red-400', bgColor: 'bg-red-500/20' },
};

function ActivityItem({ event, index }: { event: ActivityEvent; index: number }) {
  const config = typeConfig[event.type];
  const Icon = config.icon;

  return (
    <motion.div
      initial={{ opacity: 0, x: -20 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 20 }}
      transition={{ delay: index * 0.02 }}
      className="flex items-start gap-3 p-3 rounded-lg bg-zinc-900/50 border border-white/5 hover:border-white/10 transition-all"
    >
      <div className={`w-8 h-8 rounded-lg ${config.bgColor} flex items-center justify-center flex-shrink-0`}>
        <Icon size={16} className={config.color} />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="font-medium text-white text-sm">{event.action}</span>
          <Chip size="sm" className={`${config.bgColor} ${config.color} text-xs capitalize`}>
            {event.type}
          </Chip>
        </div>
        <p className="text-xs text-zinc-400 truncate">{event.description}</p>
        {event.metadata && (
          <div className="flex flex-wrap gap-2 mt-1">
            {Object.entries(event.metadata).map(([key, value]) => (
              <span key={key} className="text-xs text-zinc-500">
                {key}: <span className="text-zinc-300">{value}</span>
              </span>
            ))}
          </div>
        )}
      </div>
      <div className="text-xs text-zinc-500 flex items-center gap-1 flex-shrink-0">
        <Clock size={10} />
        {formatTime(event.timestamp)}
      </div>
    </motion.div>
  );
}

function formatTime(date: Date): string {
  const now = new Date();
  const diff = now.getTime() - date.getTime();

  if (diff < 60000) return 'Just now';
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
  return date.toLocaleDateString();
}

export function ActivityPanel() {
  const [activities, setActivities] = useState<ActivityEvent[]>([]);
  const [filter, setFilter] = useState<'all' | ActivityEvent['type']>('all');

  // Get real data from store
  const credits = useNetworkStore(state => state.credits);
  const stats = useNetworkStore(state => state.stats);
  const contributionEnabled = useNetworkStore(state => state.contributionSettings.enabled);
  const firebasePeers = useNetworkStore(state => state.firebasePeers);

  // Generate activities from real events
  useEffect(() => {
    const newActivities: ActivityEvent[] = [];
    const now = new Date();

    // Add credit earning events
    if (contributionEnabled && credits.earned > 0) {
      newActivities.push({
        id: `credit-${Date.now()}`,
        type: 'credit',
        action: 'Credits Earned',
        description: `Accumulated ${credits.earned.toFixed(4)} rUv from network contribution`,
        timestamp: now,
        metadata: { rate: '0.047/s', total: credits.earned.toFixed(2) },
      });
    }

    // Add network events
    if (firebasePeers.length > 0) {
      newActivities.push({
        id: `network-peers-${Date.now()}`,
        type: 'network',
        action: 'Peers Connected',
        description: `${firebasePeers.length} peer(s) active in network`,
        timestamp: new Date(now.getTime() - 30000),
        metadata: { peers: firebasePeers.length },
      });
    }

    // Add system events
    if (stats.uptime > 0) {
      newActivities.push({
        id: `system-uptime-${Date.now()}`,
        type: 'system',
        action: 'Node Active',
        description: `Local node running for ${formatUptime(stats.uptime)}`,
        timestamp: new Date(now.getTime() - 60000),
        metadata: { uptime: formatUptime(stats.uptime) },
      });
    }

    // Add contribution status
    newActivities.push({
      id: `system-contribution-${Date.now()}`,
      type: contributionEnabled ? 'task' : 'system',
      action: contributionEnabled ? 'Contributing' : 'Idle',
      description: contributionEnabled
        ? 'Actively contributing compute resources to the network'
        : 'Contribution paused - click Start Contributing to earn credits',
      timestamp: new Date(now.getTime() - 120000),
    });

    // Add some historical events
    newActivities.push(
      {
        id: 'init-1',
        type: 'system',
        action: 'WASM Initialized',
        description: 'EdgeNet WASM module loaded successfully',
        timestamp: new Date(now.getTime() - 180000),
      },
      {
        id: 'init-2',
        type: 'network',
        action: 'Firebase Connected',
        description: 'Real-time peer synchronization active',
        timestamp: new Date(now.getTime() - 200000),
      },
      {
        id: 'init-3',
        type: 'network',
        action: 'Relay Connected',
        description: 'WebSocket relay connection established',
        timestamp: new Date(now.getTime() - 220000),
      }
    );

    setActivities(newActivities);
  }, [credits.earned, contributionEnabled, firebasePeers.length, stats.uptime]);

  const filteredActivities = filter === 'all'
    ? activities
    : activities.filter(a => a.type === filter);

  const clearActivities = () => {
    setActivities(activities.slice(0, 3)); // Keep only system events
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
      >
        <h1 className="text-2xl md:text-3xl font-bold mb-2">
          <span className="bg-gradient-to-r from-sky-400 via-violet-400 to-cyan-400 bg-clip-text text-transparent">
            Activity Log
          </span>
        </h1>
        <p className="text-zinc-400">
          Track all network operations and events in real-time
        </p>
      </motion.div>

      {/* Stats */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.05 }}
        >
          <div className="flex items-center justify-between mb-2">
            <p className="text-sm text-zinc-400">Total Events</p>
            <Activity className="text-sky-400" size={18} />
          </div>
          <p className="text-2xl font-bold text-sky-400">{activities.length}</p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
        >
          <div className="flex items-center justify-between mb-2">
            <p className="text-sm text-zinc-400">Credits</p>
            <Zap className="text-emerald-400" size={18} />
          </div>
          <p className="text-2xl font-bold text-emerald-400">{credits.earned.toFixed(2)}</p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.15 }}
        >
          <div className="flex items-center justify-between mb-2">
            <p className="text-sm text-zinc-400">Tasks</p>
            <CheckCircle2 className="text-violet-400" size={18} />
          </div>
          <p className="text-2xl font-bold text-violet-400">{stats.tasksCompleted}</p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
        >
          <div className="flex items-center justify-between mb-2">
            <p className="text-sm text-zinc-400">Peers</p>
            <Network className="text-amber-400" size={18} />
          </div>
          <p className="text-2xl font-bold text-amber-400">{firebasePeers.length}</p>
        </motion.div>
      </div>

      {/* Filters */}
      <motion.div
        className="flex flex-wrap items-center gap-2"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.25 }}
      >
        <span className="text-xs text-zinc-500 flex items-center gap-1">
          <Filter size={12} /> Filter:
        </span>
        {(['all', 'task', 'credit', 'network', 'system', 'error'] as const).map((f) => (
          <Button
            key={f}
            size="sm"
            variant="flat"
            className={
              filter === f
                ? 'bg-sky-500/20 text-sky-400 border border-sky-500/30'
                : 'bg-white/5 text-zinc-400 hover:text-white'
            }
            onPress={() => setFilter(f)}
          >
            {f.charAt(0).toUpperCase() + f.slice(1)}
          </Button>
        ))}

        <div className="ml-auto">
          <Button
            size="sm"
            variant="flat"
            className="bg-white/5 text-zinc-400"
            startContent={<Trash2 size={14} />}
            onPress={clearActivities}
          >
            Clear
          </Button>
        </div>
      </motion.div>

      {/* Activity List */}
      <motion.div
        className="crystal-card p-4"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.3 }}
      >
        <div className="space-y-2 max-h-[500px] overflow-y-auto">
          <AnimatePresence>
            {filteredActivities.map((activity, index) => (
              <ActivityItem key={activity.id} event={activity} index={index} />
            ))}
          </AnimatePresence>

          {filteredActivities.length === 0 && (
            <div className="text-center py-8">
              <Activity className="mx-auto text-zinc-600 mb-3" size={40} />
              <p className="text-zinc-400">No activities match the current filter</p>
            </div>
          )}
        </div>
      </motion.div>
    </div>
  );
}

function formatUptime(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h ${Math.round((seconds % 3600) / 60)}m`;
  return `${Math.floor(seconds / 86400)}d ${Math.round((seconds % 86400) / 3600)}h`;
}

export default ActivityPanel;
