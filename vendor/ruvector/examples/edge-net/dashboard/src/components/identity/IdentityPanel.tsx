import { useState } from 'react';
import { Button, Card, CardBody, Input } from '@heroui/react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  User,
  Key,
  Shield,
  Copy,
  Check,
  Download,
  Upload,
  Trash2,
  Network,
  Plus,
  X,
  Zap,
  HardDrive,
  Cpu,
  Globe,
  Star,
  AlertCircle,
} from 'lucide-react';
import { useIdentityStore, availableNetworks } from '../../stores/identityStore';

const capabilityIcons: Record<string, React.ReactNode> = {
  compute: <Cpu size={14} />,
  storage: <HardDrive size={14} />,
  relay: <Network size={14} />,
  validation: <Shield size={14} />,
};

const capabilityDescriptions: Record<string, string> = {
  compute: 'Contribute CPU/GPU compute power',
  storage: 'Provide distributed storage',
  relay: 'Act as a network relay node',
  validation: 'Validate transactions and results',
};

function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button
      onClick={copy}
      className={`
        flex items-center gap-1.5 px-2 py-1 rounded text-xs
        transition-all border
        ${copied
          ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400'
          : 'bg-zinc-800 border-white/10 text-zinc-400 hover:text-white hover:border-white/20'
        }
      `}
    >
      {copied ? <Check size={12} /> : <Copy size={12} />}
      {label}
    </button>
  );
}

function GenerateIdentityCard() {
  const { generateIdentity, importIdentity, isGenerating, error } = useIdentityStore();
  const [displayName, setDisplayName] = useState('');
  const [showImport, setShowImport] = useState(false);
  const [importKey, setImportKey] = useState('');

  const handleGenerate = () => {
    if (displayName.trim()) {
      generateIdentity(displayName.trim());
    }
  };

  const handleImport = () => {
    if (importKey.trim()) {
      importIdentity(importKey.trim());
      setImportKey('');
      setShowImport(false);
    }
  };

  return (
    <Card className="bg-zinc-900/50 border border-white/10">
      <CardBody className="p-6">
        <div className="text-center mb-6">
          <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gradient-to-br from-sky-500 to-violet-500 flex items-center justify-center">
            <Key size={32} className="text-white" />
          </div>
          <h2 className="text-xl font-semibold text-white">Create Your Identity</h2>
          <p className="text-sm text-zinc-400 mt-1">
            Generate a cryptographic identity to participate in Edge-Net
          </p>
        </div>

        {error && (
          <div className="mb-4 p-3 rounded-lg bg-red-500/10 border border-red-500/30 flex items-center gap-2 text-red-400 text-sm">
            <AlertCircle size={16} />
            {error}
          </div>
        )}

        {!showImport ? (
          <div className="space-y-4">
            <div>
              <label className="text-sm text-zinc-400 mb-1 block">Display Name</label>
              <Input
                placeholder="Enter your display name"
                value={displayName}
                onValueChange={setDisplayName}
                classNames={{
                  input: 'bg-zinc-800 text-white',
                  inputWrapper: 'bg-zinc-800 border-white/10 hover:border-white/20',
                }}
              />
            </div>

            <Button
              className="w-full bg-gradient-to-r from-sky-500 to-violet-500 text-white"
              isLoading={isGenerating}
              isDisabled={!displayName.trim()}
              onPress={handleGenerate}
            >
              <Key size={16} />
              Generate Identity
            </Button>

            <div className="text-center">
              <button
                onClick={() => setShowImport(true)}
                className="text-sm text-zinc-500 hover:text-zinc-300"
              >
                Or import existing identity
              </button>
            </div>
          </div>
        ) : (
          <div className="space-y-4">
            <div>
              <label className="text-sm text-zinc-400 mb-1 block">Private Key</label>
              <Input
                placeholder="Paste your private key (64 hex chars)"
                value={importKey}
                onValueChange={setImportKey}
                type="password"
                classNames={{
                  input: 'bg-zinc-800 text-white font-mono',
                  inputWrapper: 'bg-zinc-800 border-white/10 hover:border-white/20',
                }}
              />
            </div>

            <div className="flex gap-2">
              <Button
                className="flex-1"
                variant="flat"
                onPress={() => setShowImport(false)}
              >
                Cancel
              </Button>
              <Button
                className="flex-1 bg-sky-500/20 text-sky-400"
                isLoading={isGenerating}
                isDisabled={!importKey.trim()}
                onPress={handleImport}
              >
                <Upload size={16} />
                Import
              </Button>
            </div>
          </div>
        )}
      </CardBody>
    </Card>
  );
}

function IdentityCard() {
  const { identity, exportIdentity, clearIdentity } = useIdentityStore();
  const [showConfirmClear, setShowConfirmClear] = useState(false);

  if (!identity) return null;

  const handleExport = async () => {
    // For now, export without encryption (password prompt can be added later)
    const exported = await exportIdentity('');
    if (exported) {
      const blob = new Blob([exported], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `edge-net-identity-${identity.id.substring(0, 8)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    }
  };

  return (
    <Card className="bg-zinc-900/50 border border-emerald-500/30">
      <CardBody className="p-4">
        <div className="flex items-start justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="w-12 h-12 rounded-full bg-gradient-to-br from-emerald-500 to-cyan-500 flex items-center justify-center">
              <User size={24} className="text-white" />
            </div>
            <div>
              <h3 className="font-semibold text-white">{identity.displayName}</h3>
              <p className="text-xs text-zinc-500">
                Created {new Date(identity.createdAt).toLocaleDateString()}
              </p>
            </div>
          </div>
          <span className="px-2 py-1 rounded text-xs bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
            Active
          </span>
        </div>

        {/* Peer ID */}
        <div className="mb-3">
          <label className="text-xs text-zinc-500 mb-1 block">Peer ID</label>
          <div className="flex items-center gap-2">
            <code className="flex-1 bg-zinc-950 border border-white/10 rounded px-2 py-1.5 text-xs text-zinc-300 font-mono truncate">
              {identity.id}
            </code>
            <CopyButton text={identity.id} label="Copy" />
          </div>
        </div>

        {/* Public Key */}
        <div className="mb-4">
          <label className="text-xs text-zinc-500 mb-1 block">Public Key</label>
          <div className="flex items-center gap-2">
            <code className="flex-1 bg-zinc-950 border border-white/10 rounded px-2 py-1.5 text-xs text-zinc-300 font-mono truncate">
              {identity.publicKey}
            </code>
            <CopyButton text={identity.publicKey} label="Copy" />
          </div>
        </div>

        {/* Actions */}
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="flat"
            className="flex-1 bg-sky-500/20 text-sky-400"
            onPress={handleExport}
          >
            <Download size={14} />
            Export
          </Button>

          {!showConfirmClear ? (
            <Button
              size="sm"
              variant="flat"
              className="bg-red-500/10 text-red-400"
              onPress={() => setShowConfirmClear(true)}
            >
              <Trash2 size={14} />
            </Button>
          ) : (
            <div className="flex gap-1">
              <Button
                size="sm"
                variant="flat"
                onPress={() => setShowConfirmClear(false)}
              >
                Cancel
              </Button>
              <Button
                size="sm"
                variant="flat"
                className="bg-red-500/20 text-red-400"
                onPress={() => {
                  clearIdentity();
                  setShowConfirmClear(false);
                }}
              >
                Confirm Delete
              </Button>
            </div>
          )}
        </div>
      </CardBody>
    </Card>
  );
}

function NetworkRegistrationModal({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const { registrations, registerNetwork, isRegistering } = useIdentityStore();
  const [selectedNetwork, setSelectedNetwork] = useState<string | null>(null);
  const [selectedCapabilities, setSelectedCapabilities] = useState<string[]>(['compute']);

  const unregisteredNetworks = availableNetworks.filter(
    n => !registrations.some(r => r.networkId === n.id)
  );

  const handleRegister = async () => {
    if (selectedNetwork) {
      await registerNetwork(selectedNetwork, selectedCapabilities);
      onClose();
      setSelectedNetwork(null);
      setSelectedCapabilities(['compute']);
    }
  };

  const toggleCapability = (cap: string) => {
    setSelectedCapabilities(prev =>
      prev.includes(cap)
        ? prev.filter(c => c !== cap)
        : [...prev, cap]
    );
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 bg-black/70 backdrop-blur-sm z-50"
            onClick={onClose}
          />

          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-[500px] max-w-[90vw] bg-zinc-900 border border-white/10 rounded-xl z-50 overflow-hidden"
          >
            {/* Header */}
            <div className="flex items-center justify-between p-4 border-b border-white/10">
              <div className="flex items-center gap-3">
                <Globe className="text-sky-400" size={20} />
                <h2 className="font-semibold text-white">Join Network</h2>
              </div>
              <button
                onClick={onClose}
                className="p-2 rounded-lg hover:bg-white/5 text-zinc-400 hover:text-white"
              >
                <X size={20} />
              </button>
            </div>

            {/* Content */}
            <div className="p-4 space-y-4">
              {/* Network Selection */}
              <div>
                <label className="text-sm text-zinc-400 mb-2 block">Select Network</label>
                <div className="space-y-2">
                  {unregisteredNetworks.map(network => (
                    <button
                      key={network.id}
                      onClick={() => setSelectedNetwork(network.id)}
                      className={`
                        w-full p-3 rounded-lg text-left transition-all border
                        ${selectedNetwork === network.id
                          ? 'bg-sky-500/20 border-sky-500/30'
                          : 'bg-zinc-800 border-white/10 hover:border-white/20'
                        }
                      `}
                    >
                      <div className="font-medium text-white">{network.name}</div>
                      <div className="text-xs text-zinc-500 mt-0.5">{network.description}</div>
                    </button>
                  ))}

                  {unregisteredNetworks.length === 0 && (
                    <p className="text-center text-zinc-500 py-4">
                      Already registered to all available networks
                    </p>
                  )}
                </div>
              </div>

              {/* Capabilities */}
              {selectedNetwork && (
                <div>
                  <label className="text-sm text-zinc-400 mb-2 block">Capabilities to Offer</label>
                  <div className="grid grid-cols-2 gap-2">
                    {Object.entries(capabilityDescriptions).map(([cap, desc]) => (
                      <button
                        key={cap}
                        onClick={() => toggleCapability(cap)}
                        className={`
                          p-3 rounded-lg text-left transition-all border
                          ${selectedCapabilities.includes(cap)
                            ? 'bg-emerald-500/20 border-emerald-500/30'
                            : 'bg-zinc-800 border-white/10 hover:border-white/20'
                          }
                        `}
                      >
                        <div className="flex items-center gap-2 mb-1">
                          {capabilityIcons[cap]}
                          <span className="font-medium text-white capitalize">{cap}</span>
                        </div>
                        <div className="text-xs text-zinc-500">{desc}</div>
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="flex justify-end gap-2 p-4 border-t border-white/10">
              <Button variant="flat" onPress={onClose}>
                Cancel
              </Button>
              <Button
                className="bg-sky-500 text-white"
                isLoading={isRegistering}
                isDisabled={!selectedNetwork || selectedCapabilities.length === 0}
                onPress={handleRegister}
              >
                <Plus size={16} />
                Join Network
              </Button>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

function NetworkCard({
  registration,
}: {
  registration: {
    networkId: string;
    networkName: string;
    status: string;
    joinedAt: Date;
    capabilities: string[];
    reputation: number;
    creditsEarned: number;
  };
}) {
  const { leaveNetwork } = useIdentityStore();
  const [showConfirmLeave, setShowConfirmLeave] = useState(false);

  return (
    <Card className="bg-zinc-900/50 border border-white/10">
      <CardBody className="p-4">
        <div className="flex items-start justify-between mb-3">
          <div>
            <h4 className="font-medium text-white">{registration.networkName}</h4>
            <p className="text-xs text-zinc-500">
              Joined {new Date(registration.joinedAt).toLocaleDateString()}
            </p>
          </div>
          <span
            className={`px-2 py-1 rounded text-xs ${
              registration.status === 'active'
                ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                : 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
            }`}
          >
            {registration.status}
          </span>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-2 gap-3 mb-3">
          <div className="bg-zinc-800 rounded p-2">
            <div className="flex items-center gap-1 text-xs text-zinc-500 mb-1">
              <Star size={12} />
              Reputation
            </div>
            <div className="text-lg font-semibold text-white">{registration.reputation}</div>
          </div>
          <div className="bg-zinc-800 rounded p-2">
            <div className="flex items-center gap-1 text-xs text-zinc-500 mb-1">
              <Zap size={12} />
              Credits
            </div>
            <div className="text-lg font-semibold text-emerald-400">
              {registration.creditsEarned.toFixed(2)}
            </div>
          </div>
        </div>

        {/* Capabilities */}
        <div className="mb-3">
          <label className="text-xs text-zinc-500 mb-1 block">Capabilities</label>
          <div className="flex flex-wrap gap-1">
            {registration.capabilities.map(cap => (
              <span
                key={cap}
                className="px-2 py-1 rounded text-xs bg-sky-500/20 text-sky-400 border border-sky-500/30 flex items-center gap-1"
              >
                {capabilityIcons[cap]}
                {cap}
              </span>
            ))}
          </div>
        </div>

        {/* Actions */}
        {!showConfirmLeave ? (
          <Button
            size="sm"
            variant="flat"
            className="w-full bg-red-500/10 text-red-400"
            onPress={() => setShowConfirmLeave(true)}
          >
            Leave Network
          </Button>
        ) : (
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="flat"
              className="flex-1"
              onPress={() => setShowConfirmLeave(false)}
            >
              Cancel
            </Button>
            <Button
              size="sm"
              variant="flat"
              className="flex-1 bg-red-500/20 text-red-400"
              onPress={() => {
                leaveNetwork(registration.networkId);
                setShowConfirmLeave(false);
              }}
            >
              Confirm Leave
            </Button>
          </div>
        )}
      </CardBody>
    </Card>
  );
}

export function IdentityPanel() {
  const { identity, registrations } = useIdentityStore();
  const [showRegisterModal, setShowRegisterModal] = useState(false);

  return (
    <div className="space-y-6">
      {/* Identity Section */}
      <div>
        <h2 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
          <Key size={20} className="text-sky-400" />
          Cryptographic Identity
        </h2>

        {!identity ? (
          <GenerateIdentityCard />
        ) : (
          <IdentityCard />
        )}
      </div>

      {/* Network Registrations */}
      {identity && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-white flex items-center gap-2">
              <Globe size={20} className="text-violet-400" />
              Network Registrations
            </h2>
            <Button
              size="sm"
              className="bg-sky-500/20 text-sky-400"
              onPress={() => setShowRegisterModal(true)}
            >
              <Plus size={16} />
              Join Network
            </Button>
          </div>

          {registrations.length === 0 ? (
            <Card className="bg-zinc-900/50 border border-white/10">
              <CardBody className="p-8 text-center">
                <Network size={48} className="mx-auto text-zinc-600 mb-4" />
                <h3 className="text-lg font-medium text-zinc-400 mb-2">No Networks Joined</h3>
                <p className="text-sm text-zinc-500 mb-4">
                  Join a network to start participating and earning credits
                </p>
                <Button
                  className="bg-sky-500 text-white"
                  onPress={() => setShowRegisterModal(true)}
                >
                  <Plus size={16} />
                  Join Your First Network
                </Button>
              </CardBody>
            </Card>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {registrations.map(reg => (
                <NetworkCard key={reg.networkId} registration={reg} />
              ))}
            </div>
          )}
        </motion.div>
      )}

      {/* Registration Modal */}
      <NetworkRegistrationModal
        isOpen={showRegisterModal}
        onClose={() => setShowRegisterModal(false)}
      />
    </div>
  );
}
