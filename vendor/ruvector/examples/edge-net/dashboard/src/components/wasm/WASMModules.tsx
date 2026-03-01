import { Button, Card, CardBody, Chip, Modal, ModalContent, ModalHeader, ModalBody, ModalFooter, useDisclosure } from '@heroui/react';
import { motion } from 'framer-motion';
import { Cpu, BarChart3, Check, AlertCircle, Loader2 } from 'lucide-react';
import { useState } from 'react';
import { useWASMStore } from '../../stores/wasmStore';
import type { WASMModule, WASMBenchmark } from '../../types';

const statusColors = {
  loading: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
  ready: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
  error: 'bg-red-500/20 text-red-400 border-red-500/30',
  unloaded: 'bg-zinc-500/20 text-zinc-400 border-zinc-500/30',
};

const statusIcons = {
  loading: <Loader2 size={14} className="animate-spin" />,
  ready: <Check size={14} />,
  error: <AlertCircle size={14} />,
  unloaded: <Cpu size={14} />,
};

export function WASMModules() {
  const { modules, benchmarks, loadModule, runBenchmark } = useWASMStore();
  const { isOpen, onOpen, onClose } = useDisclosure();
  const [selectedModule, setSelectedModule] = useState<WASMModule | null>(null);
  const [selectedBenchmark, setSelectedBenchmark] = useState<WASMBenchmark | null>(null);

  const formatSize = (bytes: number) => {
    if (bytes >= 1000000) return `${(bytes / 1000000).toFixed(1)} MB`;
    return `${(bytes / 1000).toFixed(0)} KB`;
  };

  const handleBenchmark = async (module: WASMModule) => {
    setSelectedModule(module);
    onOpen();
    const result = await runBenchmark(module.id);
    setSelectedBenchmark(result);
  };

  const loadedCount = modules.filter((m) => m.loaded).length;

  return (
    <div className="space-y-6">
      {/* Overview */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <p className="text-sm text-zinc-400">Total Modules</p>
          <p className="text-3xl font-bold text-white">{modules.length}</p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
        >
          <p className="text-sm text-zinc-400">Loaded</p>
          <p className="text-3xl font-bold text-emerald-400">{loadedCount}</p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
        >
          <p className="text-sm text-zinc-400">Total Size</p>
          <p className="text-3xl font-bold text-sky-400">
            {formatSize(modules.reduce((acc, m) => acc + m.size, 0))}
          </p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3 }}
        >
          <p className="text-sm text-zinc-400">Benchmarks Run</p>
          <p className="text-3xl font-bold text-violet-400">{benchmarks.length}</p>
        </motion.div>
      </div>

      {/* Module List */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {modules.map((module, idx) => (
          <motion.div
            key={module.id}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.1 * idx }}
          >
            <Card className="bg-zinc-900/50 border border-white/10 hover:border-sky-500/30 transition-colors">
              <CardBody className="p-5">
                <div className="flex items-start justify-between mb-3">
                  <div>
                    <h4 className="font-semibold text-white text-lg">{module.name}</h4>
                    <p className="text-xs text-zinc-500">v{module.version}</p>
                  </div>
                  <Chip
                    size="sm"
                    variant="bordered"
                    startContent={statusIcons[module.status]}
                    className={statusColors[module.status]}
                  >
                    {module.status}
                  </Chip>
                </div>

                <div className="flex flex-wrap gap-1.5 mb-4">
                  {module.features.map((feature) => (
                    <Chip
                      key={feature}
                      size="sm"
                      variant="flat"
                      className="bg-zinc-800 text-zinc-400 text-xs"
                    >
                      {feature}
                    </Chip>
                  ))}
                </div>

                <div className="flex items-center justify-between">
                  <span className="text-sm text-zinc-500">{formatSize(module.size)}</span>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="flat"
                      className="bg-sky-500/20 text-sky-400"
                      isDisabled={module.loaded || module.status === 'loading'}
                      isLoading={module.status === 'loading'}
                      onPress={() => loadModule(module.id)}
                    >
                      {module.loaded ? 'Loaded' : 'Load'}
                    </Button>
                    <Button
                      size="sm"
                      variant="flat"
                      className="bg-violet-500/20 text-violet-400"
                      isDisabled={!module.loaded}
                      startContent={<BarChart3 size={14} />}
                      onPress={() => handleBenchmark(module)}
                    >
                      Benchmark
                    </Button>
                  </div>
                </div>

                {module.error && (
                  <div className="mt-3 p-2 rounded bg-red-500/10 border border-red-500/30">
                    <p className="text-xs text-red-400">{module.error}</p>
                  </div>
                )}
              </CardBody>
            </Card>
          </motion.div>
        ))}
      </div>

      {/* Benchmark Modal */}
      <Modal isOpen={isOpen} onClose={onClose} size="lg" className="dark">
        <ModalContent className="bg-zinc-900 border border-white/10">
          <ModalHeader className="border-b border-white/10">
            <div className="flex items-center gap-2">
              <BarChart3 className="text-violet-400" size={20} />
              <span>Benchmark Results</span>
            </div>
          </ModalHeader>
          <ModalBody className="py-6">
            {selectedModule && (
              <div className="space-y-4">
                <div>
                  <p className="text-sm text-zinc-400">Module</p>
                  <p className="text-lg font-semibold text-white">{selectedModule.name}</p>
                </div>

                {selectedBenchmark ? (
                  <div className="grid grid-cols-2 gap-4">
                    <div className="p-4 rounded-lg bg-zinc-800/50">
                      <p className="text-xs text-zinc-400">Iterations</p>
                      <p className="text-2xl font-bold text-sky-400">
                        {selectedBenchmark.iterations.toLocaleString()}
                      </p>
                    </div>
                    <div className="p-4 rounded-lg bg-zinc-800/50">
                      <p className="text-xs text-zinc-400">Avg Time</p>
                      <p className="text-2xl font-bold text-violet-400">
                        {selectedBenchmark.avgTime.toFixed(3)}ms
                      </p>
                    </div>
                    <div className="p-4 rounded-lg bg-zinc-800/50">
                      <p className="text-xs text-zinc-400">Min/Max</p>
                      <p className="text-lg font-bold text-cyan-400">
                        {selectedBenchmark.minTime.toFixed(3)} / {selectedBenchmark.maxTime.toFixed(3)}ms
                      </p>
                    </div>
                    <div className="p-4 rounded-lg bg-zinc-800/50">
                      <p className="text-xs text-zinc-400">Throughput</p>
                      <p className="text-2xl font-bold text-emerald-400">
                        {selectedBenchmark.throughput.toFixed(0)}/s
                      </p>
                    </div>
                  </div>
                ) : (
                  <div className="flex items-center justify-center py-8">
                    <Loader2 className="animate-spin text-sky-400" size={32} />
                  </div>
                )}
              </div>
            )}
          </ModalBody>
          <ModalFooter className="border-t border-white/10">
            <Button variant="flat" onPress={onClose}>
              Close
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </div>
  );
}
