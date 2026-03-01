import { Button } from '@heroui/react';
import { motion } from 'framer-motion';
import { Activity, Wifi, WifiOff, Sun, Menu } from 'lucide-react';
import { useNetworkStore } from '../../stores/networkStore';

interface HeaderProps {
  onMenuToggle?: () => void;
  isMobile?: boolean;
}

function StatusChip({
  icon,
  label,
  colorClass
}: {
  icon: React.ReactNode;
  label: string;
  colorClass: string;
}) {
  return (
    <div className={`
      inline-flex items-center gap-2 px-3 py-1.5 rounded-full
      border text-xs font-medium
      ${colorClass}
    `}>
      <span className="flex-shrink-0 flex items-center">{icon}</span>
      <span>{label}</span>
    </div>
  );
}

export function Header({ onMenuToggle, isMobile }: HeaderProps) {
  const { isConnected, stats } = useNetworkStore();

  // Defensive defaults for stats
  const totalCompute = stats?.totalCompute ?? 0;
  const activeNodes = stats?.activeNodes ?? 0;

  return (
    <header className="h-16 bg-zinc-900/50 backdrop-blur-xl border-b border-white/10 px-4 flex items-center">
      {/* Left section */}
      <div className="flex items-center gap-3">
        {isMobile && onMenuToggle && (
          <Button
            isIconOnly
            variant="light"
            onPress={onMenuToggle}
            className="text-zinc-400 hover:text-white"
          >
            <Menu size={20} />
          </Button>
        )}

        {/* Crystal Logo */}
        <motion.div
          className="relative w-10 h-10 flex-shrink-0"
          animate={{ rotate: 360 }}
          transition={{ duration: 20, repeat: Infinity, ease: 'linear' }}
        >
          <div
            className="absolute inset-0"
            style={{
              background: 'linear-gradient(135deg, #0ea5e9, #7c3aed, #06b6d4)',
              clipPath: 'polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)',
            }}
          />
          <motion.div
            className="absolute inset-2"
            style={{
              background: 'linear-gradient(135deg, #06b6d4, #0ea5e9)',
              clipPath: 'polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)',
            }}
            animate={{ opacity: [0.5, 1, 0.5] }}
            transition={{ duration: 2, repeat: Infinity }}
          />
        </motion.div>

        <div className="flex flex-col justify-center">
          <span className="font-bold text-lg leading-tight bg-gradient-to-r from-sky-400 via-violet-400 to-cyan-400 bg-clip-text text-transparent">
            Edge-Net
          </span>
          <span className="text-[10px] text-zinc-500 leading-tight">Collective AI Computing</span>
        </div>
      </div>

      {/* Center section - Stats */}
      <div className="flex-1 flex items-center justify-center gap-3 hidden md:flex">
        <StatusChip
          icon={<Activity size={14} />}
          label={`${totalCompute.toFixed(1)} TFLOPS`}
          colorClass="bg-sky-500/10 border-sky-500/30 text-sky-400"
        />

        <StatusChip
          icon={
            <motion.div
              animate={{ scale: [1, 1.2, 1] }}
              transition={{ duration: 1, repeat: Infinity }}
              className="w-2 h-2 rounded-full bg-emerald-400"
            />
          }
          label={`${activeNodes.toLocaleString()} nodes`}
          colorClass="bg-emerald-500/10 border-emerald-500/30 text-emerald-400"
        />
      </div>

      {/* Right section */}
      <div className="flex items-center gap-2">
        <motion.div
          animate={isConnected ? { opacity: [0.5, 1, 0.5] } : {}}
          transition={{ duration: 2, repeat: Infinity }}
        >
          <StatusChip
            icon={isConnected ? <Wifi size={14} /> : <WifiOff size={14} />}
            label={isConnected ? 'Connected' : 'Offline'}
            colorClass={isConnected
              ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
              : 'bg-red-500/10 border-red-500/30 text-red-400'
            }
          />
        </motion.div>

        <Button
          isIconOnly
          variant="light"
          className="text-zinc-400 hover:text-white hidden sm:flex"
        >
          <Sun size={18} />
        </Button>
      </div>
    </header>
  );
}
