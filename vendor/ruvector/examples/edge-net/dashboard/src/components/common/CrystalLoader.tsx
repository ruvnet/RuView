import { motion } from 'framer-motion';

interface CrystalLoaderProps {
  size?: 'sm' | 'md' | 'lg';
  text?: string;
}

const sizes = {
  sm: { container: 'w-8 h-8', crystal: 'w-4 h-4' },
  md: { container: 'w-16 h-16', crystal: 'w-8 h-8' },
  lg: { container: 'w-24 h-24', crystal: 'w-12 h-12' },
};

export function CrystalLoader({ size = 'md', text }: CrystalLoaderProps) {
  const { container, crystal } = sizes[size];

  return (
    <div className="flex flex-col items-center gap-4">
      <div className={`${container} relative`}>
        {/* Outer rotating ring */}
        <motion.div
          className="absolute inset-0 rounded-full border-2 border-sky-500/30"
          animate={{ rotate: 360 }}
          transition={{ duration: 3, repeat: Infinity, ease: 'linear' }}
        />

        {/* Middle rotating ring (opposite direction) */}
        <motion.div
          className="absolute inset-2 rounded-full border-2 border-violet-500/30"
          animate={{ rotate: -360 }}
          transition={{ duration: 2, repeat: Infinity, ease: 'linear' }}
        />

        {/* Inner pulsing crystal */}
        <motion.div
          className={`absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 ${crystal}`}
          style={{
            background: 'linear-gradient(135deg, #0ea5e9, #7c3aed, #06b6d4)',
            clipPath: 'polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)',
          }}
          animate={{
            scale: [1, 1.2, 1],
            opacity: [0.8, 1, 0.8],
          }}
          transition={{
            duration: 1.5,
            repeat: Infinity,
            ease: 'easeInOut',
          }}
        />

        {/* Glow effect */}
        <motion.div
          className="absolute inset-0 rounded-full"
          style={{
            background:
              'radial-gradient(circle, rgba(14,165,233,0.3) 0%, transparent 70%)',
          }}
          animate={{
            opacity: [0.3, 0.6, 0.3],
            scale: [1, 1.1, 1],
          }}
          transition={{
            duration: 2,
            repeat: Infinity,
            ease: 'easeInOut',
          }}
        />
      </div>

      {text && (
        <motion.p
          className="text-sm text-zinc-400"
          animate={{ opacity: [0.5, 1, 0.5] }}
          transition={{ duration: 1.5, repeat: Infinity }}
        >
          {text}
        </motion.p>
      )}
    </div>
  );
}
