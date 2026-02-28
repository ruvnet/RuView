import { Button, Card, CardBody, Chip, Input, Tabs, Tab, ScrollShadow } from '@heroui/react';
import { motion } from 'framer-motion';
import { Play, Search, Users, Brain, Database, GitBranch, ListTodo, Loader2, Check, X } from 'lucide-react';
import { useState, useMemo } from 'react';
import { useMCPStore } from '../../stores/mcpStore';
import type { MCPTool } from '../../types';

const categoryIcons = {
  swarm: <Users size={16} />,
  agent: <Brain size={16} />,
  memory: <Database size={16} />,
  neural: <Brain size={16} />,
  task: <ListTodo size={16} />,
  github: <GitBranch size={16} />,
};

const categoryColors = {
  swarm: 'from-sky-500/20 to-sky-600/10 border-sky-500/30',
  agent: 'from-violet-500/20 to-violet-600/10 border-violet-500/30',
  memory: 'from-cyan-500/20 to-cyan-600/10 border-cyan-500/30',
  neural: 'from-emerald-500/20 to-emerald-600/10 border-emerald-500/30',
  task: 'from-amber-500/20 to-amber-600/10 border-amber-500/30',
  github: 'from-zinc-500/20 to-zinc-600/10 border-zinc-500/30',
};

const statusColors = {
  ready: 'bg-emerald-500/20 text-emerald-400',
  running: 'bg-sky-500/20 text-sky-400',
  error: 'bg-red-500/20 text-red-400',
  disabled: 'bg-zinc-500/20 text-zinc-400',
};

export function MCPTools() {
  const { tools, results, activeTools, isConnected, executeTool } = useMCPStore();
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');

  const categories = useMemo(() => {
    const cats = [...new Set(tools.map((t) => t.category))];
    return ['all', ...cats];
  }, [tools]);

  const filteredTools = useMemo(() => {
    return tools.filter((tool) => {
      const matchesSearch =
        tool.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        tool.description.toLowerCase().includes(searchQuery.toLowerCase());
      const matchesCategory = selectedCategory === 'all' || tool.category === selectedCategory;
      return matchesSearch && matchesCategory;
    });
  }, [tools, searchQuery, selectedCategory]);

  const handleExecute = async (tool: MCPTool) => {
    console.log(`[MCP] Executing tool: ${tool.id}`);
    await executeTool(tool.id);
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col md:flex-row gap-4 items-start md:items-center justify-between">
        <div>
          <h2 className="text-xl font-bold text-white">MCP Tools</h2>
          <p className="text-sm text-zinc-400">
            Execute Model Context Protocol tools for swarm coordination
          </p>
        </div>

        <Chip
          variant="flat"
          className={isConnected ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400'}
        >
          {isConnected ? 'Connected' : 'Disconnected'}
        </Chip>
      </div>

      {/* Search and Filters */}
      <div className="flex flex-col md:flex-row gap-4">
        <Input
          placeholder="Search tools..."
          value={searchQuery}
          onValueChange={setSearchQuery}
          startContent={<Search size={18} className="text-zinc-400" />}
          classNames={{
            input: 'bg-transparent',
            inputWrapper: 'bg-zinc-900/50 border border-white/10',
          }}
          className="flex-1"
        />

        <Tabs
          selectedKey={selectedCategory}
          onSelectionChange={(key) => setSelectedCategory(key as string)}
          variant="bordered"
          classNames={{
            tabList: 'bg-zinc-900/50 border-white/10',
            cursor: 'bg-sky-500/20',
            tab: 'text-zinc-400 data-[selected=true]:text-sky-400',
          }}
        >
          {categories.map((cat) => (
            <Tab
              key={cat}
              title={
                <div className="flex items-center gap-1.5">
                  {cat !== 'all' && categoryIcons[cat as keyof typeof categoryIcons]}
                  <span className="capitalize">{cat}</span>
                </div>
              }
            />
          ))}
        </Tabs>
      </div>

      {/* Tools Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {filteredTools.map((tool, idx) => {
          const isActive = activeTools.includes(tool.id);

          return (
            <motion.div
              key={tool.id}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: idx * 0.05 }}
            >
              <Card
                className={`bg-gradient-to-br ${categoryColors[tool.category]} border ${
                  isActive ? 'ring-2 ring-sky-500/50' : ''
                }`}
              >
                <CardBody className="p-4">
                  <div className="flex items-start justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <div className="p-1.5 rounded bg-white/5">
                        {categoryIcons[tool.category]}
                      </div>
                      <div>
                        <h4 className="font-medium text-white">{tool.name}</h4>
                        <p className="text-xs text-zinc-500">{tool.id}</p>
                      </div>
                    </div>
                    <Chip size="sm" variant="flat" className={statusColors[tool.status]}>
                      {isActive ? (
                        <Loader2 size={12} className="animate-spin" />
                      ) : tool.status === 'ready' ? (
                        <Check size={12} />
                      ) : tool.status === 'error' ? (
                        <X size={12} />
                      ) : null}
                    </Chip>
                  </div>

                  <p className="text-sm text-zinc-400 mb-4 line-clamp-2">
                    {tool.description}
                  </p>

                  <div className="flex items-center justify-between">
                    {tool.lastRun && (
                      <span className="text-xs text-zinc-500">
                        Last: {new Date(tool.lastRun).toLocaleTimeString()}
                      </span>
                    )}
                    <Button
                      size="sm"
                      variant="flat"
                      className="bg-white/10 text-white hover:bg-white/20 ml-auto"
                      isDisabled={isActive || tool.status === 'disabled'}
                      isLoading={isActive}
                      startContent={!isActive && <Play size={14} />}
                      onPress={() => handleExecute(tool)}
                    >
                      Execute
                    </Button>
                  </div>
                </CardBody>
              </Card>
            </motion.div>
          );
        })}
      </div>

      {/* Recent Results */}
      {results.length > 0 && (
        <div className="crystal-card p-4">
          <h3 className="text-lg font-semibold mb-3">Recent Results</h3>
          <ScrollShadow className="max-h-[200px]">
            <div className="space-y-2">
              {results.slice(0, 10).map((result, idx) => (
                <div
                  key={idx}
                  className={`p-3 rounded-lg border ${
                    result.success
                      ? 'bg-emerald-500/10 border-emerald-500/30'
                      : 'bg-red-500/10 border-red-500/30'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-medium text-white">{result.toolId}</span>
                    <span className="text-xs text-zinc-400">
                      {result.duration.toFixed(0)}ms
                    </span>
                  </div>
                  {result.error && (
                    <p className="text-xs text-red-400 mt-1">{result.error}</p>
                  )}
                </div>
              ))}
            </div>
          </ScrollShadow>
        </div>
      )}
    </div>
  );
}
