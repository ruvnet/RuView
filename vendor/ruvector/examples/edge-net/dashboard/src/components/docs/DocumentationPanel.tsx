import { useState } from 'react';
import { motion } from 'framer-motion';
import { Card, CardBody, Code, Snippet } from '@heroui/react';
import {
  BookOpen,
  Zap,
  Shield,
  Cpu,
  Code2,
  Terminal,
  Wallet,
  Users,
  ChevronRight,
} from 'lucide-react';

interface DocSection {
  id: string;
  title: string;
  icon: React.ReactNode;
  content: React.ReactNode;
}

export function DocumentationPanel() {
  const [selectedSection, setSelectedSection] = useState('getting-started');

  const sections: DocSection[] = [
    {
      id: 'getting-started',
      title: 'Getting Started',
      icon: <BookOpen size={18} />,
      content: <GettingStartedSection />,
    },
    {
      id: 'how-it-works',
      title: 'How It Works',
      icon: <Zap size={18} />,
      content: <HowItWorksSection />,
    },
    {
      id: 'pi-key',
      title: 'PiKey Identity',
      icon: <Shield size={18} />,
      content: <PiKeySection />,
    },
    {
      id: 'contributing',
      title: 'Contributing Compute',
      icon: <Cpu size={18} />,
      content: <ContributingSection />,
    },
    {
      id: 'credits',
      title: 'rUv Credits',
      icon: <Wallet size={18} />,
      content: <CreditsSection />,
    },
    {
      id: 'api',
      title: 'API Reference',
      icon: <Code2 size={18} />,
      content: <ApiSection />,
    },
    {
      id: 'cli',
      title: 'CLI Usage',
      icon: <Terminal size={18} />,
      content: <CliSection />,
    },
  ];

  return (
    <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
      {/* Navigation */}
      <div className="lg:col-span-1">
        <div className="crystal-card p-4 sticky top-4">
          <h3 className="text-sm font-medium text-zinc-400 mb-4">Documentation</h3>
          <nav className="space-y-1">
            {sections.map((section) => (
              <button
                key={section.id}
                onClick={() => setSelectedSection(section.id)}
                className={`
                  w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm
                  transition-all duration-200
                  ${
                    selectedSection === section.id
                      ? 'bg-sky-500/20 text-sky-400 border border-sky-500/30'
                      : 'text-zinc-400 hover:text-white hover:bg-white/5'
                  }
                `}
              >
                {section.icon}
                <span>{section.title}</span>
                {selectedSection === section.id && (
                  <ChevronRight size={14} className="ml-auto" />
                )}
              </button>
            ))}
          </nav>
        </div>
      </div>

      {/* Content */}
      <div className="lg:col-span-3">
        <motion.div
          key={selectedSection}
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2 }}
        >
          {sections.find((s) => s.id === selectedSection)?.content}
        </motion.div>
      </div>
    </div>
  );
}

function GettingStartedSection() {
  return (
    <div className="crystal-card p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-2">Welcome to Edge-Net</h2>
        <p className="text-zinc-400">
          Edge-Net is a collective AI computing network that allows you to share idle
          browser resources and earn rUv credits in return.
        </p>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Quick Start</h3>

        <div className="space-y-3">
          <div className="flex items-start gap-3 p-4 bg-zinc-800/50 rounded-lg">
            <div className="w-6 h-6 rounded-full bg-sky-500/20 text-sky-400 flex items-center justify-center text-sm font-bold">
              1
            </div>
            <div>
              <p className="text-white font-medium">Generate Your Identity</p>
              <p className="text-sm text-zinc-400">
                Go to the Identity tab and create a PiKey cryptographic identity.
                This is your unique identifier on the network.
              </p>
            </div>
          </div>

          <div className="flex items-start gap-3 p-4 bg-zinc-800/50 rounded-lg">
            <div className="w-6 h-6 rounded-full bg-sky-500/20 text-sky-400 flex items-center justify-center text-sm font-bold">
              2
            </div>
            <div>
              <p className="text-white font-medium">Give Consent</p>
              <p className="text-sm text-zinc-400">
                Click the floating button in the bottom-right corner and accept
                the consent dialog to start contributing.
              </p>
            </div>
          </div>

          <div className="flex items-start gap-3 p-4 bg-zinc-800/50 rounded-lg">
            <div className="w-6 h-6 rounded-full bg-sky-500/20 text-sky-400 flex items-center justify-center text-sm font-bold">
              3
            </div>
            <div>
              <p className="text-white font-medium">Earn rUv Credits</p>
              <p className="text-sm text-zinc-400">
                Watch your credits grow as you contribute compute. Use them for
                AI tasks or transfer to other users.
              </p>
            </div>
          </div>
        </div>
      </div>

      <div className="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-lg">
        <div className="flex items-center gap-2 mb-2">
          <Users size={18} className="text-emerald-400" />
          <span className="font-medium text-emerald-400">Join the Collective</span>
        </div>
        <p className="text-sm text-zinc-300">
          When you contribute, you become part of a decentralized network of
          nodes working together to power AI computations.
        </p>
      </div>
    </div>
  );
}

function HowItWorksSection() {
  return (
    <div className="crystal-card p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-2">How Edge-Net Works</h2>
        <p className="text-zinc-400">
          Edge-Net uses WebAssembly (WASM) to run secure, sandboxed computations
          in your browser.
        </p>
      </div>

      <div className="grid gap-4">
        <Card className="bg-zinc-800/50 border border-zinc-700">
          <CardBody className="gap-3">
            <h4 className="font-semibold text-sky-400">WASM Runtime</h4>
            <p className="text-sm text-zinc-400">
              All computations run in a WebAssembly sandbox, ensuring security
              and isolation from your system.
            </p>
          </CardBody>
        </Card>

        <Card className="bg-zinc-800/50 border border-zinc-700">
          <CardBody className="gap-3">
            <h4 className="font-semibold text-violet-400">Time Crystal Sync</h4>
            <p className="text-sm text-zinc-400">
              Nodes synchronize using a novel time crystal protocol that ensures
              coherent distributed computation without a central clock.
            </p>
          </CardBody>
        </Card>

        <Card className="bg-zinc-800/50 border border-zinc-700">
          <CardBody className="gap-3">
            <h4 className="font-semibold text-emerald-400">Adaptive Security</h4>
            <p className="text-sm text-zinc-400">
              Machine learning-based security system that detects and prevents
              malicious activity in real-time.
            </p>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}

function PiKeySection() {
  return (
    <div className="crystal-card p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-2">PiKey Cryptographic Identity</h2>
        <p className="text-zinc-400">
          PiKey provides a unique, mathematically-proven identity using Ed25519
          cryptography with pi-based derivation.
        </p>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Features</h3>
        <ul className="space-y-2 text-zinc-300">
          <li className="flex items-center gap-2">
            <Shield size={16} className="text-sky-400" />
            Ed25519 digital signatures
          </li>
          <li className="flex items-center gap-2">
            <Shield size={16} className="text-violet-400" />
            Argon2id encrypted backups
          </li>
          <li className="flex items-center gap-2">
            <Shield size={16} className="text-emerald-400" />
            Pi-magic verification for authenticity
          </li>
          <li className="flex items-center gap-2">
            <Shield size={16} className="text-amber-400" />
            Cross-platform portability
          </li>
        </ul>
      </div>

      <div>
        <h3 className="text-lg font-semibold text-white mb-3">Backup Your Key</h3>
        <p className="text-sm text-zinc-400 mb-4">
          Always create an encrypted backup of your PiKey. Without it, you cannot
          recover your identity or earned credits.
        </p>
        <Code className="w-full p-3 bg-zinc-900 text-sm">
          {`// Export encrypted backup
const backup = piKey.createEncryptedBackup("your-password");
// Save backup hex string securely`}
        </Code>
      </div>
    </div>
  );
}

function ContributingSection() {
  return (
    <div className="crystal-card p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-2">Contributing Compute</h2>
        <p className="text-zinc-400">
          Share your idle browser resources to power AI computations and earn credits.
        </p>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Resource Settings</h3>

        <div className="grid gap-3">
          <div className="p-4 bg-zinc-800/50 rounded-lg">
            <div className="flex items-center gap-2 mb-2">
              <Cpu size={16} className="text-sky-400" />
              <span className="font-medium text-white">CPU Limit</span>
            </div>
            <p className="text-sm text-zinc-400">
              Control how much CPU to allocate (10-80%). Higher values earn more
              credits but may affect browser performance.
            </p>
          </div>

          <div className="p-4 bg-zinc-800/50 rounded-lg">
            <div className="flex items-center gap-2 mb-2">
              <Zap size={16} className="text-violet-400" />
              <span className="font-medium text-white">GPU Acceleration</span>
            </div>
            <p className="text-sm text-zinc-400">
              Enable WebGL/WebGPU for AI inference. Earns 3x more credits than
              CPU-only contributions.
            </p>
          </div>
        </div>
      </div>

      <div className="p-4 bg-amber-500/10 border border-amber-500/30 rounded-lg">
        <p className="text-sm text-amber-300">
          <strong>Privacy First:</strong> No personal data is collected. Your
          identity is purely cryptographic, and all computations are sandboxed.
        </p>
      </div>
    </div>
  );
}

function CreditsSection() {
  return (
    <div className="crystal-card p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-2">rUv Credits</h2>
        <p className="text-zinc-400">
          rUv (Resource Utility Vouchers) are the currency of Edge-Net.
        </p>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Credit Economy</h3>

        <div className="grid gap-3">
          <div className="flex justify-between items-center p-3 bg-zinc-800/50 rounded-lg">
            <span className="text-zinc-300">CPU contribution (per hour)</span>
            <span className="text-emerald-400 font-mono">~0.5 rUv</span>
          </div>
          <div className="flex justify-between items-center p-3 bg-zinc-800/50 rounded-lg">
            <span className="text-zinc-300">GPU contribution (per hour)</span>
            <span className="text-emerald-400 font-mono">~1.5 rUv</span>
          </div>
          <div className="flex justify-between items-center p-3 bg-zinc-800/50 rounded-lg">
            <span className="text-zinc-300">AI inference task</span>
            <span className="text-amber-400 font-mono">0.01-1.0 rUv</span>
          </div>
        </div>
      </div>

      <div>
        <h3 className="text-lg font-semibold text-white mb-3">Use Cases</h3>
        <ul className="space-y-2 text-zinc-300 text-sm">
          <li>- Submit AI inference tasks to the network</li>
          <li>- Access premium WASM modules</li>
          <li>- Transfer to other network participants</li>
          <li>- Reserve compute capacity for projects</li>
        </ul>
      </div>
    </div>
  );
}

function ApiSection() {
  return (
    <div className="crystal-card p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-2">API Reference</h2>
        <p className="text-zinc-400">
          Integrate Edge-Net into your applications using our JavaScript API.
        </p>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Installation</h3>
        <Snippet symbol="$" variant="bordered" className="bg-zinc-900">
          npm install @ruvector/edge-net
        </Snippet>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Basic Usage</h3>
        <Code className="w-full p-4 bg-zinc-900 text-sm overflow-x-auto">
          {`import init, { EdgeNetConfig, PiKey } from '@ruvector/edge-net';

// Initialize WASM
await init();

// Create identity
const piKey = new PiKey();
console.log('Node ID:', piKey.getShortId());

// Create and start node
const node = new EdgeNetConfig('my-app')
  .cpuLimit(0.5)
  .respectBattery(true)
  .build();

node.start();

// Get stats
const stats = node.getStats();
console.log('Credits earned:', stats.ruv_earned);`}
        </Code>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Key Classes</h3>
        <div className="space-y-2">
          <div className="p-3 bg-zinc-800/50 rounded-lg">
            <code className="text-sky-400">EdgeNetNode</code>
            <span className="text-zinc-400 text-sm ml-2">- Main node instance</span>
          </div>
          <div className="p-3 bg-zinc-800/50 rounded-lg">
            <code className="text-violet-400">PiKey</code>
            <span className="text-zinc-400 text-sm ml-2">- Cryptographic identity</span>
          </div>
          <div className="p-3 bg-zinc-800/50 rounded-lg">
            <code className="text-emerald-400">AdaptiveSecurity</code>
            <span className="text-zinc-400 text-sm ml-2">- ML security system</span>
          </div>
          <div className="p-3 bg-zinc-800/50 rounded-lg">
            <code className="text-amber-400">TimeCrystal</code>
            <span className="text-zinc-400 text-sm ml-2">- Distributed sync</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function CliSection() {
  return (
    <div className="crystal-card p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-2">CLI Usage</h2>
        <p className="text-zinc-400">
          Run Edge-Net from the command line for server-side contributions.
        </p>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Install</h3>
        <Snippet symbol="$" variant="bordered" className="bg-zinc-900">
          npm install -g @ruvector/edge-net
        </Snippet>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Commands</h3>
        <div className="space-y-3">
          <div className="p-3 bg-zinc-800/50 rounded-lg font-mono text-sm">
            <div className="text-emerald-400">edge-net start</div>
            <div className="text-zinc-500 mt-1">Start contributing node</div>
          </div>
          <div className="p-3 bg-zinc-800/50 rounded-lg font-mono text-sm">
            <div className="text-emerald-400">edge-net status</div>
            <div className="text-zinc-500 mt-1">View node status and stats</div>
          </div>
          <div className="p-3 bg-zinc-800/50 rounded-lg font-mono text-sm">
            <div className="text-emerald-400">edge-net identity generate</div>
            <div className="text-zinc-500 mt-1">Create new PiKey identity</div>
          </div>
          <div className="p-3 bg-zinc-800/50 rounded-lg font-mono text-sm">
            <div className="text-emerald-400">edge-net credits balance</div>
            <div className="text-zinc-500 mt-1">Check rUv credit balance</div>
          </div>
        </div>
      </div>

      <div className="p-4 bg-sky-500/10 border border-sky-500/30 rounded-lg">
        <p className="text-sm text-sky-300">
          <strong>Node.js Support:</strong> The CLI uses the same WASM module
          as the browser, ensuring consistent behavior across platforms.
        </p>
      </div>
    </div>
  );
}
