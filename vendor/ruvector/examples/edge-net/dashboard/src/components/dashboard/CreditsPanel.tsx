import { Card, CardBody, Button, Progress } from '@heroui/react';
import { motion } from 'framer-motion';
import { Coins, ArrowUpRight, ArrowDownRight, Clock, Wallet, TrendingUp } from 'lucide-react';
import { useNetworkStore } from '../../stores/networkStore';

export function CreditsPanel() {
  const { credits, stats } = useNetworkStore();

  const transactions = [
    { id: '1', type: 'earn' as const, amount: 25.50, description: 'Compute contribution', time: '2 min ago' },
    { id: '2', type: 'earn' as const, amount: 12.75, description: 'Task completion bonus', time: '15 min ago' },
    { id: '3', type: 'spend' as const, amount: -5.00, description: 'API request', time: '1 hour ago' },
    { id: '4', type: 'earn' as const, amount: 45.00, description: 'Neural training reward', time: '2 hours ago' },
    { id: '5', type: 'spend' as const, amount: -15.00, description: 'Premium feature', time: '3 hours ago' },
  ];

  return (
    <div className="space-y-6">
      {/* Balance Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <Card className="bg-gradient-to-br from-emerald-500/20 to-emerald-600/10 border border-emerald-500/30">
            <CardBody className="p-5">
              <div className="flex items-center justify-between mb-2">
                <Wallet className="text-emerald-400" size={24} />
                <span className="text-xs text-emerald-400/70">Available</span>
              </div>
              <p className="text-3xl font-bold text-white">{credits.available.toFixed(2)}</p>
              <p className="text-sm text-emerald-400 mt-1">Credits</p>
            </CardBody>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
        >
          <Card className="bg-gradient-to-br from-amber-500/20 to-amber-600/10 border border-amber-500/30">
            <CardBody className="p-5">
              <div className="flex items-center justify-between mb-2">
                <Clock className="text-amber-400" size={24} />
                <span className="text-xs text-amber-400/70">Pending</span>
              </div>
              <p className="text-3xl font-bold text-white">{credits.pending.toFixed(2)}</p>
              <p className="text-sm text-amber-400 mt-1">Credits</p>
            </CardBody>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
        >
          <Card className="bg-gradient-to-br from-sky-500/20 to-sky-600/10 border border-sky-500/30">
            <CardBody className="p-5">
              <div className="flex items-center justify-between mb-2">
                <TrendingUp className="text-sky-400" size={24} />
                <span className="text-xs text-sky-400/70">Total Earned</span>
              </div>
              <p className="text-3xl font-bold text-white">{credits.earned.toFixed(2)}</p>
              <p className="text-sm text-sky-400 mt-1">Credits</p>
            </CardBody>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3 }}
        >
          <Card className="bg-gradient-to-br from-violet-500/20 to-violet-600/10 border border-violet-500/30">
            <CardBody className="p-5">
              <div className="flex items-center justify-between mb-2">
                <Coins className="text-violet-400" size={24} />
                <span className="text-xs text-violet-400/70">Net Balance</span>
              </div>
              <p className="text-3xl font-bold text-white">
                {(credits.earned - credits.spent).toFixed(2)}
              </p>
              <p className="text-sm text-violet-400 mt-1">Credits</p>
            </CardBody>
          </Card>
        </motion.div>
      </div>

      {/* Earning Progress */}
      <motion.div
        className="crystal-card p-6"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.4 }}
      >
        <h3 className="text-lg font-semibold mb-4">Daily Earning Progress</h3>
        <div className="space-y-4">
          <div>
            <div className="flex justify-between text-sm mb-2">
              <span className="text-zinc-400">Compute Contribution</span>
              <span className="text-emerald-400">45.8 / 100 TFLOPS</span>
            </div>
            <Progress
              value={45.8}
              maxValue={100}
              classNames={{
                indicator: 'bg-gradient-to-r from-emerald-500 to-cyan-500',
                track: 'bg-zinc-800',
              }}
            />
          </div>

          <div>
            <div className="flex justify-between text-sm mb-2">
              <span className="text-zinc-400">Tasks Completed</span>
              <span className="text-sky-400">89,432 / 100,000</span>
            </div>
            <Progress
              value={89.432}
              maxValue={100}
              classNames={{
                indicator: 'bg-gradient-to-r from-sky-500 to-violet-500',
                track: 'bg-zinc-800',
              }}
            />
          </div>

          <div>
            <div className="flex justify-between text-sm mb-2">
              <span className="text-zinc-400">Uptime Bonus</span>
              <span className="text-violet-400">{stats.uptime.toFixed(1)}%</span>
            </div>
            <Progress
              value={stats.uptime}
              maxValue={100}
              classNames={{
                indicator: 'bg-gradient-to-r from-violet-500 to-pink-500',
                track: 'bg-zinc-800',
              }}
            />
          </div>
        </div>
      </motion.div>

      {/* Recent Transactions */}
      <motion.div
        className="crystal-card p-6"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.5 }}
      >
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold">Recent Transactions</h3>
          <Button size="sm" variant="flat" className="bg-white/5 text-zinc-400">
            View All
          </Button>
        </div>

        <div className="space-y-3">
          {transactions.map((tx) => (
            <div
              key={tx.id}
              className="flex items-center justify-between p-3 rounded-lg bg-zinc-800/50"
            >
              <div className="flex items-center gap-3">
                <div
                  className={`p-2 rounded-full ${
                    tx.type === 'earn' ? 'bg-emerald-500/20' : 'bg-red-500/20'
                  }`}
                >
                  {tx.type === 'earn' ? (
                    <ArrowUpRight className="text-emerald-400" size={16} />
                  ) : (
                    <ArrowDownRight className="text-red-400" size={16} />
                  )}
                </div>
                <div>
                  <p className="text-sm font-medium text-white">{tx.description}</p>
                  <p className="text-xs text-zinc-500">{tx.time}</p>
                </div>
              </div>
              <span
                className={`font-semibold ${
                  tx.type === 'earn' ? 'text-emerald-400' : 'text-red-400'
                }`}
              >
                {tx.type === 'earn' ? '+' : ''}{tx.amount.toFixed(2)}
              </span>
            </div>
          ))}
        </div>
      </motion.div>
    </div>
  );
}
