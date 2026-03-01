import { useEffect, useState } from 'react';
import { Button, Chip, Input, ScrollShadow } from '@heroui/react';
import { motion } from 'framer-motion';
import { Terminal, Trash2, Download, Filter, Info, AlertTriangle, XCircle, Bug } from 'lucide-react';
import { subscribeToLogs, clearLogs } from '../../utils/debug';
import type { DebugLog } from '../../types';

const levelIcons = {
  info: <Info size={14} />,
  warn: <AlertTriangle size={14} />,
  error: <XCircle size={14} />,
  debug: <Bug size={14} />,
};

const levelColors = {
  info: 'text-sky-400',
  warn: 'text-amber-400',
  error: 'text-red-400',
  debug: 'text-violet-400',
};

const levelBg = {
  info: 'bg-sky-500/10 border-sky-500/30',
  warn: 'bg-amber-500/10 border-amber-500/30',
  error: 'bg-red-500/10 border-red-500/30',
  debug: 'bg-violet-500/10 border-violet-500/30',
};

export function ConsolePanel() {
  const [logs, setLogs] = useState<DebugLog[]>([]);
  const [filter, setFilter] = useState('');
  const [levelFilter, setLevelFilter] = useState<string>('all');

  useEffect(() => {
    const unsubscribe = subscribeToLogs(setLogs);
    return unsubscribe;
  }, []);

  const filteredLogs = logs.filter((log) => {
    const matchesText =
      filter === '' ||
      log.message.toLowerCase().includes(filter.toLowerCase()) ||
      log.source.toLowerCase().includes(filter.toLowerCase());
    const matchesLevel = levelFilter === 'all' || log.level === levelFilter;
    return matchesText && matchesLevel;
  });

  const handleExport = () => {
    const data = JSON.stringify(logs, null, 2);
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `edge-net-logs-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const logCounts = logs.reduce(
    (acc, log) => {
      acc[log.level] = (acc[log.level] || 0) + 1;
      return acc;
    },
    {} as Record<string, number>
  );

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex flex-col md:flex-row gap-4 items-start md:items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-zinc-800">
            <Terminal className="text-emerald-400" size={20} />
          </div>
          <div>
            <h2 className="text-lg font-semibold text-white">Debug Console</h2>
            <p className="text-xs text-zinc-500">{logs.length} entries</p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Chip
            size="sm"
            variant="flat"
            className="bg-sky-500/20 text-sky-400"
          >
            {logCounts.info || 0} info
          </Chip>
          <Chip
            size="sm"
            variant="flat"
            className="bg-amber-500/20 text-amber-400"
          >
            {logCounts.warn || 0} warn
          </Chip>
          <Chip
            size="sm"
            variant="flat"
            className="bg-red-500/20 text-red-400"
          >
            {logCounts.error || 0} error
          </Chip>
        </div>
      </div>

      {/* Controls */}
      <div className="flex flex-col md:flex-row gap-3">
        <Input
          placeholder="Filter logs..."
          value={filter}
          onValueChange={setFilter}
          startContent={<Filter size={16} className="text-zinc-400" />}
          classNames={{
            input: 'bg-transparent',
            inputWrapper: 'bg-zinc-900/50 border border-white/10',
          }}
          className="flex-1"
        />

        <div className="flex gap-2">
          <Button
            size="sm"
            variant={levelFilter === 'all' ? 'solid' : 'flat'}
            className={levelFilter === 'all' ? 'bg-zinc-700' : 'bg-zinc-900'}
            onPress={() => setLevelFilter('all')}
          >
            All
          </Button>
          {(['info', 'warn', 'error', 'debug'] as const).map((level) => (
            <Button
              key={level}
              size="sm"
              variant={levelFilter === level ? 'solid' : 'flat'}
              className={levelFilter === level ? levelBg[level] : 'bg-zinc-900'}
              onPress={() => setLevelFilter(level)}
            >
              {levelIcons[level]}
            </Button>
          ))}
        </div>

        <div className="flex gap-2">
          <Button
            size="sm"
            variant="flat"
            className="bg-zinc-900"
            startContent={<Download size={14} />}
            onPress={handleExport}
          >
            Export
          </Button>
          <Button
            size="sm"
            variant="flat"
            className="bg-red-500/20 text-red-400"
            startContent={<Trash2 size={14} />}
            onPress={clearLogs}
          >
            Clear
          </Button>
        </div>
      </div>

      {/* Log List */}
      <div className="crystal-card overflow-hidden">
        <ScrollShadow className="h-[500px]">
          <div className="font-mono text-sm">
            {filteredLogs.length === 0 ? (
              <div className="p-8 text-center text-zinc-500">
                <Terminal size={32} className="mx-auto mb-2 opacity-50" />
                <p>No logs to display</p>
              </div>
            ) : (
              filteredLogs.map((log, idx) => (
                <motion.div
                  key={log.id}
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: idx * 0.01 }}
                  className={`flex items-start gap-3 p-3 border-b border-white/5 hover:bg-white/5 ${
                    log.level === 'error' ? 'bg-red-500/5' : ''
                  }`}
                >
                  <span className={`flex-shrink-0 ${levelColors[log.level]}`}>
                    {levelIcons[log.level]}
                  </span>

                  <span className="text-zinc-500 flex-shrink-0 w-20">
                    {new Date(log.timestamp).toLocaleTimeString()}
                  </span>

                  <span className="text-zinc-600 flex-shrink-0 w-16">
                    [{log.source}]
                  </span>

                  <span className="text-zinc-300 break-all flex-1">
                    {log.message}
                  </span>

                  {log.data !== undefined && (
                    <button
                      className="text-xs text-zinc-500 hover:text-zinc-300"
                      onClick={() => console.log('Log data:', log.data)}
                    >
                      [data]
                    </button>
                  )}
                </motion.div>
              ))
            )}
          </div>
        </ScrollShadow>
      </div>

      {/* Instructions */}
      <div className="text-xs text-zinc-500 p-3 rounded-lg bg-zinc-900/50 border border-white/5">
        <p className="font-medium text-zinc-400 mb-1">Debug Commands:</p>
        <code className="text-sky-400">window.edgeNet.logs()</code> - View all logs<br />
        <code className="text-sky-400">window.edgeNet.clear()</code> - Clear logs<br />
        <code className="text-sky-400">window.edgeNet.stats()</code> - View log statistics<br />
        <code className="text-sky-400">window.edgeNet.export()</code> - Export logs as JSON
      </div>
    </div>
  );
}
