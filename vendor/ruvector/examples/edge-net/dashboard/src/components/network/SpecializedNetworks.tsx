import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Microscope,
  Radio,
  TrendingUp,
  Brain,
  Gamepad2,
  Users,
  Server,
  Zap,
  Clock,
  Award,
  CheckCircle,
  XCircle,
  Loader2,
  ChevronRight,
  X,
  Globe,
} from 'lucide-react';
import type { SpecializedNetwork } from '../../types';
import { useNetworkStore } from '../../stores/networkStore';

// Relay endpoint for real stats
const RELAY_URL = 'https://edge-net-relay-875130704813.us-central1.run.app';

// Relay stats interface
interface RelayStats {
  nodes: number;
  uptime: number;
  tasks: number;
  connectedNodes: string[];
}

// Fetch real network stats from relay
async function fetchRelayStats(): Promise<RelayStats> {
  try {
    const response = await fetch(`${RELAY_URL}/stats`);
    if (!response.ok) throw new Error('Failed to fetch');
    const data = await response.json();
    return {
      nodes: data.activeNodes || 0,
      uptime: data.uptime || 0,
      tasks: data.totalTasks || 0,
      connectedNodes: data.connectedNodes || [],
    };
  } catch {
    return { nodes: 0, uptime: 0, tasks: 0, connectedNodes: [] };
  }
}

// Real network - Edge-Net Genesis (the only real one)
function createRealNetwork(relayStats: { nodes: number; uptime: number; tasks: number }): SpecializedNetwork {
  const uptimePercent = relayStats.uptime > 0 ? Math.min(100, (relayStats.uptime / (24 * 60 * 60 * 1000)) * 100) : 0;
  return {
    id: 'edge-net-genesis',
    name: 'Edge-Net Genesis',
    description: 'The founding distributed compute network. Join to contribute idle CPU cycles and earn rUv credits.',
    category: 'compute',
    icon: 'globe',
    color: 'sky',
    stats: {
      nodes: relayStats.nodes,
      compute: relayStats.nodes * 0.5, // Estimate 0.5 TFLOPS per node
      tasks: relayStats.tasks,
      uptime: Number(uptimePercent.toFixed(1)),
    },
    requirements: { minCompute: 0.1, minBandwidth: 5, capabilities: ['compute'] },
    rewards: { baseRate: 1.0, bonusMultiplier: 1.0 },
    status: 'active',
    joined: false,
  };
}

// Planned networks - clearly marked as "Coming Soon"
const PLANNED_NETWORKS: SpecializedNetwork[] = [
  {
    id: 'medical-research',
    name: 'MedGrid',
    description: 'Planned: Distributed medical research computing for drug discovery and genomics analysis.',
    category: 'healthcare',
    icon: 'microscope',
    color: 'rose',
    stats: { nodes: 0, compute: 0, tasks: 0, uptime: 0 },
    requirements: { minCompute: 0.5, minBandwidth: 10, capabilities: ['compute', 'storage'] },
    rewards: { baseRate: 2.5, bonusMultiplier: 1.5 },
    status: 'launching',
    joined: false,
  },
  {
    id: 'seti-search',
    name: 'SETI@Edge',
    description: 'Planned: Search for extraterrestrial intelligence by analyzing radio telescope data.',
    category: 'science',
    icon: 'radio',
    color: 'violet',
    stats: { nodes: 0, compute: 0, tasks: 0, uptime: 0 },
    requirements: { minCompute: 0.2, minBandwidth: 5, capabilities: ['compute'] },
    rewards: { baseRate: 1.0, bonusMultiplier: 1.2 },
    status: 'launching',
    joined: false,
  },
  {
    id: 'ai-training',
    name: 'NeuralMesh',
    description: 'Planned: Distributed AI model training for open-source machine learning projects.',
    category: 'ai',
    icon: 'brain',
    color: 'amber',
    stats: { nodes: 0, compute: 0, tasks: 0, uptime: 0 },
    requirements: { minCompute: 2.0, minBandwidth: 50, capabilities: ['compute', 'storage'] },
    rewards: { baseRate: 3.5, bonusMultiplier: 1.8 },
    status: 'launching',
    joined: false,
  },
  {
    id: 'game-rendering',
    name: 'CloudPlay',
    description: 'Planned: Cloud gaming infrastructure for low-latency game streaming.',
    category: 'gaming',
    icon: 'gamepad',
    color: 'emerald',
    stats: { nodes: 0, compute: 0, tasks: 0, uptime: 0 },
    requirements: { minCompute: 1.5, minBandwidth: 200, capabilities: ['compute', 'relay'] },
    rewards: { baseRate: 4.0, bonusMultiplier: 1.6 },
    status: 'launching',
    joined: false,
  },
];

const iconMap: Record<string, React.ReactNode> = {
  microscope: <Microscope size={24} />,
  radio: <Radio size={24} />,
  trending: <TrendingUp size={24} />,
  brain: <Brain size={24} />,
  gamepad: <Gamepad2 size={24} />,
  users: <Users size={24} />,
  globe: <Globe size={24} />,
};

const colorMap: Record<string, { bg: string; border: string; text: string; glow: string }> = {
  rose: { bg: 'bg-rose-500/10', border: 'border-rose-500/30', text: 'text-rose-400', glow: 'shadow-rose-500/20' },
  violet: { bg: 'bg-violet-500/10', border: 'border-violet-500/30', text: 'text-violet-400', glow: 'shadow-violet-500/20' },
  emerald: { bg: 'bg-emerald-500/10', border: 'border-emerald-500/30', text: 'text-emerald-400', glow: 'shadow-emerald-500/20' },
  amber: { bg: 'bg-amber-500/10', border: 'border-amber-500/30', text: 'text-amber-400', glow: 'shadow-amber-500/20' },
  sky: { bg: 'bg-sky-500/10', border: 'border-sky-500/30', text: 'text-sky-400', glow: 'shadow-sky-500/20' },
  cyan: { bg: 'bg-cyan-500/10', border: 'border-cyan-500/30', text: 'text-cyan-400', glow: 'shadow-cyan-500/20' },
};

interface NetworkCardProps {
  network: SpecializedNetwork;
  onJoin: (id: string) => void;
  onLeave: (id: string) => void;
  onViewDetails: (network: SpecializedNetwork) => void;
}

function NetworkCard({ network, onJoin, onLeave, onViewDetails }: NetworkCardProps) {
  const [isJoining, setIsJoining] = useState(false);
  const colors = colorMap[network.color] || colorMap.sky;

  const handleJoinToggle = async () => {
    setIsJoining(true);
    await new Promise((r) => setTimeout(r, 1000));
    if (network.joined) {
      onLeave(network.id);
    } else {
      onJoin(network.id);
    }
    setIsJoining(false);
  };

  const statusBadge = {
    active: { label: 'Active', color: 'bg-emerald-500/20 text-emerald-400' },
    maintenance: { label: 'Maintenance', color: 'bg-amber-500/20 text-amber-400' },
    launching: { label: 'Coming Soon', color: 'bg-violet-500/20 text-violet-400' },
    closed: { label: 'Closed', color: 'bg-zinc-500/20 text-zinc-400' },
  }[network.status];

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className={`crystal-card p-5 ${network.joined ? `shadow-lg ${colors.glow}` : ''}`}
    >
      {/* Header */}
      <div className="flex items-start justify-between mb-4">
        <div className="flex items-center gap-3">
          <div className={`p-3 rounded-xl ${colors.bg} ${colors.border} border ${colors.text}`}>
            {iconMap[network.icon]}
          </div>
          <div>
            <h3 className="font-semibold text-white flex items-center gap-2">
              {network.name}
              {network.joined && <CheckCircle size={16} className="text-emerald-400" />}
            </h3>
            <span className={`text-xs px-2 py-0.5 rounded-full ${statusBadge.color}`}>
              {statusBadge.label}
            </span>
          </div>
        </div>
      </div>

      {/* Description */}
      <p className="text-sm text-zinc-400 mb-4 line-clamp-2">{network.description}</p>

      {/* Stats Grid */}
      <div className="grid grid-cols-2 gap-3 mb-4">
        <div className="flex items-center gap-2 text-sm">
          <Server size={14} className="text-zinc-500" />
          <span className="text-zinc-400">{network.stats.nodes.toLocaleString()} nodes</span>
        </div>
        <div className="flex items-center gap-2 text-sm">
          <Zap size={14} className="text-zinc-500" />
          <span className="text-zinc-400">{network.stats.compute.toFixed(1)} TFLOPS</span>
        </div>
        <div className="flex items-center gap-2 text-sm">
          <Clock size={14} className="text-zinc-500" />
          <span className="text-zinc-400">{network.stats.uptime}% uptime</span>
        </div>
        <div className="flex items-center gap-2 text-sm">
          <Award size={14} className={colors.text} />
          <span className={colors.text}>{network.rewards.baseRate} cr/hr</span>
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-2">
        <button
          onClick={handleJoinToggle}
          disabled={isJoining || network.status === 'closed' || network.status === 'launching'}
          className={`flex-1 h-9 rounded-lg font-medium text-sm flex items-center justify-center gap-2 transition-all
            ${network.joined
              ? 'bg-zinc-700 hover:bg-zinc-600 text-white'
              : `${colors.bg} ${colors.border} border ${colors.text} hover:bg-opacity-20`
            }
            disabled:opacity-50 disabled:cursor-not-allowed
          `}
        >
          {isJoining ? (
            <Loader2 size={16} className="animate-spin" />
          ) : network.joined ? (
            <>
              <XCircle size={16} /> Leave
            </>
          ) : (
            <>
              <CheckCircle size={16} /> Join
            </>
          )}
        </button>
        <button
          onClick={() => onViewDetails(network)}
          className="h-9 px-3 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 transition-colors"
        >
          <ChevronRight size={16} className="text-zinc-400" />
        </button>
      </div>
    </motion.div>
  );
}

interface NetworkDetailsModalProps {
  network: SpecializedNetwork | null;
  onClose: () => void;
  onJoin: (id: string) => void;
  onLeave: (id: string) => void;
}

function NetworkDetailsModal({ network, onClose, onJoin, onLeave }: NetworkDetailsModalProps) {
  if (!network) return null;
  const colors = colorMap[network.color] || colorMap.sky;

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      onClick={onClose}
    >
      <motion.div
        initial={{ scale: 0.95, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.95, opacity: 0 }}
        className="bg-zinc-900 border border-white/10 rounded-xl max-w-lg w-full max-h-[80vh] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className={`p-6 ${colors.bg} border-b ${colors.border}`}>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <div className={`p-4 rounded-xl bg-black/20 ${colors.text}`}>
                {iconMap[network.icon]}
              </div>
              <div>
                <h2 className="text-xl font-bold text-white">{network.name}</h2>
                <p className="text-sm text-zinc-400">{network.category.charAt(0).toUpperCase() + network.category.slice(1)} Network</p>
              </div>
            </div>
            <button onClick={onClose} className="p-2 hover:bg-white/10 rounded-lg transition-colors">
              <X size={20} className="text-zinc-400" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 overflow-auto max-h-[50vh]">
          <div>
            <h3 className="text-sm font-medium text-zinc-400 mb-2">About</h3>
            <p className="text-white">{network.description}</p>
          </div>

          <div>
            <h3 className="text-sm font-medium text-zinc-400 mb-3">Network Statistics</h3>
            <div className="grid grid-cols-2 gap-4">
              <div className="p-3 bg-white/5 rounded-lg">
                <p className="text-2xl font-bold text-white">{network.stats.nodes.toLocaleString()}</p>
                <p className="text-xs text-zinc-400">Active Nodes</p>
              </div>
              <div className="p-3 bg-white/5 rounded-lg">
                <p className="text-2xl font-bold text-white">{network.stats.compute.toFixed(1)}</p>
                <p className="text-xs text-zinc-400">Total TFLOPS</p>
              </div>
              <div className="p-3 bg-white/5 rounded-lg">
                <p className="text-2xl font-bold text-white">{network.stats.tasks.toLocaleString()}</p>
                <p className="text-xs text-zinc-400">Tasks Completed</p>
              </div>
              <div className="p-3 bg-white/5 rounded-lg">
                <p className="text-2xl font-bold text-white">{network.stats.uptime}%</p>
                <p className="text-xs text-zinc-400">Network Uptime</p>
              </div>
            </div>
          </div>

          <div>
            <h3 className="text-sm font-medium text-zinc-400 mb-3">Requirements</h3>
            <div className="space-y-2">
              <div className="flex justify-between text-sm">
                <span className="text-zinc-400">Minimum Compute</span>
                <span className="text-white">{network.requirements.minCompute} TFLOPS</span>
              </div>
              <div className="flex justify-between text-sm">
                <span className="text-zinc-400">Minimum Bandwidth</span>
                <span className="text-white">{network.requirements.minBandwidth} Mbps</span>
              </div>
              <div className="flex justify-between text-sm">
                <span className="text-zinc-400">Required Capabilities</span>
                <span className="text-white">{network.requirements.capabilities.join(', ')}</span>
              </div>
            </div>
          </div>

          <div>
            <h3 className="text-sm font-medium text-zinc-400 mb-3">Rewards</h3>
            <div className="p-4 bg-gradient-to-r from-amber-500/10 to-orange-500/10 border border-amber-500/30 rounded-lg">
              <div className="flex items-center justify-between mb-2">
                <span className="text-amber-400 font-medium">Base Rate</span>
                <span className="text-xl font-bold text-white">{network.rewards.baseRate} credits/hour</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-amber-400 font-medium">Bonus Multiplier</span>
                <span className="text-lg font-semibold text-white">{network.rewards.bonusMultiplier}x</span>
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-white/10">
          <button
            onClick={() => {
              network.joined ? onLeave(network.id) : onJoin(network.id);
              onClose();
            }}
            disabled={network.status === 'closed' || network.status === 'launching'}
            className={`w-full h-11 rounded-lg font-medium flex items-center justify-center gap-2 transition-all
              ${network.joined
                ? 'bg-zinc-700 hover:bg-zinc-600 text-white'
                : `bg-gradient-to-r from-sky-500 to-violet-500 text-white hover:opacity-90`
              }
              disabled:opacity-50 disabled:cursor-not-allowed
            `}
          >
            {network.joined ? (
              <>
                <XCircle size={18} /> Leave Network
              </>
            ) : (
              <>
                <CheckCircle size={18} /> Join Network
              </>
            )}
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

// Persist joined networks to localStorage
const STORAGE_KEY = 'edge-net-joined-networks';

function loadJoinedIds(): Set<string> {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved ? new Set(JSON.parse(saved)) : new Set();
  } catch {
    return new Set();
  }
}

function saveJoinedIds(ids: Set<string>) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify([...ids]));
}

export function SpecializedNetworks() {
  const [networks, setNetworks] = useState<SpecializedNetwork[]>([]);
  const [selectedNetwork, setSelectedNetwork] = useState<SpecializedNetwork | null>(null);
  const [filter, setFilter] = useState<string>('all');
  const [isLoading, setIsLoading] = useState(true);
  const [joinedIds, setJoinedIds] = useState<Set<string>>(loadJoinedIds);

  // Connect to the network store for real contribution
  const { contributionSettings, startContributing, stopContributing, giveConsent } = useNetworkStore();

  // Sync join status with contribution status
  useEffect(() => {
    if (contributionSettings.enabled && !joinedIds.has('edge-net-genesis')) {
      const newJoinedIds = new Set(joinedIds);
      newJoinedIds.add('edge-net-genesis');
      setJoinedIds(newJoinedIds);
      saveJoinedIds(newJoinedIds);
    }
  }, [contributionSettings.enabled, joinedIds]);

  // Fetch real stats on mount and periodically
  useEffect(() => {
    const loadRealStats = async () => {
      const relayStats = await fetchRelayStats();
      const realNetwork = createRealNetwork(relayStats);
      const allNetworks = [realNetwork, ...PLANNED_NETWORKS];

      // Apply persisted join status, but Edge-Net Genesis follows contribution status
      setNetworks(allNetworks.map(n => ({
        ...n,
        joined: n.id === 'edge-net-genesis'
          ? contributionSettings.enabled || joinedIds.has(n.id)
          : joinedIds.has(n.id),
      })));
      setIsLoading(false);
    };

    loadRealStats();
    const interval = setInterval(loadRealStats, 10000); // Refresh every 10s
    return () => clearInterval(interval);
  }, [joinedIds, contributionSettings.enabled]);

  const handleJoin = (id: string) => {
    const newJoinedIds = new Set(joinedIds);
    newJoinedIds.add(id);
    setJoinedIds(newJoinedIds);
    saveJoinedIds(newJoinedIds);
    setNetworks((prev) =>
      prev.map((n) => (n.id === id ? { ...n, joined: true, joinedAt: new Date() } : n))
    );

    // For Edge-Net Genesis, actually start contributing to the network
    if (id === 'edge-net-genesis') {
      if (!contributionSettings.consentGiven) {
        giveConsent();
      }
      startContributing();
      console.log('[Networks] Joined Edge-Net Genesis - started contributing');
    }
  };

  const handleLeave = (id: string) => {
    const newJoinedIds = new Set(joinedIds);
    newJoinedIds.delete(id);
    setJoinedIds(newJoinedIds);
    saveJoinedIds(newJoinedIds);
    setNetworks((prev) =>
      prev.map((n) => (n.id === id ? { ...n, joined: false, joinedAt: undefined } : n))
    );

    // For Edge-Net Genesis, stop contributing
    if (id === 'edge-net-genesis') {
      stopContributing();
      console.log('[Networks] Left Edge-Net Genesis - stopped contributing');
    }
  };

  const categories = ['all', 'compute', 'science', 'healthcare', 'ai', 'gaming'];
  const filteredNetworks = filter === 'all'
    ? networks
    : networks.filter((n) => n.category === filter);

  const joinedCount = networks.filter((n) => n.joined).length;
  const totalEarnings = networks
    .filter((n) => n.joined)
    .reduce((sum, n) => sum + n.rewards.baseRate, 0);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="w-8 h-8 animate-spin text-sky-400" />
        <span className="ml-3 text-zinc-400">Fetching network data...</span>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Summary */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="crystal-card p-4"
        >
          <p className="text-sm text-zinc-400 mb-1">Joined Networks</p>
          <p className="text-2xl font-bold text-white">{joinedCount}</p>
        </motion.div>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
          className="crystal-card p-4"
        >
          <p className="text-sm text-zinc-400 mb-1">Available Networks</p>
          <p className="text-2xl font-bold text-white">{networks.filter((n) => n.status === 'active').length}</p>
        </motion.div>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
          className="crystal-card p-4"
        >
          <p className="text-sm text-zinc-400 mb-1">Potential Earnings</p>
          <p className="text-2xl font-bold text-amber-400">{totalEarnings.toFixed(1)} cr/hr</p>
        </motion.div>
      </div>

      {/* Filter */}
      <div className="flex gap-2 overflow-x-auto pb-2">
        {categories.map((cat) => (
          <button
            key={cat}
            onClick={() => setFilter(cat)}
            className={`px-4 py-2 rounded-lg text-sm font-medium whitespace-nowrap transition-all
              ${filter === cat
                ? 'bg-sky-500/20 text-sky-400 border border-sky-500/30'
                : 'bg-white/5 text-zinc-400 hover:bg-white/10 border border-transparent'
              }
            `}
          >
            {cat.charAt(0).toUpperCase() + cat.slice(1)}
          </button>
        ))}
      </div>

      {/* Network Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {filteredNetworks.map((network) => (
          <NetworkCard
            key={network.id}
            network={network}
            onJoin={handleJoin}
            onLeave={handleLeave}
            onViewDetails={setSelectedNetwork}
          />
        ))}
      </div>

      {/* Details Modal */}
      <AnimatePresence>
        {selectedNetwork && (
          <NetworkDetailsModal
            network={selectedNetwork}
            onClose={() => setSelectedNetwork(null)}
            onJoin={handleJoin}
            onLeave={handleLeave}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
