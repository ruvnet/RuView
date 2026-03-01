import { useState } from 'react';
import { Button, Card, CardBody, Switch, Progress } from '@heroui/react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Download,
  Check,
  Package,
  Cpu,
  Shield,
  Network,
  Wrench,
  Copy,
  ExternalLink,
  Code,
  Terminal,
  X,
  FileCode,
  Clipboard,
} from 'lucide-react';
import { useCDNStore } from '../../stores/cdnStore';
import type { CDNScript } from '../../types';

const categoryIcons = {
  wasm: <Cpu size={16} />,
  ai: <Package size={16} />,
  crypto: <Shield size={16} />,
  network: <Network size={16} />,
  utility: <Wrench size={16} />,
};

const categoryColors = {
  wasm: 'bg-sky-500/20 text-sky-400 border-sky-500/30',
  ai: 'bg-violet-500/20 text-violet-400 border-violet-500/30',
  crypto: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
  network: 'bg-cyan-500/20 text-cyan-400 border-cyan-500/30',
  utility: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
};

// Generate code snippets for a script
function getCodeSnippets(script: CDNScript) {
  const isWasm = script.category === 'wasm';
  const packageName = script.name.startsWith('@') ? script.name : script.id;

  return {
    scriptTag: `<script src="${script.url}"></script>`,
    esModule: isWasm
      ? `import init, { /* exports */ } from '${script.url.replace('_bg.wasm', '.js')}';\n\nawait init();`
      : `import '${script.url}';`,
    npmInstall: `npm install ${packageName}`,
    cdnFetch: `const response = await fetch('${script.url}');\nconst ${isWasm ? 'wasmModule = await WebAssembly.instantiate(await response.arrayBuffer())' : 'script = await response.text()'};`,
    dynamicImport: `const module = await import('${script.url.replace('_bg.wasm', '.js')}');`,
  };
}

// Usage examples for different categories
function getUsageExample(script: CDNScript): string {
  switch (script.id) {
    case 'edge-net-wasm':
      return `import init, {
  TimeCrystal,
  CreditEconomy,
  SwarmCoordinator
} from '@ruvector/edge-net';

// Initialize WASM module
await init();

// Create Time Crystal coordinator
const crystal = new TimeCrystal();
crystal.set_frequency(1.618); // Golden ratio

// Initialize credit economy
const economy = new CreditEconomy();
const balance = economy.get_balance();

// Start swarm coordination
const swarm = new SwarmCoordinator();
swarm.join_network('wss://edge-net.ruvector.dev');`;

    case 'attention-wasm':
      return `import init, { DAGAttention } from '@ruvector/attention-unified-wasm';

await init();

const attention = new DAGAttention();
attention.add_node('task-1', ['dep-a', 'dep-b']);
attention.add_node('task-2', ['task-1']);

const order = attention.topological_sort();
const critical = attention.critical_path();`;

    case 'tensorflow':
      return `// TensorFlow.js is loaded globally as 'tf'
const model = tf.sequential();
model.add(tf.layers.dense({ units: 32, inputShape: [10] }));
model.add(tf.layers.dense({ units: 1 }));

model.compile({ optimizer: 'sgd', loss: 'meanSquaredError' });

const xs = tf.randomNormal([100, 10]);
const ys = tf.randomNormal([100, 1]);
await model.fit(xs, ys, { epochs: 10 });`;

    case 'onnx-runtime':
      return `// ONNX Runtime is loaded globally as 'ort'
const session = await ort.InferenceSession.create('model.onnx');

const inputTensor = new ort.Tensor('float32', inputData, [1, 3, 224, 224]);
const feeds = { input: inputTensor };

const results = await session.run(feeds);
const output = results.output.data;`;

    case 'noble-curves':
      return `import { ed25519 } from '@noble/curves/ed25519';
import { secp256k1 } from '@noble/curves/secp256k1';

// Ed25519 signing
const privateKey = ed25519.utils.randomPrivateKey();
const publicKey = ed25519.getPublicKey(privateKey);
const message = new TextEncoder().encode('Hello Edge-Net');
const signature = ed25519.sign(message, privateKey);
const isValid = ed25519.verify(signature, message, publicKey);`;

    case 'libp2p':
      return `import { createLibp2p } from 'libp2p';
import { webRTC } from '@libp2p/webrtc';
import { noise } from '@chainsafe/libp2p-noise';

const node = await createLibp2p({
  transports: [webRTC()],
  connectionEncryption: [noise()],
});

await node.start();
console.log('Node started:', node.peerId.toString());`;

    case 'comlink':
      return `import * as Comlink from 'comlink';

// In worker.js
const api = {
  compute: (data) => heavyComputation(data),
};
Comlink.expose(api);

// In main thread
const worker = new Worker('worker.js');
const api = Comlink.wrap(worker);
const result = await api.compute(data);`;

    default:
      return `// Load ${script.name}
// See documentation for usage examples`;
  }
}

function CodeBlock({
  code,
  onCopy,
  copied,
}: {
  code: string;
  onCopy: () => void;
  copied: boolean;
}) {
  return (
    <div className="relative group">
      <pre className="bg-zinc-950 border border-white/10 rounded-lg p-4 overflow-x-auto text-sm">
        <code className="text-zinc-300 font-mono whitespace-pre">{code}</code>
      </pre>
      <button
        onClick={onCopy}
        className={`
          absolute top-2 right-2 p-2 rounded-md transition-all
          ${copied
            ? 'bg-emerald-500/20 text-emerald-400'
            : 'bg-zinc-800 text-zinc-400 hover:text-white hover:bg-zinc-700'
          }
        `}
      >
        {copied ? <Check size={16} /> : <Copy size={16} />}
      </button>
    </div>
  );
}

function ScriptCard({
  script,
  onToggle,
  onLoad,
  onUnload,
  isLoading,
  onShowCode,
}: {
  script: CDNScript;
  onToggle: () => void;
  onLoad: () => void;
  onUnload: () => void;
  isLoading: boolean;
  onShowCode: () => void;
}) {
  const [copied, setCopied] = useState<string | null>(null);

  const copyToClipboard = async (text: string, type: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(type);
    setTimeout(() => setCopied(null), 2000);
  };

  const snippets = getCodeSnippets(script);

  return (
    <Card
      className={`bg-zinc-900/50 border ${
        script.loaded
          ? 'border-emerald-500/30'
          : script.enabled
          ? 'border-sky-500/30'
          : 'border-white/10'
      }`}
    >
      <CardBody className="p-4">
        {/* Header */}
        <div className="flex items-start justify-between gap-3 mb-3">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className={`p-1 rounded ${categoryColors[script.category]}`}>
                {categoryIcons[script.category]}
              </span>
              <h4 className="font-medium text-white truncate">{script.name}</h4>
              {script.loaded && (
                <span className="flex items-center gap-1 text-xs text-emerald-400">
                  <Check size={12} /> Loaded
                </span>
              )}
            </div>
            <p className="text-xs text-zinc-500 mt-1 line-clamp-2">
              {script.description}
            </p>
          </div>

          <Switch
            isSelected={script.enabled}
            onValueChange={onToggle}
            size="sm"
            classNames={{
              wrapper: 'bg-zinc-700 group-data-[selected=true]:bg-sky-500',
            }}
          />
        </div>

        {/* Quick Copy Buttons */}
        <div className="flex flex-wrap gap-2 mb-3">
          <button
            onClick={() => copyToClipboard(snippets.scriptTag, 'script')}
            className={`
              flex items-center gap-1.5 px-2 py-1 rounded text-xs
              transition-all border
              ${copied === 'script'
                ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400'
                : 'bg-zinc-800 border-white/10 text-zinc-400 hover:text-white hover:border-white/20'
              }
            `}
          >
            {copied === 'script' ? <Check size={12} /> : <Code size={12} />}
            Script Tag
          </button>

          <button
            onClick={() => copyToClipboard(script.url, 'url')}
            className={`
              flex items-center gap-1.5 px-2 py-1 rounded text-xs
              transition-all border
              ${copied === 'url'
                ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400'
                : 'bg-zinc-800 border-white/10 text-zinc-400 hover:text-white hover:border-white/20'
              }
            `}
          >
            {copied === 'url' ? <Check size={12} /> : <ExternalLink size={12} />}
            CDN URL
          </button>

          <button
            onClick={() => copyToClipboard(snippets.npmInstall, 'npm')}
            className={`
              flex items-center gap-1.5 px-2 py-1 rounded text-xs
              transition-all border
              ${copied === 'npm'
                ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400'
                : 'bg-zinc-800 border-white/10 text-zinc-400 hover:text-white hover:border-white/20'
              }
            `}
          >
            {copied === 'npm' ? <Check size={12} /> : <Terminal size={12} />}
            npm
          </button>

          <button
            onClick={onShowCode}
            className="flex items-center gap-1.5 px-2 py-1 rounded text-xs bg-violet-500/20 border border-violet-500/30 text-violet-400 hover:bg-violet-500/30 transition-all"
          >
            <FileCode size={12} />
            Usage
          </button>
        </div>

        {/* Actions */}
        <div className="flex items-center justify-between">
          <span className="text-xs text-zinc-500">{script.size}</span>

          <div className="flex gap-2">
            <Button
              size="sm"
              variant="flat"
              isDisabled={!script.enabled || isLoading}
              isLoading={isLoading}
              className={
                script.loaded
                  ? 'bg-red-500/20 text-red-400'
                  : 'bg-sky-500/20 text-sky-400'
              }
              onPress={script.loaded ? onUnload : onLoad}
            >
              {script.loaded ? 'Unload' : 'Load'}
            </Button>
          </div>
        </div>
      </CardBody>
    </Card>
  );
}

function CodeModal({
  script,
  isOpen,
  onClose,
}: {
  script: CDNScript | null;
  isOpen: boolean;
  onClose: () => void;
}) {
  const [copied, setCopied] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState('usage');

  if (!script) return null;

  const snippets = getCodeSnippets(script);
  const usage = getUsageExample(script);

  const copyToClipboard = async (text: string, type: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(type);
    setTimeout(() => setCopied(null), 2000);
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
            className="fixed inset-4 md:inset-auto md:left-1/2 md:top-1/2 md:-translate-x-1/2 md:-translate-y-1/2 md:w-[800px] md:max-h-[80vh] bg-zinc-900 border border-white/10 rounded-xl z-50 flex flex-col overflow-hidden"
          >
            {/* Header */}
            <div className="flex items-center justify-between p-4 border-b border-white/10">
              <div className="flex items-center gap-3">
                <span className={`p-2 rounded-lg ${categoryColors[script.category]}`}>
                  {categoryIcons[script.category]}
                </span>
                <div>
                  <h2 className="font-semibold text-white">{script.name}</h2>
                  <p className="text-xs text-zinc-500">{script.description}</p>
                </div>
              </div>
              <button
                onClick={onClose}
                className="p-2 rounded-lg hover:bg-white/5 text-zinc-400 hover:text-white transition-colors"
              >
                <X size={20} />
              </button>
            </div>

            {/* Tabs */}
            <div className="border-b border-white/10">
              <div className="flex gap-1 p-2">
                {['usage', 'import', 'cdn', 'npm'].map((tab) => (
                  <button
                    key={tab}
                    onClick={() => setActiveTab(tab)}
                    className={`
                      px-4 py-2 rounded-lg text-sm font-medium transition-all
                      ${activeTab === tab
                        ? 'bg-sky-500/20 text-sky-400'
                        : 'text-zinc-400 hover:text-white hover:bg-white/5'
                      }
                    `}
                  >
                    {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  </button>
                ))}
              </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-auto p-4">
              {activeTab === 'usage' && (
                <div className="space-y-4">
                  <h3 className="text-sm font-medium text-zinc-300">Usage Example</h3>
                  <CodeBlock
                    code={usage}
                    onCopy={() => copyToClipboard(usage, 'usage')}
                    copied={copied === 'usage'}
                  />
                </div>
              )}

              {activeTab === 'import' && (
                <div className="space-y-4">
                  <div>
                    <h3 className="text-sm font-medium text-zinc-300 mb-2">ES Module Import</h3>
                    <CodeBlock
                      code={snippets.esModule}
                      onCopy={() => copyToClipboard(snippets.esModule, 'esModule')}
                      copied={copied === 'esModule'}
                    />
                  </div>
                  <div>
                    <h3 className="text-sm font-medium text-zinc-300 mb-2">Dynamic Import</h3>
                    <CodeBlock
                      code={snippets.dynamicImport}
                      onCopy={() => copyToClipboard(snippets.dynamicImport, 'dynamicImport')}
                      copied={copied === 'dynamicImport'}
                    />
                  </div>
                </div>
              )}

              {activeTab === 'cdn' && (
                <div className="space-y-4">
                  <div>
                    <h3 className="text-sm font-medium text-zinc-300 mb-2">Script Tag</h3>
                    <CodeBlock
                      code={snippets.scriptTag}
                      onCopy={() => copyToClipboard(snippets.scriptTag, 'scriptTag')}
                      copied={copied === 'scriptTag'}
                    />
                  </div>
                  <div>
                    <h3 className="text-sm font-medium text-zinc-300 mb-2">Fetch & Instantiate</h3>
                    <CodeBlock
                      code={snippets.cdnFetch}
                      onCopy={() => copyToClipboard(snippets.cdnFetch, 'cdnFetch')}
                      copied={copied === 'cdnFetch'}
                    />
                  </div>
                  <div>
                    <h3 className="text-sm font-medium text-zinc-300 mb-2">CDN URL</h3>
                    <div className="flex items-center gap-2">
                      <input
                        type="text"
                        readOnly
                        value={script.url}
                        className="flex-1 bg-zinc-950 border border-white/10 rounded-lg px-3 py-2 text-sm text-zinc-300 font-mono"
                      />
                      <button
                        onClick={() => copyToClipboard(script.url, 'cdnUrl')}
                        className={`
                          p-2 rounded-lg transition-all
                          ${copied === 'cdnUrl'
                            ? 'bg-emerald-500/20 text-emerald-400'
                            : 'bg-zinc-800 text-zinc-400 hover:text-white'
                          }
                        `}
                      >
                        {copied === 'cdnUrl' ? <Check size={16} /> : <Copy size={16} />}
                      </button>
                    </div>
                  </div>
                </div>
              )}

              {activeTab === 'npm' && (
                <div className="space-y-4">
                  <div>
                    <h3 className="text-sm font-medium text-zinc-300 mb-2">Install via npm</h3>
                    <CodeBlock
                      code={snippets.npmInstall}
                      onCopy={() => copyToClipboard(snippets.npmInstall, 'npmInstall')}
                      copied={copied === 'npmInstall'}
                    />
                  </div>
                  <div>
                    <h3 className="text-sm font-medium text-zinc-300 mb-2">Package Info</h3>
                    <div className="bg-zinc-950 border border-white/10 rounded-lg p-4 space-y-2">
                      <div className="flex items-center justify-between">
                        <span className="text-zinc-500 text-sm">Package</span>
                        <span className="text-zinc-300 font-mono text-sm">{script.name}</span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-zinc-500 text-sm">Size</span>
                        <span className="text-zinc-300 text-sm">{script.size}</span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-zinc-500 text-sm">Category</span>
                        <span className={`px-2 py-0.5 rounded text-xs ${categoryColors[script.category]}`}>
                          {script.category}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="flex items-center justify-between p-4 border-t border-white/10">
              <a
                href={script.url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 text-sm text-sky-400 hover:text-sky-300"
              >
                <ExternalLink size={14} />
                Open in new tab
              </a>
              <Button
                color="primary"
                onPress={onClose}
              >
                Done
              </Button>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

export function CDNPanel() {
  const {
    scripts,
    autoLoad,
    cacheEnabled,
    isLoading,
    loadScript,
    unloadScript,
    toggleScript,
    setAutoLoad,
    setCacheEnabled,
  } = useCDNStore();

  const [selectedScript, setSelectedScript] = useState<CDNScript | null>(null);
  const [showCodeModal, setShowCodeModal] = useState(false);

  const groupedScripts = scripts.reduce((acc, script) => {
    if (!acc[script.category]) acc[script.category] = [];
    acc[script.category].push(script);
    return acc;
  }, {} as Record<string, CDNScript[]>);

  const loadedCount = scripts.filter((s) => s.loaded).length;
  const enabledCount = scripts.filter((s) => s.enabled).length;

  return (
    <div className="space-y-6">
      {/* Header Stats */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <div className="flex items-center justify-between mb-2">
            <p className="text-sm text-zinc-400">Loaded</p>
            <Download className="text-sky-400" size={20} />
          </div>
          <p className="text-2xl font-bold text-sky-400">
            {loadedCount}<span className="text-lg text-zinc-500">/{scripts.length}</span>
          </p>
          <Progress
            value={(loadedCount / scripts.length) * 100}
            className="mt-2"
            classNames={{
              indicator: 'bg-gradient-to-r from-sky-500 to-cyan-500',
              track: 'bg-zinc-800',
            }}
          />
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.05 }}
        >
          <div className="flex items-center justify-between mb-2">
            <p className="text-sm text-zinc-400">Enabled</p>
            <Check className="text-emerald-400" size={20} />
          </div>
          <p className="text-2xl font-bold text-emerald-400">
            {enabledCount}<span className="text-lg text-zinc-500">/{scripts.length}</span>
          </p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-zinc-400">Auto-Load</span>
            <Switch
              isSelected={autoLoad}
              onValueChange={setAutoLoad}
              size="sm"
              classNames={{
                wrapper: 'bg-zinc-700 group-data-[selected=true]:bg-sky-500',
              }}
            />
          </div>
          <p className="text-xs text-zinc-500">Load enabled scripts on startup</p>
        </motion.div>

        <motion.div
          className="crystal-card p-4"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.15 }}
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-zinc-400">Cache</span>
            <Switch
              isSelected={cacheEnabled}
              onValueChange={setCacheEnabled}
              size="sm"
              classNames={{
                wrapper: 'bg-zinc-700 group-data-[selected=true]:bg-emerald-500',
              }}
            />
          </div>
          <p className="text-xs text-zinc-500">Cache in browser storage</p>
        </motion.div>
      </div>

      {/* Quick Copy Section */}
      <motion.div
        className="crystal-card p-4"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.2 }}
      >
        <h3 className="text-sm font-medium text-zinc-300 mb-3 flex items-center gap-2">
          <Clipboard size={16} />
          Quick Start - Copy to your project
        </h3>
        <div className="bg-zinc-950 border border-white/10 rounded-lg p-3 font-mono text-sm overflow-x-auto">
          <code className="text-zinc-300">
            {'<script src="https://unpkg.com/@ruvector/edge-net@0.1.1"></script>'}
          </code>
        </div>
        <div className="flex gap-2 mt-3">
          <Button
            size="sm"
            variant="flat"
            className="bg-sky-500/20 text-sky-400"
            onPress={() => {
              navigator.clipboard.writeText('<script src="https://unpkg.com/@ruvector/edge-net@0.1.1"></script>');
            }}
          >
            <Copy size={14} />
            Copy Script Tag
          </Button>
          <Button
            size="sm"
            variant="flat"
            className="bg-violet-500/20 text-violet-400"
            onPress={() => {
              navigator.clipboard.writeText('npm install @ruvector/edge-net');
            }}
          >
            <Terminal size={14} />
            Copy npm Install
          </Button>
        </div>
      </motion.div>

      {/* Scripts by Category */}
      {Object.entries(groupedScripts).map(([category, categoryScripts], idx) => (
        <motion.div
          key={category}
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 * (idx + 3) }}
        >
          <div className="flex items-center gap-2 mb-3">
            <div className={`p-1.5 rounded ${categoryColors[category as keyof typeof categoryColors]}`}>
              {categoryIcons[category as keyof typeof categoryIcons]}
            </div>
            <h3 className="text-lg font-semibold capitalize">{category}</h3>
            <span className="px-2 py-0.5 rounded bg-zinc-800 text-zinc-400 text-xs">
              {categoryScripts.length}
            </span>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
            {categoryScripts.map((script) => (
              <ScriptCard
                key={script.id}
                script={script}
                onToggle={() => toggleScript(script.id)}
                onLoad={() => loadScript(script.id)}
                onUnload={() => unloadScript(script.id)}
                isLoading={isLoading}
                onShowCode={() => {
                  setSelectedScript(script);
                  setShowCodeModal(true);
                }}
              />
            ))}
          </div>
        </motion.div>
      ))}

      {/* Code Modal */}
      <CodeModal
        script={selectedScript}
        isOpen={showCodeModal}
        onClose={() => setShowCodeModal(false)}
      />
    </div>
  );
}
