/**
 * Tests for Graph Export Module
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  buildGraphFromEntries,
  exportToGraphML,
  exportToGEXF,
  exportToNeo4j,
  exportToD3,
  exportToNetworkX,
  exportGraph,
  validateGraph,
  type VectorEntry,
  type Graph,
  type GraphNode,
  type GraphEdge
} from '../src/exporters.js';

// Sample test data
const sampleEntries: VectorEntry[] = [
  {
    id: 'vec1',
    vector: [1.0, 0.0, 0.0],
    metadata: { label: 'Vector 1', category: 'A' }
  },
  {
    id: 'vec2',
    vector: [0.9, 0.1, 0.0],
    metadata: { label: 'Vector 2', category: 'A' }
  },
  {
    id: 'vec3',
    vector: [0.0, 1.0, 0.0],
    metadata: { label: 'Vector 3', category: 'B' }
  }
];

const sampleGraph: Graph = {
  nodes: [
    { id: 'n1', label: 'Node 1', attributes: { type: 'test' } },
    { id: 'n2', label: 'Node 2', attributes: { type: 'test' } }
  ],
  edges: [
    { source: 'n1', target: 'n2', weight: 0.95, type: 'similar' }
  ]
};

describe('Graph Building', () => {
  it('should build graph from vector entries', () => {
    const graph = buildGraphFromEntries(sampleEntries, {
      maxNeighbors: 2,
      threshold: 0.5
    });

    assert.strictEqual(graph.nodes.length, 3, 'Should have 3 nodes');
    assert.ok(graph.edges.length > 0, 'Should have edges');
    assert.ok(graph.metadata, 'Should have metadata');
  });

  it('should respect threshold parameter', () => {
    const highThreshold = buildGraphFromEntries(sampleEntries, {
      threshold: 0.95
    });

    const lowThreshold = buildGraphFromEntries(sampleEntries, {
      threshold: 0.1
    });

    assert.ok(
      highThreshold.edges.length <= lowThreshold.edges.length,
      'Higher threshold should result in fewer edges'
    );
  });

  it('should respect maxNeighbors parameter', () => {
    const graph = buildGraphFromEntries(sampleEntries, {
      maxNeighbors: 1,
      threshold: 0.0
    });

    // Each node should have at most 1 outgoing edge
    const outgoingEdges = new Map<string, number>();
    for (const edge of graph.edges) {
      outgoingEdges.set(edge.source, (outgoingEdges.get(edge.source) || 0) + 1);
    }

    for (const count of outgoingEdges.values()) {
      assert.ok(count <= 1, 'Should respect maxNeighbors limit');
    }
  });

  it('should include metadata when requested', () => {
    const graph = buildGraphFromEntries(sampleEntries, {
      includeMetadata: true
    });

    const nodeWithMetadata = graph.nodes.find(n => n.attributes);
    assert.ok(nodeWithMetadata, 'Should include metadata in nodes');
    assert.ok(nodeWithMetadata!.attributes!.category, 'Should preserve metadata fields');
  });

  it('should include vectors when requested', () => {
    const graph = buildGraphFromEntries(sampleEntries, {
      includeVectors: true
    });

    const nodeWithVector = graph.nodes.find(n => n.vector);
    assert.ok(nodeWithVector, 'Should include vectors in nodes');
    assert.ok(Array.isArray(nodeWithVector!.vector), 'Vector should be an array');
  });
});

describe('GraphML Export', () => {
  it('should export valid GraphML XML', () => {
    const graphML = exportToGraphML(sampleGraph);

    assert.ok(graphML.includes('<?xml'), 'Should have XML declaration');
    assert.ok(graphML.includes('<graphml'), 'Should have graphml root element');
    assert.ok(graphML.includes('<node'), 'Should have node elements');
    assert.ok(graphML.includes('<edge'), 'Should have edge elements');
    assert.ok(graphML.includes('</graphml>'), 'Should close graphml element');
  });

  it('should include node labels', () => {
    const graphML = exportToGraphML(sampleGraph);
    assert.ok(graphML.includes('Node 1'), 'Should include node labels');
    assert.ok(graphML.includes('Node 2'), 'Should include node labels');
  });

  it('should include edge weights', () => {
    const graphML = exportToGraphML(sampleGraph);
    assert.ok(graphML.includes('0.95'), 'Should include edge weight');
  });

  it('should include node attributes', () => {
    const graphML = exportToGraphML(sampleGraph, { includeMetadata: true });
    assert.ok(graphML.includes('type'), 'Should include attribute keys');
    assert.ok(graphML.includes('test'), 'Should include attribute values');
  });

  it('should escape XML special characters', () => {
    const graph: Graph = {
      nodes: [
        { id: 'n1', label: 'Test <>&"\'' },
        { id: 'n2', label: 'Normal' }
      ],
      edges: [
        { source: 'n1', target: 'n2', weight: 1.0 }
      ]
    };

    const graphML = exportToGraphML(graph);
    assert.ok(graphML.includes('&lt;'), 'Should escape < character');
    assert.ok(graphML.includes('&gt;'), 'Should escape > character');
    assert.ok(graphML.includes('&amp;'), 'Should escape & character');
  });
});

describe('GEXF Export', () => {
  it('should export valid GEXF XML', () => {
    const gexf = exportToGEXF(sampleGraph);

    assert.ok(gexf.includes('<?xml'), 'Should have XML declaration');
    assert.ok(gexf.includes('<gexf'), 'Should have gexf root element');
    assert.ok(gexf.includes('<nodes>'), 'Should have nodes section');
    assert.ok(gexf.includes('<edges>'), 'Should have edges section');
    assert.ok(gexf.includes('</gexf>'), 'Should close gexf element');
  });

  it('should include metadata', () => {
    const gexf = exportToGEXF(sampleGraph, {
      graphName: 'Test Graph',
      graphDescription: 'A test graph'
    });

    assert.ok(gexf.includes('<meta'), 'Should have meta section');
    assert.ok(gexf.includes('A test graph'), 'Should include description');
  });

  it('should define attributes', () => {
    const gexf = exportToGEXF(sampleGraph);
    assert.ok(gexf.includes('<attributes'), 'Should define attributes');
    assert.ok(gexf.includes('weight'), 'Should define weight attribute');
  });
});

describe('Neo4j Export', () => {
  it('should export valid Cypher queries', () => {
    const cypher = exportToNeo4j(sampleGraph);

    assert.ok(cypher.includes('CREATE (:Vector'), 'Should have CREATE statements');
    assert.ok(cypher.includes('MATCH'), 'Should have MATCH statements for edges');
    assert.ok(cypher.includes('CREATE CONSTRAINT'), 'Should create constraints');
  });

  it('should create nodes with properties', () => {
    const cypher = exportToNeo4j(sampleGraph, { includeMetadata: true });

    assert.ok(cypher.includes('id: "n1"'), 'Should include node ID');
    assert.ok(cypher.includes('label: "Node 1"'), 'Should include node label');
    assert.ok(cypher.includes('type: "test"'), 'Should include node attributes');
  });

  it('should create relationships with weights', () => {
    const cypher = exportToNeo4j(sampleGraph);

    assert.ok(cypher.includes('weight: 0.95'), 'Should include edge weight');
    assert.ok(cypher.includes('[:'), 'Should create relationships');
  });

  it('should escape special characters in Cypher', () => {
    const graph: Graph = {
      nodes: [
        { id: 'n1', label: 'Test "quoted"' },
        { id: 'n2', label: 'Normal' }
      ],
      edges: [
        { source: 'n1', target: 'n2', weight: 1.0 }
      ]
    };

    const cypher = exportToNeo4j(graph);
    assert.ok(cypher.includes('\\"'), 'Should escape quotes');
  });
});

describe('D3.js Export', () => {
  it('should export valid D3 JSON format', () => {
    const d3Data = exportToD3(sampleGraph);

    assert.ok(d3Data.nodes, 'Should have nodes array');
    assert.ok(d3Data.links, 'Should have links array');
    assert.ok(Array.isArray(d3Data.nodes), 'Nodes should be an array');
    assert.ok(Array.isArray(d3Data.links), 'Links should be an array');
  });

  it('should include node properties', () => {
    const d3Data = exportToD3(sampleGraph, { includeMetadata: true });

    const node = d3Data.nodes[0];
    assert.ok(node.id, 'Node should have ID');
    assert.ok(node.name, 'Node should have name');
    assert.strictEqual(node.type, 'test', 'Node should include attributes');
  });

  it('should include link properties', () => {
    const d3Data = exportToD3(sampleGraph);

    const link = d3Data.links[0];
    assert.ok(link.source, 'Link should have source');
    assert.ok(link.target, 'Link should have target');
    assert.strictEqual(link.value, 0.95, 'Link should have value (weight)');
  });
});

describe('NetworkX Export', () => {
  it('should export valid NetworkX JSON format', () => {
    const nxData = exportToNetworkX(sampleGraph);

    assert.strictEqual(nxData.directed, true, 'Should be directed graph');
    assert.ok(nxData.nodes, 'Should have nodes array');
    assert.ok(nxData.links, 'Should have links array');
    assert.ok(nxData.graph, 'Should have graph metadata');
  });

  it('should include node attributes', () => {
    const nxData = exportToNetworkX(sampleGraph, { includeMetadata: true });

    const node = nxData.nodes.find((n: any) => n.id === 'n1');
    assert.ok(node, 'Should find node');
    assert.strictEqual(node.label, 'Node 1', 'Should have label');
    assert.strictEqual(node.type, 'test', 'Should have attributes');
  });

  it('should include edge attributes', () => {
    const nxData = exportToNetworkX(sampleGraph);

    const link = nxData.links[0];
    assert.strictEqual(link.weight, 0.95, 'Should have weight');
    assert.strictEqual(link.type, 'similar', 'Should have type');
  });
});

describe('Unified Export Function', () => {
  it('should export to all formats', () => {
    const formats = ['graphml', 'gexf', 'neo4j', 'd3', 'networkx'] as const;

    for (const format of formats) {
      const result = exportGraph(sampleGraph, format);

      assert.strictEqual(result.format, format, `Should return correct format: ${format}`);
      assert.ok(result.data, 'Should have data');
      assert.strictEqual(result.nodeCount, 2, 'Should have correct node count');
      assert.strictEqual(result.edgeCount, 1, 'Should have correct edge count');
      assert.ok(result.metadata, 'Should have metadata');
    }
  });

  it('should throw error for unsupported format', () => {
    assert.throws(
      () => exportGraph(sampleGraph, 'invalid' as any),
      /Unsupported export format/,
      'Should throw error for invalid format'
    );
  });
});

describe('Graph Validation', () => {
  it('should validate correct graph', () => {
    assert.doesNotThrow(() => validateGraph(sampleGraph), 'Should not throw for valid graph');
  });

  it('should reject graph without nodes array', () => {
    const invalidGraph = { edges: [] } as any;
    assert.throws(
      () => validateGraph(invalidGraph),
      /must have a nodes array/,
      'Should reject graph without nodes'
    );
  });

  it('should reject graph without edges array', () => {
    const invalidGraph = { nodes: [] } as any;
    assert.throws(
      () => validateGraph(invalidGraph),
      /must have an edges array/,
      'Should reject graph without edges'
    );
  });

  it('should reject nodes without IDs', () => {
    const invalidGraph: Graph = {
      nodes: [{ id: '', label: 'Invalid' }],
      edges: []
    };
    assert.throws(
      () => validateGraph(invalidGraph),
      /must have an id/,
      'Should reject nodes without IDs'
    );
  });

  it('should reject edges with missing nodes', () => {
    const invalidGraph: Graph = {
      nodes: [{ id: 'n1' }],
      edges: [{ source: 'n1', target: 'n99', weight: 1.0 }]
    };
    assert.throws(
      () => validateGraph(invalidGraph),
      /non-existent.*node/,
      'Should reject edges referencing non-existent nodes'
    );
  });

  it('should reject edges without weight', () => {
    const invalidGraph: Graph = {
      nodes: [{ id: 'n1' }, { id: 'n2' }],
      edges: [{ source: 'n1', target: 'n2', weight: 'invalid' as any }]
    };
    assert.throws(
      () => validateGraph(invalidGraph),
      /numeric weight/,
      'Should reject edges without numeric weight'
    );
  });
});

describe('Edge Cases', () => {
  it('should handle empty graph', () => {
    const emptyGraph: Graph = { nodes: [], edges: [] };

    const graphML = exportToGraphML(emptyGraph);
    assert.ok(graphML.includes('<graphml'), 'Should export empty graph');

    const d3Data = exportToD3(emptyGraph);
    assert.strictEqual(d3Data.nodes.length, 0, 'Should have no nodes');
    assert.strictEqual(d3Data.links.length, 0, 'Should have no links');
  });

  it('should handle graph with nodes but no edges', () => {
    const graph: Graph = {
      nodes: [{ id: 'n1' }, { id: 'n2' }],
      edges: []
    };

    const result = exportGraph(graph, 'd3');
    assert.strictEqual(result.nodeCount, 2, 'Should have 2 nodes');
    assert.strictEqual(result.edgeCount, 0, 'Should have 0 edges');
  });

  it('should handle large attribute values', () => {
    const graph: Graph = {
      nodes: [
        {
          id: 'n1',
          label: 'Node with long text',
          attributes: {
            description: 'A'.repeat(1000),
            largeArray: Array(100).fill(1)
          }
        }
      ],
      edges: []
    };

    assert.doesNotThrow(
      () => exportToGraphML(graph, { includeMetadata: true }),
      'Should handle large attributes'
    );
  });

  it('should handle special characters in all formats', () => {
    const graph: Graph = {
      nodes: [
        { id: 'n1', label: 'Test <>&"\' special chars' },
        { id: 'n2', label: 'Normal' }
      ],
      edges: [{ source: 'n1', target: 'n2', weight: 1.0 }]
    };

    // Should not throw for any format
    assert.doesNotThrow(() => exportToGraphML(graph), 'GraphML should handle special chars');
    assert.doesNotThrow(() => exportToGEXF(graph), 'GEXF should handle special chars');
    assert.doesNotThrow(() => exportToNeo4j(graph), 'Neo4j should handle special chars');
    assert.doesNotThrow(() => exportToD3(graph), 'D3 should handle special chars');
    assert.doesNotThrow(() => exportToNetworkX(graph), 'NetworkX should handle special chars');
  });

  it('should handle circular references in graph', () => {
    const graph: Graph = {
      nodes: [
        { id: 'n1' },
        { id: 'n2' },
        { id: 'n3' }
      ],
      edges: [
        { source: 'n1', target: 'n2', weight: 1.0 },
        { source: 'n2', target: 'n3', weight: 1.0 },
        { source: 'n3', target: 'n1', weight: 1.0 }
      ]
    };

    assert.doesNotThrow(
      () => exportGraph(graph, 'd3'),
      'Should handle circular graph'
    );
  });
});

describe('Performance', () => {
  it('should handle moderately large graphs', () => {
    const nodes: GraphNode[] = [];
    const edges: GraphEdge[] = [];

    // Create 100 nodes
    for (let i = 0; i < 100; i++) {
      nodes.push({
        id: `node${i}`,
        label: `Node ${i}`,
        attributes: { index: i }
      });
    }

    // Create edges (each node connects to next 5)
    for (let i = 0; i < 95; i++) {
      for (let j = i + 1; j < Math.min(i + 6, 100); j++) {
        edges.push({
          source: `node${i}`,
          target: `node${j}`,
          weight: Math.random()
        });
      }
    }

    const graph: Graph = { nodes, edges };

    const startTime = Date.now();
    const result = exportGraph(graph, 'graphml');
    const duration = Date.now() - startTime;

    assert.ok(duration < 1000, `Export should complete in under 1s (took ${duration}ms)`);
    assert.strictEqual(result.nodeCount, 100, 'Should export all nodes');
    assert.ok(result.edgeCount > 0, 'Should export edges');
  });
});
