import { Card, CardBody } from '@heroui/react';
import { motion } from 'framer-motion';
import type { ReactNode } from 'react';

interface StatCardProps {
  title: string;
  value: string | number;
  change?: number;
  icon?: ReactNode;
  color?: 'crystal' | 'temporal' | 'quantum' | 'success' | 'warning' | 'danger';
  size?: 'sm' | 'md' | 'lg';
  animated?: boolean;
}

const colorClasses = {
  crystal: 'from-sky-500/20 to-sky-600/10 border-sky-500/30',
  temporal: 'from-violet-500/20 to-violet-600/10 border-violet-500/30',
  quantum: 'from-cyan-500/20 to-cyan-600/10 border-cyan-500/30',
  success: 'from-emerald-500/20 to-emerald-600/10 border-emerald-500/30',
  warning: 'from-amber-500/20 to-amber-600/10 border-amber-500/30',
  danger: 'from-red-500/20 to-red-600/10 border-red-500/30',
};

const iconColorClasses = {
  crystal: 'text-sky-400',
  temporal: 'text-violet-400',
  quantum: 'text-cyan-400',
  success: 'text-emerald-400',
  warning: 'text-amber-400',
  danger: 'text-red-400',
};

export function StatCard({
  title,
  value,
  change,
  icon,
  color = 'crystal',
  size = 'md',
  animated = true,
}: StatCardProps) {
  const sizeClasses = {
    sm: 'p-3',
    md: 'p-4',
    lg: 'p-6',
  };

  const valueSizeClasses = {
    sm: 'text-xl',
    md: 'text-2xl',
    lg: 'text-4xl',
  };

  return (
    <motion.div
      initial={animated ? { opacity: 0, y: 20 } : false}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3 }}
    >
      <Card
        className={`crystal-card bg-gradient-to-br ${colorClasses[color]} border`}
      >
        <CardBody className={sizeClasses[size]}>
          <div className="flex items-start justify-between">
            <div className="flex-1">
              <p className="text-sm text-zinc-400 mb-1">{title}</p>
              <motion.p
                className={`${valueSizeClasses[size]} font-bold stat-value text-white`}
                key={String(value)}
                initial={animated ? { scale: 1.1, opacity: 0.5 } : false}
                animate={{ scale: 1, opacity: 1 }}
                transition={{ duration: 0.2 }}
              >
                {typeof value === 'number' ? value.toLocaleString() : value}
              </motion.p>
              {change !== undefined && (
                <p
                  className={`text-sm mt-1 ${
                    change >= 0 ? 'text-emerald-400' : 'text-red-400'
                  }`}
                >
                  {change >= 0 ? '↑' : '↓'} {Math.abs(change).toFixed(1)}%
                </p>
              )}
            </div>
            {icon && (
              <div className={`${iconColorClasses[color]} opacity-80`}>
                {icon}
              </div>
            )}
          </div>
        </CardBody>
      </Card>
    </motion.div>
  );
}
