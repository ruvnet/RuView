/**
 * Settings Panel - Configuration for EdgeNet dashboard
 */

import { useState } from 'react';
import { motion } from 'framer-motion';
import {
  Cpu,
  Zap,
  Battery,
  Clock,
  Bell,
  Shield,
  Database,
  Globe,
  Save,
  Trash2,
  Download,
  Upload,
  AlertTriangle,
  Check,
} from 'lucide-react';
import { Button, Switch, Slider, Card, CardBody } from '@heroui/react';
import { useNetworkStore } from '../../stores/networkStore';

interface SettingsSection {
  id: string;
  title: string;
  icon: React.ReactNode;
  description: string;
}

const sections: SettingsSection[] = [
  { id: 'contribution', title: 'Contribution', icon: <Cpu size={20} />, description: 'Configure compute resource sharing' },
  { id: 'network', title: 'Network', icon: <Globe size={20} />, description: 'Network and relay settings' },
  { id: 'notifications', title: 'Notifications', icon: <Bell size={20} />, description: 'Alert and notification preferences' },
  { id: 'storage', title: 'Storage', icon: <Database size={20} />, description: 'Local data and cache management' },
  { id: 'security', title: 'Security', icon: <Shield size={20} />, description: 'Privacy and security options' },
];

export function SettingsPanel() {
  const [activeSection, setActiveSection] = useState('contribution');
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved'>('idle');

  // Get settings from store
  const {
    contributionSettings,
    setContributionSettings,
    clearLocalData,
  } = useNetworkStore();

  const handleSave = async () => {
    setSaveStatus('saving');
    await new Promise(resolve => setTimeout(resolve, 500));
    setSaveStatus('saved');
    setTimeout(() => setSaveStatus('idle'), 2000);
  };

  const handleClearData = () => {
    if (confirm('Are you sure you want to clear all local data? This cannot be undone.')) {
      clearLocalData();
      window.location.reload();
    }
  };

  const handleExportSettings = () => {
    const settings = {
      contribution: contributionSettings,
      exportedAt: new Date().toISOString(),
    };
    const blob = new Blob([JSON.stringify(settings, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'edge-net-settings.json';
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
      >
        <h1 className="text-2xl md:text-3xl font-bold mb-2">
          <span className="bg-gradient-to-r from-zinc-200 via-zinc-400 to-zinc-200 bg-clip-text text-transparent">
            Settings
          </span>
        </h1>
        <p className="text-zinc-400">
          Configure your Edge-Net dashboard preferences
        </p>
      </motion.div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* Sidebar */}
        <motion.div
          className="lg:col-span-1"
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: 0.1 }}
        >
          <div className="crystal-card p-2 space-y-1">
            {sections.map((section) => (
              <button
                key={section.id}
                onClick={() => setActiveSection(section.id)}
                className={`w-full flex items-center gap-3 p-3 rounded-lg transition-all text-left ${
                  activeSection === section.id
                    ? 'bg-sky-500/20 text-sky-400 border border-sky-500/30'
                    : 'text-zinc-400 hover:bg-white/5 hover:text-white'
                }`}
              >
                {section.icon}
                <div>
                  <p className="font-medium text-sm">{section.title}</p>
                  <p className="text-xs text-zinc-500 hidden md:block">{section.description}</p>
                </div>
              </button>
            ))}
          </div>
        </motion.div>

        {/* Content */}
        <motion.div
          className="lg:col-span-3"
          initial={{ opacity: 0, x: 20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: 0.2 }}
        >
          <Card className="bg-zinc-900/50 border border-white/10">
            <CardBody className="p-6">
              {activeSection === 'contribution' && (
                <div className="space-y-6">
                  <div>
                    <h3 className="text-lg font-semibold text-white mb-1">Contribution Settings</h3>
                    <p className="text-sm text-zinc-400">Control how your device contributes to the network</p>
                  </div>

                  <div className="space-y-4">
                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div className="flex items-center gap-3">
                        <Cpu className="text-sky-400" size={20} />
                        <div>
                          <p className="font-medium text-white">Enable Contribution</p>
                          <p className="text-xs text-zinc-400">Share compute resources with the network</p>
                        </div>
                      </div>
                      <Switch
                        isSelected={contributionSettings.enabled}
                        onValueChange={(value) => setContributionSettings({ enabled: value })}
                      />
                    </div>

                    <div className="p-4 bg-zinc-800/50 rounded-lg">
                      <div className="flex items-center justify-between mb-3">
                        <div className="flex items-center gap-3">
                          <Cpu className="text-sky-400" size={20} />
                          <div>
                            <p className="font-medium text-white">CPU Limit</p>
                            <p className="text-xs text-zinc-400">Maximum CPU usage for tasks</p>
                          </div>
                        </div>
                        <span className="text-sky-400 font-bold">{contributionSettings.cpuLimit}%</span>
                      </div>
                      <Slider
                        size="sm"
                        step={5}
                        minValue={10}
                        maxValue={80}
                        value={contributionSettings.cpuLimit}
                        onChange={(value) => setContributionSettings({ cpuLimit: value as number })}
                        classNames={{
                          track: 'bg-zinc-700',
                          filler: 'bg-gradient-to-r from-sky-500 to-cyan-500',
                        }}
                      />
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div className="flex items-center gap-3">
                        <Zap className="text-violet-400" size={20} />
                        <div>
                          <p className="font-medium text-white">GPU Acceleration</p>
                          <p className="text-xs text-zinc-400">Use GPU for compatible tasks</p>
                        </div>
                      </div>
                      <Switch
                        isSelected={contributionSettings.gpuEnabled}
                        onValueChange={(value) => setContributionSettings({ gpuEnabled: value })}
                      />
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div className="flex items-center gap-3">
                        <Battery className="text-emerald-400" size={20} />
                        <div>
                          <p className="font-medium text-white">Respect Battery</p>
                          <p className="text-xs text-zinc-400">Pause when on battery power</p>
                        </div>
                      </div>
                      <Switch
                        isSelected={contributionSettings.respectBattery}
                        onValueChange={(value) => setContributionSettings({ respectBattery: value })}
                      />
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div className="flex items-center gap-3">
                        <Clock className="text-amber-400" size={20} />
                        <div>
                          <p className="font-medium text-white">Only When Idle</p>
                          <p className="text-xs text-zinc-400">Contribute only when browser is idle</p>
                        </div>
                      </div>
                      <Switch
                        isSelected={contributionSettings.onlyWhenIdle}
                        onValueChange={(value) => setContributionSettings({ onlyWhenIdle: value })}
                      />
                    </div>
                  </div>
                </div>
              )}

              {activeSection === 'network' && (
                <div className="space-y-6">
                  <div>
                    <h3 className="text-lg font-semibold text-white mb-1">Network Settings</h3>
                    <p className="text-sm text-zinc-400">Configure network connections and relay servers</p>
                  </div>

                  <div className="space-y-4">
                    <div className="p-4 bg-zinc-800/50 rounded-lg">
                      <p className="font-medium text-white mb-2">Relay Server</p>
                      <code className="block p-2 bg-zinc-900 rounded text-sm text-zinc-300 font-mono">
                        wss://edge-net-relay-875130704813.us-central1.run.app
                      </code>
                    </div>

                    <div className="p-4 bg-zinc-800/50 rounded-lg">
                      <p className="font-medium text-white mb-2">Firebase Project</p>
                      <code className="block p-2 bg-zinc-900 rounded text-sm text-zinc-300 font-mono">
                        ruv-edge-net (peer synchronization)
                      </code>
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div>
                        <p className="font-medium text-white">Auto-Reconnect</p>
                        <p className="text-xs text-zinc-400">Automatically reconnect to relay</p>
                      </div>
                      <Switch isSelected={true} isDisabled />
                    </div>
                  </div>
                </div>
              )}

              {activeSection === 'notifications' && (
                <div className="space-y-6">
                  <div>
                    <h3 className="text-lg font-semibold text-white mb-1">Notification Settings</h3>
                    <p className="text-sm text-zinc-400">Control alerts and notifications</p>
                  </div>

                  <div className="space-y-4">
                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div>
                        <p className="font-medium text-white">Credit Milestones</p>
                        <p className="text-xs text-zinc-400">Notify on earning milestones</p>
                      </div>
                      <Switch isSelected={true} />
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div>
                        <p className="font-medium text-white">Network Events</p>
                        <p className="text-xs text-zinc-400">Peer joins/leaves notifications</p>
                      </div>
                      <Switch isSelected={false} />
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div>
                        <p className="font-medium text-white">Task Completions</p>
                        <p className="text-xs text-zinc-400">Notify when tasks complete</p>
                      </div>
                      <Switch isSelected={true} />
                    </div>
                  </div>
                </div>
              )}

              {activeSection === 'storage' && (
                <div className="space-y-6">
                  <div>
                    <h3 className="text-lg font-semibold text-white mb-1">Storage Settings</h3>
                    <p className="text-sm text-zinc-400">Manage local data and cache</p>
                  </div>

                  <div className="space-y-4">
                    <div className="p-4 bg-zinc-800/50 rounded-lg">
                      <div className="flex items-center justify-between mb-2">
                        <p className="font-medium text-white">Local Storage</p>
                        <span className="text-sm text-zinc-400">IndexedDB</span>
                      </div>
                      <p className="text-xs text-zinc-400">Used for node state and credentials</p>
                    </div>

                    <div className="flex gap-3">
                      <Button
                        variant="flat"
                        className="bg-sky-500/20 text-sky-400"
                        startContent={<Download size={16} />}
                        onPress={handleExportSettings}
                      >
                        Export Settings
                      </Button>
                      <Button
                        variant="flat"
                        className="bg-violet-500/20 text-violet-400"
                        startContent={<Upload size={16} />}
                      >
                        Import Settings
                      </Button>
                    </div>

                    <div className="p-4 bg-red-500/10 border border-red-500/30 rounded-lg">
                      <div className="flex items-center gap-2 mb-2">
                        <AlertTriangle className="text-red-400" size={16} />
                        <p className="font-medium text-red-400">Danger Zone</p>
                      </div>
                      <p className="text-xs text-zinc-400 mb-3">
                        Clear all local data including identity and credits. This cannot be undone.
                      </p>
                      <Button
                        variant="flat"
                        className="bg-red-500/20 text-red-400"
                        startContent={<Trash2 size={16} />}
                        onPress={handleClearData}
                      >
                        Clear All Data
                      </Button>
                    </div>
                  </div>
                </div>
              )}

              {activeSection === 'security' && (
                <div className="space-y-6">
                  <div>
                    <h3 className="text-lg font-semibold text-white mb-1">Security Settings</h3>
                    <p className="text-sm text-zinc-400">Privacy and security options</p>
                  </div>

                  <div className="space-y-4">
                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div>
                        <p className="font-medium text-white">WASM Sandbox</p>
                        <p className="text-xs text-zinc-400">Run tasks in isolated sandbox</p>
                      </div>
                      <Switch isSelected={true} isDisabled />
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div>
                        <p className="font-medium text-white">Verify Task Sources</p>
                        <p className="text-xs text-zinc-400">Only accept verified tasks</p>
                      </div>
                      <Switch isSelected={true} />
                    </div>

                    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
                      <div>
                        <p className="font-medium text-white">Anonymous Mode</p>
                        <p className="text-xs text-zinc-400">Hide identity from other peers</p>
                      </div>
                      <Switch isSelected={false} />
                    </div>
                  </div>
                </div>
              )}

              {/* Save Button */}
              <div className="flex justify-end pt-6 mt-6 border-t border-white/10">
                <Button
                  className="bg-gradient-to-r from-sky-500 to-violet-500 text-white"
                  startContent={saveStatus === 'saved' ? <Check size={16} /> : <Save size={16} />}
                  isLoading={saveStatus === 'saving'}
                  onPress={handleSave}
                >
                  {saveStatus === 'saved' ? 'Saved!' : 'Save Changes'}
                </Button>
              </div>
            </CardBody>
          </Card>
        </motion.div>
      </div>
    </div>
  );
}

export default SettingsPanel;
