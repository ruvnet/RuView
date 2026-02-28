import { Chip } from '@heroui/react';
import { motion } from 'framer-motion';
import type { ReactNode } from 'react';

interface GlowingBadgeProps {
  children: ReactNode;
  color?: 'crystal' | 'temporal' | 'quantum' | 'success' | 'warning' | 'danger';
  variant?: 'solid' | 'bordered' | 'flat';
  size?: 'sm' | 'md' | 'lg';
  startContent?: ReactNode;
  endContent?: ReactNode;
  animate?: boolean;
}

const glowColors = {
  crystal: 'shadow-sky-500/50',
  temporal: 'shadow-violet-500/50',
  quantum: 'shadow-cyan-500/50',
  success: 'shadow-emerald-500/50',
  warning: 'shadow-amber-500/50',
  danger: 'shadow-red-500/50',
};

const bgColors = {
  crystal: 'bg-sky-500/20 border-sky-500/50 text-sky-300',
  temporal: 'bg-violet-500/20 border-violet-500/50 text-violet-300',
  quantum: 'bg-cyan-500/20 border-cyan-500/50 text-cyan-300',
  success: 'bg-emerald-500/20 border-emerald-500/50 text-emerald-300',
  warning: 'bg-amber-500/20 border-amber-500/50 text-amber-300',
  danger: 'bg-red-500/20 border-red-500/50 text-red-300',
};

export function GlowingBadge({
  children,
  color = 'crystal',
  variant = 'flat',
  size = 'md',
  startContent,
  endContent,
  animate = false,
}: GlowingBadgeProps) {
  const Component = animate ? motion.div : 'div';

  return (
    <Component
      {...(animate
        ? {
            animate: { boxShadow: ['0 0 10px', '0 0 20px', '0 0 10px'] },
            transition: { duration: 2, repeat: Infinity },
          }
        : {})}
      className={`inline-block ${animate ? glowColors[color] : ''}`}
    >
      <Chip
        variant={variant}
        size={size}
        startContent={startContent}
        endContent={endContent}
        classNames={{
          base: `${bgColors[color]} border`,
          content: 'font-medium',
        }}
      >
        {children}
      </Chip>
    </Component>
  );
}
