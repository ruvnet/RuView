import { Button } from '@heroui/react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  LayoutDashboard,
  Network,
  Cpu,
  Package,
  Wrench,
  Terminal,
  Settings,
  X,
  Coins,
  Activity,
  KeyRound,
  BookOpen,
} from 'lucide-react';
import type { ReactNode } from 'react';

interface SidebarProps {
  activeTab: string;
  onTabChange: (tab: string) => void;
  isOpen: boolean;
  onClose: () => void;
  isMobile: boolean;
}

interface NavItem {
  id: string;
  label: string;
  icon: ReactNode;
  badge?: number;
}

const navItems: NavItem[] = [
  { id: 'overview', label: 'Overview', icon: <LayoutDashboard size={18} /> },
  { id: 'identity', label: 'Identity', icon: <KeyRound size={18} /> },
  { id: 'network', label: 'Network', icon: <Network size={18} /> },
  { id: 'wasm', label: 'WASM Modules', icon: <Cpu size={18} /> },
  { id: 'cdn', label: 'CDN Scripts', icon: <Package size={18} /> },
  { id: 'mcp', label: 'MCP Tools', icon: <Wrench size={18} /> },
  { id: 'credits', label: 'Credits', icon: <Coins size={18} /> },
  { id: 'console', label: 'Console', icon: <Terminal size={18} /> },
  { id: 'docs', label: 'Documentation', icon: <BookOpen size={18} /> },
];

const bottomItems: NavItem[] = [
  { id: 'activity', label: 'Activity', icon: <Activity size={18} /> },
  { id: 'settings', label: 'Settings', icon: <Settings size={18} /> },
];

export function Sidebar({ activeTab, onTabChange, isOpen, onClose, isMobile }: SidebarProps) {
  const NavButton = ({ item, activeColor = 'sky' }: { item: NavItem; activeColor?: string }) => {
    const isActive = activeTab === item.id;
    const colorClasses = activeColor === 'sky'
      ? 'bg-sky-500/20 text-sky-400 border-sky-500/30'
      : 'bg-violet-500/20 text-violet-400 border-violet-500/30';

    return (
      <button
        onClick={() => {
          onTabChange(item.id);
          if (isMobile) onClose();
        }}
        className={`
          w-full h-10 px-3 rounded-lg
          flex items-center gap-3
          transition-all duration-200
          ${isActive
            ? `${colorClasses} border`
            : 'text-zinc-400 hover:text-white hover:bg-white/5 border border-transparent'
          }
        `}
      >
        <span className="flex-shrink-0 flex items-center justify-center w-5">
          {item.icon}
        </span>
        <span className="flex-1 text-left text-sm font-medium truncate">
          {item.label}
        </span>
        {item.badge !== undefined && (
          <span className="text-xs bg-sky-500/20 text-sky-400 px-2 py-0.5 rounded-full">
            {item.badge.toLocaleString()}
          </span>
        )}
      </button>
    );
  };

  const content = (
    <div className="flex flex-col h-full py-4">
      {/* Close button (mobile) */}
      {isMobile && (
        <div className="flex justify-end px-4 mb-4">
          <Button isIconOnly variant="light" onPress={onClose} className="text-zinc-400">
            <X size={20} />
          </Button>
        </div>
      )}

      {/* Main Navigation */}
      <nav className="flex-1 px-3">
        <div className="space-y-1">
          {navItems.map((item) => (
            <NavButton key={item.id} item={item} activeColor="sky" />
          ))}
        </div>
      </nav>

      {/* Divider */}
      <div className="border-t border-white/10 mx-3 my-4" />

      {/* Bottom Navigation */}
      <nav className="px-3">
        <div className="space-y-1">
          {bottomItems.map((item) => (
            <NavButton key={item.id} item={item} activeColor="violet" />
          ))}
        </div>
      </nav>

      {/* Version info */}
      <div className="px-4 pt-4 border-t border-white/10 mt-auto">
        <p className="text-xs text-zinc-500">Edge-Net v0.1.1</p>
        <p className="text-xs text-zinc-600">@ruvector/edge-net</p>
      </div>
    </div>
  );

  // Mobile: Slide-in drawer
  if (isMobile) {
    return (
      <AnimatePresence>
        {isOpen && (
          <>
            {/* Backdrop */}
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="fixed inset-0 bg-black/60 backdrop-blur-sm z-40"
              onClick={onClose}
            />

            {/* Drawer */}
            <motion.aside
              initial={{ x: -280 }}
              animate={{ x: 0 }}
              exit={{ x: -280 }}
              transition={{ type: 'spring', damping: 25, stiffness: 200 }}
              className="fixed left-0 top-0 bottom-0 w-[280px] bg-zinc-900/95 backdrop-blur-xl border-r border-white/10 z-50"
            >
              {content}
            </motion.aside>
          </>
        )}
      </AnimatePresence>
    );
  }

  // Desktop: Static sidebar
  return (
    <aside className="w-[240px] bg-zinc-900/50 backdrop-blur-xl border-r border-white/10 flex-shrink-0">
      {content}
    </aside>
  );
}
