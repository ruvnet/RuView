import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';
import {
  Button,
  Slider,
  Switch,
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
} from '@heroui/react';
import {
  Cpu,
  Zap,
  Battery,
  Clock,
  ChevronUp,
  ChevronDown,
  Shield,
  X,
  Settings,
  Play,
  Pause,
} from 'lucide-react';
import { useNetworkStore } from '../../stores/networkStore';

export function ConsentWidget() {
  const [isExpanded, setIsExpanded] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const {
    contributionSettings,
    setContributionSettings,
    giveConsent,
    revokeConsent,
    startContributing,
    stopContributing,
    stats,
    credits,
  } = useNetworkStore();

  const { consentGiven, enabled, cpuLimit, gpuEnabled, respectBattery, onlyWhenIdle } =
    contributionSettings;

  // Show initial consent dialog if not given
  const [showInitialConsent, setShowInitialConsent] = useState(false);

  useEffect(() => {
    // Only show after a delay to not be intrusive
    const timer = setTimeout(() => {
      if (!consentGiven) {
        setShowInitialConsent(true);
      }
    }, 3000);
    return () => clearTimeout(timer);
  }, [consentGiven]);

  const handleGiveConsent = () => {
    giveConsent();
    setShowInitialConsent(false);
    startContributing();
  };

  const handleToggleContribution = () => {
    if (enabled) {
      stopContributing();
    } else {
      startContributing();
    }
  };

  // Minimized floating button - always visible
  if (!isExpanded) {
    return (
      <>
        <motion.div
          className="fixed bottom-4 right-4 z-50"
          initial={{ scale: 0 }}
          animate={{ scale: 1 }}
          transition={{ type: 'spring', stiffness: 260, damping: 20 }}
        >
          <button
            onClick={() => consentGiven ? setIsExpanded(true) : setShowInitialConsent(true)}
            className={`
              flex items-center gap-2 px-4 py-2 rounded-full shadow-lg
              border backdrop-blur-xl transition-all
              ${
                enabled
                  ? 'bg-emerald-500/20 border-emerald-500/50 text-emerald-400 hover:bg-emerald-500/30'
                  : consentGiven
                    ? 'bg-zinc-800/80 border-zinc-700 text-zinc-400 hover:bg-zinc-700/80'
                    : 'bg-violet-500/20 border-violet-500/50 text-violet-400 hover:bg-violet-500/30'
              }
            `}
            aria-label="Open Edge-Net contribution panel"
          >
            {enabled ? (
              <motion.div
                animate={{ rotate: 360 }}
                transition={{ duration: 2, repeat: Infinity, ease: 'linear' }}
              >
                <Cpu size={18} />
              </motion.div>
            ) : (
              <Zap size={18} />
            )}
            <span className="text-sm font-medium">
              {enabled ? `${credits.earned.toFixed(2)} rUv` : consentGiven ? 'Paused' : 'Join Edge-Net'}
            </span>
            <ChevronUp size={14} />
          </button>
        </motion.div>

        {/* Initial consent modal */}
        <Modal
          isOpen={showInitialConsent}
          onClose={() => setShowInitialConsent(false)}
          size="md"
          placement="center"
          backdrop="blur"
          classNames={{
            base: 'bg-zinc-900/95 backdrop-blur-xl border border-zinc-700/50 shadow-2xl mx-4',
            wrapper: 'items-center justify-center',
            header: 'border-b-0 pb-0',
            body: 'px-8 py-6',
            footer: 'border-t border-zinc-800/50 pt-6 px-8 pb-6',
          }}
        >
          <ModalContent>
            <ModalHeader className="flex flex-col items-center text-center pt-8 px-8">
              {/* Logo */}
              <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-sky-500 via-violet-500 to-cyan-500 flex items-center justify-center mb-4 shadow-lg shadow-violet-500/30">
                <Zap size={32} className="text-white" />
              </div>
              <h3 className="text-2xl font-bold text-white">
                Join Edge-Net
              </h3>
              <p className="text-sm text-zinc-400 mt-2">
                The Collective AI Computing Network
              </p>
            </ModalHeader>

            <ModalBody>
              <div className="space-y-6">
                {/* Introduction - improved text */}
                <div className="text-center space-y-3">
                  <p className="text-zinc-200 text-base leading-relaxed">
                    Transform your idle browser into a powerful AI compute node.
                  </p>
                  <p className="text-zinc-400 text-sm leading-relaxed">
                    When you're not using your browser, Edge-Net harnesses unused CPU cycles
                    to power distributed AI computations. In return, you earn{' '}
                    <span className="text-emerald-400 font-semibold">rUv credits</span> that
                    can be used for AI services across the network.
                  </p>
                </div>

                {/* Features - compact grid */}
                <div className="grid grid-cols-2 gap-3">
                  <div className="flex items-center gap-3 p-3 bg-zinc-800/40 rounded-xl border border-zinc-700/30">
                    <Cpu size={18} className="text-sky-400 flex-shrink-0" />
                    <div>
                      <div className="text-sm text-zinc-200 font-medium">Idle Only</div>
                      <div className="text-xs text-zinc-500">Uses spare CPU cycles</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-3 p-3 bg-zinc-800/40 rounded-xl border border-zinc-700/30">
                    <Battery size={18} className="text-emerald-400 flex-shrink-0" />
                    <div>
                      <div className="text-sm text-zinc-200 font-medium">Battery Aware</div>
                      <div className="text-xs text-zinc-500">Pauses on low power</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-3 p-3 bg-zinc-800/40 rounded-xl border border-zinc-700/30">
                    <Shield size={18} className="text-violet-400 flex-shrink-0" />
                    <div>
                      <div className="text-sm text-zinc-200 font-medium">Privacy First</div>
                      <div className="text-xs text-zinc-500">WASM sandboxed</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-3 p-3 bg-zinc-800/40 rounded-xl border border-zinc-700/30">
                    <Clock size={18} className="text-amber-400 flex-shrink-0" />
                    <div>
                      <div className="text-sm text-zinc-200 font-medium">Full Control</div>
                      <div className="text-xs text-zinc-500">Pause anytime</div>
                    </div>
                  </div>
                </div>

                {/* Trust badge */}
                <div className="text-center pt-2">
                  <p className="text-xs text-zinc-500">
                    Secured by WASM sandbox isolation & PiKey cryptography
                  </p>
                </div>
              </div>
            </ModalBody>

            <ModalFooter className="flex-col gap-3">
              <Button
                fullWidth
                color="primary"
                size="lg"
                onPress={handleGiveConsent}
                className="bg-gradient-to-r from-sky-500 to-violet-500 font-semibold text-base h-12"
                startContent={<Play size={18} />}
              >
                Start Contributing
              </Button>
              <Button
                fullWidth
                variant="light"
                size="sm"
                onPress={() => setShowInitialConsent(false)}
                className="text-zinc-500 hover:text-zinc-300"
              >
                Maybe Later
              </Button>
            </ModalFooter>
          </ModalContent>
        </Modal>
      </>
    );
  }

  // Expanded panel with settings modal
  return (
    <>
      <motion.div
        className="fixed bottom-4 right-4 z-50 w-80"
        initial={{ y: 100, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        exit={{ y: 100, opacity: 0 }}
      >
        <div className="crystal-card p-4 shadow-xl">
          {/* Header */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <div
                className={`w-2 h-2 rounded-full ${
                  enabled ? 'bg-emerald-400 animate-pulse' : 'bg-zinc-500'
                }`}
              />
              <span className="text-sm font-medium text-white">
                {enabled ? 'Contributing' : 'Paused'}
              </span>
            </div>
            <div className="flex items-center gap-1">
              <Button
                isIconOnly
                size="sm"
                variant="light"
                onPress={() => setShowSettings(true)}
                aria-label="Open settings"
              >
                <Settings size={16} />
              </Button>
              <Button
                isIconOnly
                size="sm"
                variant="light"
                onPress={() => setIsExpanded(false)}
                aria-label="Minimize panel"
              >
                <ChevronDown size={16} />
              </Button>
            </div>
          </div>

          {/* Stats */}
          <div className="grid grid-cols-2 gap-3 mb-4">
            <div className="bg-zinc-800/50 rounded-lg p-3">
              <div className="text-xs text-zinc-500 mb-1">rUv Earned</div>
              <div className="text-lg font-bold text-emerald-400">
                {credits.earned.toFixed(2)}
              </div>
            </div>
            <div className="bg-zinc-800/50 rounded-lg p-3">
              <div className="text-xs text-zinc-500 mb-1">Tasks</div>
              <div className="text-lg font-bold text-sky-400">
                {stats.tasksCompleted}
              </div>
            </div>
          </div>

          {/* CPU Slider */}
          <div className="mb-4">
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs text-zinc-400">CPU Limit</span>
              <span className="text-xs text-white font-medium">{cpuLimit}%</span>
            </div>
            <Slider
              size="sm"
              step={10}
              minValue={10}
              maxValue={80}
              value={cpuLimit}
              onChange={(value) =>
                setContributionSettings({ cpuLimit: value as number })
              }
              classNames={{
                track: 'bg-zinc-700',
                filler: 'bg-gradient-to-r from-sky-500 to-violet-500',
              }}
              aria-label="CPU usage limit"
            />
          </div>

          {/* Quick toggles */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2 text-xs text-zinc-400">
              <Battery size={14} />
              <span>Respect Battery</span>
            </div>
            <Switch
              size="sm"
              isSelected={respectBattery}
              onValueChange={(value) =>
                setContributionSettings({ respectBattery: value })
              }
              aria-label="Respect battery power"
            />
          </div>

          {/* Control button */}
          <Button
            fullWidth
            color={enabled ? 'warning' : 'success'}
            variant="flat"
            onPress={handleToggleContribution}
            startContent={enabled ? <Pause size={16} /> : <Play size={16} />}
          >
            {enabled ? 'Pause Contribution' : 'Start Contributing'}
          </Button>
        </div>
      </motion.div>

      {/* Settings Modal */}
      <Modal
        isOpen={showSettings}
        onClose={() => setShowSettings(false)}
        size="sm"
        placement="center"
        classNames={{
          base: 'bg-zinc-900/95 backdrop-blur-xl border border-zinc-700/50 mx-4',
          header: 'border-b border-zinc-800 py-3 px-5',
          body: 'py-5 px-5',
          footer: 'border-t border-zinc-800 py-3 px-5',
          closeButton: 'top-3 right-3 hover:bg-zinc-700/50',
        }}
      >
        <ModalContent>
          <ModalHeader className="flex justify-between items-center">
            <h3 className="text-base font-semibold text-white">
              Contribution Settings
            </h3>
          </ModalHeader>

          <ModalBody>
            <div className="space-y-4">
              {/* CPU Settings */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <Cpu size={16} className="text-sky-400" />
                    <span className="text-white text-sm">CPU Limit</span>
                  </div>
                  <span className="text-sky-400 text-sm font-bold">{cpuLimit}%</span>
                </div>
                <Slider
                  size="sm"
                  step={5}
                  minValue={10}
                  maxValue={80}
                  value={cpuLimit}
                  onChange={(value) =>
                    setContributionSettings({ cpuLimit: value as number })
                  }
                  classNames={{
                    track: 'bg-zinc-700',
                    filler: 'bg-gradient-to-r from-sky-500 to-cyan-500',
                  }}
                  aria-label="CPU usage limit slider"
                />
              </div>

              {/* GPU Settings */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Zap size={16} className="text-violet-400" />
                  <span className="text-white text-sm">GPU Acceleration</span>
                </div>
                <Switch
                  size="sm"
                  isSelected={gpuEnabled}
                  onValueChange={(value) =>
                    setContributionSettings({ gpuEnabled: value })
                  }
                  aria-label="Enable GPU acceleration"
                />
              </div>

              {/* Other settings */}
              <div className="space-y-3 pt-2 border-t border-zinc-800">
                <div className="flex items-center justify-between">
                  <span className="text-zinc-300 text-sm">Respect Battery</span>
                  <Switch
                    size="sm"
                    isSelected={respectBattery}
                    onValueChange={(value) =>
                      setContributionSettings({ respectBattery: value })
                    }
                    aria-label="Respect battery power"
                  />
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-zinc-300 text-sm">Only When Idle</span>
                  <Switch
                    size="sm"
                    isSelected={onlyWhenIdle}
                    onValueChange={(value) =>
                      setContributionSettings({ onlyWhenIdle: value })
                    }
                    aria-label="Only contribute when idle"
                  />
                </div>
              </div>

              {/* Revoke consent */}
              <div className="pt-3 border-t border-zinc-800">
                <Button
                  size="sm"
                  variant="flat"
                  color="danger"
                  onPress={() => {
                    revokeConsent();
                    setShowSettings(false);
                    setIsExpanded(false);
                  }}
                  startContent={<X size={14} />}
                >
                  Revoke Consent
                </Button>
              </div>
            </div>
          </ModalBody>

          <ModalFooter>
            <Button onPress={() => setShowSettings(false)}>Done</Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </>
  );
}
