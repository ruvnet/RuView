import { useState } from 'react';
import { Card, CardBody } from '@heroui/react';
import { CircleDot, Link2 } from 'lucide-react';

interface GraphVisualizationProps {
  nodes: Array<{
    id: string;
    labels: string[];
    properties: Record<string, unknown>;
  }>;
  relationships: Array<{
    id: string;
    type: string;
    start: string;
    end: string;
    properties: Record<string, unknown>;
  }>;
}

export function GraphVisualization({ nodes, relationships }: GraphVisualizationProps) {
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const [hoveredRel, setHoveredRel] = useState<string | null>(null);

  // Simple circular layout
  const layoutNodes = () => {
    const radius = 150;
    const centerX = 300;
    const centerY = 200;
    const angleStep = (2 * Math.PI) / Math.max(nodes.length, 1);

    return nodes.map((node, index) => {
      const angle = index * angleStep;
      return {
        ...node,
        x: centerX + radius * Math.cos(angle),
        y: centerY + radius * Math.sin(angle),
      };
    });
  };

  const layoutedNodes = layoutNodes();
  const nodePositions = new Map(layoutedNodes.map(n => [n.id, { x: n.x, y: n.y }]));

  // Color palette for node labels
  const labelColors: Record<string, string> = {
    Person: '#00e68a',
    Movie: '#7c3aed',
    Actor: '#ff6b9d',
    Director: '#fbbf24',
    City: '#3b82f6',
    Country: '#10b981',
  };

  const getNodeColor = (labels: string[]) => {
    if (labels.length === 0) return '#6b7280';
    return labelColors[labels[0]] || '#6b7280';
  };

  const hoveredNodeData = hoveredNode ? nodes.find(n => n.id === hoveredNode) : null;
  const hoveredRelData = hoveredRel ? relationships.find(r => r.id === hoveredRel) : null;

  return (
    <div className="relative w-full h-full">
      <svg width="100%" height="400" className="bg-gray-950/50 rounded-lg">
        {/* Relationships (lines) */}
        {relationships.map(rel => {
          const start = nodePositions.get(rel.start);
          const end = nodePositions.get(rel.end);
          if (!start || !end) return null;

          const isHovered = hoveredRel === rel.id;
          const midX = (start.x + end.x) / 2;
          const midY = (start.y + end.y) / 2;

          return (
            <g key={rel.id}>
              {/* Line */}
              <line
                x1={start.x}
                y1={start.y}
                x2={end.x}
                y2={end.y}
                stroke={isHovered ? '#00e68a' : '#4b5563'}
                strokeWidth={isHovered ? 3 : 2}
                markerEnd="url(#arrowhead)"
                className="cursor-pointer transition-all"
                onMouseEnter={() => setHoveredRel(rel.id)}
                onMouseLeave={() => setHoveredRel(null)}
              />
              {/* Relationship label */}
              <text
                x={midX}
                y={midY - 5}
                fill={isHovered ? '#00e68a' : '#9ca3af'}
                fontSize="10"
                textAnchor="middle"
                className="pointer-events-none select-none"
              >
                {rel.type}
              </text>
            </g>
          );
        })}

        {/* Arrow marker definition */}
        <defs>
          <marker
            id="arrowhead"
            markerWidth="10"
            markerHeight="10"
            refX="8"
            refY="3"
            orient="auto"
          >
            <polygon points="0 0, 10 3, 0 6" fill="#4b5563" />
          </marker>
        </defs>

        {/* Nodes */}
        {layoutedNodes.map(node => {
          const isHovered = hoveredNode === node.id;
          const color = getNodeColor(node.labels);
          const label = node.labels[0] || 'Node';
          const nameProperty = node.properties.name || node.properties.title || node.id;

          return (
            <g key={node.id}>
              {/* Node circle */}
              <circle
                cx={node.x}
                cy={node.y}
                r={isHovered ? 25 : 20}
                fill={color}
                fillOpacity={0.2}
                stroke={color}
                strokeWidth={isHovered ? 3 : 2}
                className="cursor-pointer transition-all"
                onMouseEnter={() => setHoveredNode(node.id)}
                onMouseLeave={() => setHoveredNode(null)}
              />
              {/* Node label */}
              <text
                x={node.x}
                y={node.y + 35}
                fill="#e5e7eb"
                fontSize="11"
                fontWeight="600"
                textAnchor="middle"
                className="pointer-events-none select-none"
              >
                {label}
              </text>
              {/* Node name/title */}
              <text
                x={node.x}
                y={node.y + 48}
                fill="#9ca3af"
                fontSize="9"
                textAnchor="middle"
                className="pointer-events-none select-none"
              >
                {String(nameProperty).substring(0, 15)}
              </text>
            </g>
          );
        })}
      </svg>

      {/* Tooltip for hovered node */}
      {hoveredNodeData && (
        <Card className="absolute top-2 right-2 bg-gray-800 border border-gray-700 max-w-xs z-10">
          <CardBody className="p-3">
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <CircleDot className="w-4 h-4" style={{ color: getNodeColor(hoveredNodeData.labels) }} />
                <span className="font-semibold text-sm">{hoveredNodeData.labels.join(', ') || 'Node'}</span>
              </div>
              <div className="text-xs text-gray-400">ID: {hoveredNodeData.id}</div>
              {Object.keys(hoveredNodeData.properties).length > 0 && (
                <div className="mt-2 space-y-1">
                  <div className="text-xs font-semibold text-gray-300">Properties:</div>
                  {Object.entries(hoveredNodeData.properties).map(([key, value]) => (
                    <div key={key} className="text-xs text-gray-400 flex gap-2">
                      <span className="font-mono text-cyan-400">{key}:</span>
                      <span className="truncate">{String(value)}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </CardBody>
        </Card>
      )}

      {/* Tooltip for hovered relationship */}
      {hoveredRelData && !hoveredNodeData && (
        <Card className="absolute top-2 right-2 bg-gray-800 border border-gray-700 max-w-xs z-10">
          <CardBody className="p-3">
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Link2 className="w-4 h-4 text-cyan-400" />
                <span className="font-semibold text-sm">{hoveredRelData.type}</span>
              </div>
              <div className="text-xs text-gray-400">
                {hoveredRelData.start} → {hoveredRelData.end}
              </div>
              {Object.keys(hoveredRelData.properties).length > 0 && (
                <div className="mt-2 space-y-1">
                  <div className="text-xs font-semibold text-gray-300">Properties:</div>
                  {Object.entries(hoveredRelData.properties).map(([key, value]) => (
                    <div key={key} className="text-xs text-gray-400 flex gap-2">
                      <span className="font-mono text-cyan-400">{key}:</span>
                      <span className="truncate">{String(value)}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </CardBody>
        </Card>
      )}

      {/* Legend */}
      <div className="mt-4 flex flex-wrap gap-3">
        {Array.from(new Set(nodes.flatMap(n => n.labels))).map(label => (
          <div key={label} className="flex items-center gap-2">
            <div
              className="w-3 h-3 rounded-full"
              style={{ backgroundColor: labelColors[label] || '#6b7280' }}
            />
            <span className="text-xs text-gray-400">{label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
