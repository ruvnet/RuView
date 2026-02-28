import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Header } from './components/dashboard/Header';
import { Sidebar } from './components/dashboard/Sidebar';
import { NetworkStats } from './components/network/NetworkStats';
import { NetworkVisualization } from './components/network/NetworkVisualization';
import { SpecializedNetworks } from './components/network/SpecializedNetworks';
import { CDNPanel } from './components/cdn/CDNPanel';
import { WASMModules } from './components/wasm/WASMModules';
import { MCPTools } from './components/mcp/MCPTools';
import { CreditsPanel } from './components/dashboard/CreditsPanel';
import { ConsolePanel } from './components/dashboard/ConsolePanel';
import { IdentityPanel } from './components/identity/IdentityPanel';
import { DocumentationPanel } from './components/docs/DocumentationPanel';
import { CrystalLoader } from './components/common/CrystalLoader';
import { ConsentWidget } from './components/common/ConsentWidget';
import { useNetworkStore } from './stores/networkStore';

function App() {
  const [activeTab, setActiveTab] = useState('overview');
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [isMobile, setIsMobile] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const { initializeEdgeNet, updateRealStats, isWASMReady } = useNetworkStore();

  // Check for mobile viewport
  useEffect(() => {
    const checkMobile = () => setIsMobile(window.innerWidth < 768);
    checkMobile();
    window.addEventListener('resize', checkMobile);
    return () => window.removeEventListener('resize', checkMobile);
  }, []);

  // Initialize real EdgeNet WASM module
  useEffect(() => {
    const init = async () => {
      try {
        await initializeEdgeNet();
        console.log('[App] EdgeNet initialized, WASM ready:', isWASMReady);
      } catch (error) {
        console.error('[App] EdgeNet initialization failed:', error);
      } finally {
        setIsLoading(false);
      }
    };
    init();
  }, [initializeEdgeNet, isWASMReady]);

  // Update real stats from EdgeNet node
  useEffect(() => {
    const interval = setInterval(updateRealStats, 1000);
    return () => clearInterval(interval);
  }, [updateRealStats]);

  // Render active tab content
  const renderContent = () => {
    const content = {
      overview: (
        <div className="space-y-6">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
          >
            <h1 className="text-2xl md:text-3xl font-bold mb-2">
              <span className="bg-gradient-to-r from-sky-400 via-violet-400 to-cyan-400 bg-clip-text text-transparent">
                Network Overview
              </span>
            </h1>
            <p className="text-zinc-400">
              Monitor your distributed compute network in real-time
            </p>
          </motion.div>
          <NetworkStats />
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <NetworkVisualization />
            <motion.div
              className="crystal-card p-4"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ delay: 0.3 }}
            >
              <h3 className="text-sm font-medium text-zinc-400 mb-3">Quick Actions</h3>
              <div className="grid grid-cols-2 gap-3">
                <button
                  className="p-4 rounded-lg bg-sky-500/10 border border-sky-500/30 hover:bg-sky-500/20 transition-colors text-left"
                  onClick={() => setActiveTab('wasm')}
                >
                  <p className="font-medium text-white">Load WASM</p>
                  <p className="text-xs text-zinc-400 mt-1">Initialize modules</p>
                </button>
                <button
                  className="p-4 rounded-lg bg-violet-500/10 border border-violet-500/30 hover:bg-violet-500/20 transition-colors text-left"
                  onClick={() => setActiveTab('mcp')}
                >
                  <p className="font-medium text-white">MCP Tools</p>
                  <p className="text-xs text-zinc-400 mt-1">Execute tools</p>
                </button>
                <button
                  className="p-4 rounded-lg bg-emerald-500/10 border border-emerald-500/30 hover:bg-emerald-500/20 transition-colors text-left"
                  onClick={() => setActiveTab('cdn')}
                >
                  <p className="font-medium text-white">CDN Scripts</p>
                  <p className="text-xs text-zinc-400 mt-1">Load libraries</p>
                </button>
                <button
                  className="p-4 rounded-lg bg-amber-500/10 border border-amber-500/30 hover:bg-amber-500/20 transition-colors text-left"
                  onClick={() => setActiveTab('identity')}
                >
                  <p className="font-medium text-white">Identity</p>
                  <p className="text-xs text-zinc-400 mt-1">Crypto ID & Networks</p>
                </button>
              </div>
            </motion.div>
          </div>
        </div>
      ),
      network: (
        <div className="space-y-6">
          <h1 className="text-2xl font-bold">
            <span className="bg-gradient-to-r from-sky-400 to-cyan-400 bg-clip-text text-transparent">
              Network & Communities
            </span>
          </h1>
          <p className="text-zinc-400">Join specialized networks to earn credits by contributing compute</p>
          <NetworkStats />
          <SpecializedNetworks />
          <div className="mt-8">
            <h2 className="text-lg font-semibold text-zinc-300 mb-4">Network Topology</h2>
            <NetworkVisualization />
          </div>
        </div>
      ),
      wasm: (
        <div className="space-y-6">
          <h1 className="text-2xl font-bold">WASM Modules</h1>
          <WASMModules />
        </div>
      ),
      cdn: (
        <div className="space-y-6">
          <h1 className="text-2xl font-bold">CDN Script Manager</h1>
          <CDNPanel />
        </div>
      ),
      mcp: <MCPTools />,
      credits: (
        <div className="space-y-6">
          <h1 className="text-2xl font-bold">Credit Economy</h1>
          <CreditsPanel />
        </div>
      ),
      identity: (
        <div className="space-y-6">
          <h1 className="text-2xl font-bold">
            <span className="bg-gradient-to-r from-amber-400 to-orange-400 bg-clip-text text-transparent">
              Identity & Networks
            </span>
          </h1>
          <p className="text-zinc-400">Manage your cryptographic identity and network participation</p>
          <IdentityPanel />
        </div>
      ),
      console: <ConsolePanel />,
      activity: (
        <div className="crystal-card p-8 text-center">
          <p className="text-zinc-400">Activity log coming soon...</p>
        </div>
      ),
      settings: (
        <div className="crystal-card p-8 text-center">
          <p className="text-zinc-400">Settings panel coming soon...</p>
        </div>
      ),
      docs: (
        <div className="space-y-6">
          <h1 className="text-2xl font-bold">
            <span className="bg-gradient-to-r from-sky-400 to-cyan-400 bg-clip-text text-transparent">
              Documentation
            </span>
          </h1>
          <p className="text-zinc-400">Learn how to use Edge-Net and integrate it into your projects</p>
          <DocumentationPanel />
        </div>
      ),
    };

    return content[activeTab as keyof typeof content] || content.overview;
  };

  // Loading screen
  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <CrystalLoader size="lg" text="Initializing Edge-Net..." />
      </div>
    );
  }

  return (
    <div className="min-h-screen flex flex-col">
      <Header
        onMenuToggle={() => setIsSidebarOpen(true)}
        isMobile={isMobile}
      />

      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          activeTab={activeTab}
          onTabChange={setActiveTab}
          isOpen={isSidebarOpen}
          onClose={() => setIsSidebarOpen(false)}
          isMobile={isMobile}
        />

        <main className="flex-1 overflow-auto p-4 md:p-6 quantum-grid">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -20 }}
              transition={{ duration: 0.2 }}
              className="max-w-7xl mx-auto"
            >
              {renderContent()}
            </motion.div>
          </AnimatePresence>
        </main>
      </div>

      {/* Floating consent widget for CPU/GPU contribution */}
      <ConsentWidget />
    </div>
  );
}

export default App;
