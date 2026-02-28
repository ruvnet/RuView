"use strict";
/**
 * Graph Export Examples
 *
 * Demonstrates how to use the graph export module with various formats
 * and configurations.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.example1_basicExport = example1_basicExport;
exports.example2_graphMLExport = example2_graphMLExport;
exports.example3_gephiExport = example3_gephiExport;
exports.example4_neo4jExport = example4_neo4jExport;
exports.example5_d3Export = example5_d3Export;
exports.example6_networkXExport = example6_networkXExport;
exports.example7_streamingExport = example7_streamingExport;
exports.example8_customGraph = example8_customGraph;
exports.runAllExamples = runAllExamples;
const exporters_js_1 = require("../src/exporters.js");
const fs_1 = require("fs");
const promises_1 = require("fs/promises");
// ============================================================================
// Example 1: Basic Graph Export to Multiple Formats
// ============================================================================
async function example1_basicExport() {
    console.log('\n=== Example 1: Basic Graph Export ===\n');
    // Sample vector entries (embeddings from a document collection)
    const entries = [
        {
            id: 'doc1',
            vector: [0.1, 0.2, 0.3, 0.4],
            metadata: { title: 'Introduction to AI', category: 'AI', year: 2023 }
        },
        {
            id: 'doc2',
            vector: [0.15, 0.25, 0.35, 0.42],
            metadata: { title: 'Machine Learning Basics', category: 'ML', year: 2023 }
        },
        {
            id: 'doc3',
            vector: [0.8, 0.1, 0.05, 0.05],
            metadata: { title: 'History of Rome', category: 'History', year: 2022 }
        },
        {
            id: 'doc4',
            vector: [0.12, 0.22, 0.32, 0.38],
            metadata: { title: 'Neural Networks', category: 'AI', year: 2024 }
        }
    ];
    // Build graph from vector entries
    const graph = (0, exporters_js_1.buildGraphFromEntries)(entries, {
        maxNeighbors: 2,
        threshold: 0.5,
        includeVectors: false,
        includeMetadata: true
    });
    console.log(`Graph built: ${graph.nodes.length} nodes, ${graph.edges.length} edges\n`);
    // Export to different formats
    const formats = ['graphml', 'gexf', 'neo4j', 'd3', 'networkx'];
    for (const format of formats) {
        const result = (0, exporters_js_1.exportGraph)(graph, format, {
            graphName: 'Document Similarity Network',
            graphDescription: 'Similarity network of document embeddings',
            includeMetadata: true
        });
        console.log(`${format.toUpperCase()}:`);
        console.log(`  Nodes: ${result.nodeCount}, Edges: ${result.edgeCount}`);
        if (typeof result.data === 'string') {
            console.log(`  Size: ${result.data.length} characters`);
            console.log(`  Preview: ${result.data.substring(0, 100)}...\n`);
        }
        else {
            console.log(`  Type: JSON object`);
            console.log(`  Preview: ${JSON.stringify(result.data).substring(0, 100)}...\n`);
        }
    }
}
// ============================================================================
// Example 2: Export to GraphML with Full Configuration
// ============================================================================
async function example2_graphMLExport() {
    console.log('\n=== Example 2: GraphML Export ===\n');
    const entries = [
        {
            id: 'vec1',
            vector: [1.0, 0.0, 0.0],
            metadata: { label: 'Vector 1', type: 'test', score: 0.95 }
        },
        {
            id: 'vec2',
            vector: [0.9, 0.1, 0.0],
            metadata: { label: 'Vector 2', type: 'test', score: 0.87 }
        },
        {
            id: 'vec3',
            vector: [0.0, 1.0, 0.0],
            metadata: { label: 'Vector 3', type: 'control', score: 0.92 }
        }
    ];
    const graph = (0, exporters_js_1.buildGraphFromEntries)(entries, {
        maxNeighbors: 2,
        threshold: 0.0,
        includeVectors: true, // Include vectors in export
        includeMetadata: true
    });
    const graphml = (0, exporters_js_1.exportToGraphML)(graph, {
        graphName: 'Test Vectors',
        includeVectors: true
    });
    console.log('GraphML Export:');
    console.log(graphml);
    // Save to file
    await (0, promises_1.writeFile)('examples/output/graph.graphml', graphml);
    console.log('\nSaved to: examples/output/graph.graphml');
}
// ============================================================================
// Example 3: Export to GEXF for Gephi Visualization
// ============================================================================
async function example3_gephiExport() {
    console.log('\n=== Example 3: GEXF Export for Gephi ===\n');
    // Simulate a larger network
    const entries = [];
    for (let i = 0; i < 20; i++) {
        entries.push({
            id: `node${i}`,
            vector: Array(128).fill(0).map(() => Math.random()),
            metadata: {
                label: `Node ${i}`,
                cluster: Math.floor(i / 5),
                importance: Math.random()
            }
        });
    }
    const graph = (0, exporters_js_1.buildGraphFromEntries)(entries, {
        maxNeighbors: 3,
        threshold: 0.7,
        includeMetadata: true
    });
    const gexf = (0, exporters_js_1.exportToGEXF)(graph, {
        graphName: 'Large Network',
        graphDescription: 'Network with 20 nodes and cluster information'
    });
    await (0, promises_1.writeFile)('examples/output/network.gexf', gexf);
    console.log('GEXF file created: examples/output/network.gexf');
    console.log('Import this file into Gephi for visualization!');
}
// ============================================================================
// Example 4: Export to Neo4j and Execute Queries
// ============================================================================
async function example4_neo4jExport() {
    console.log('\n=== Example 4: Neo4j Export ===\n');
    const entries = [
        {
            id: 'person1',
            vector: [0.5, 0.5],
            metadata: { name: 'Alice', role: 'Engineer', experience: 5 }
        },
        {
            id: 'person2',
            vector: [0.52, 0.48],
            metadata: { name: 'Bob', role: 'Engineer', experience: 3 }
        },
        {
            id: 'person3',
            vector: [0.1, 0.9],
            metadata: { name: 'Charlie', role: 'Manager', experience: 10 }
        }
    ];
    const graph = (0, exporters_js_1.buildGraphFromEntries)(entries, {
        maxNeighbors: 2,
        threshold: 0.5,
        includeMetadata: true
    });
    const cypher = (0, exporters_js_1.exportToNeo4j)(graph, {
        includeMetadata: true
    });
    console.log('Neo4j Cypher Queries:');
    console.log(cypher);
    await (0, promises_1.writeFile)('examples/output/import.cypher', cypher);
    console.log('\nSaved to: examples/output/import.cypher');
    console.log('\nTo import into Neo4j:');
    console.log('  1. Open Neo4j Browser');
    console.log('  2. Copy and paste the Cypher queries');
    console.log('  3. Execute to create the graph');
}
// ============================================================================
// Example 5: Export to D3.js for Web Visualization
// ============================================================================
async function example5_d3Export() {
    console.log('\n=== Example 5: D3.js Export ===\n');
    const entries = [
        {
            id: 'central',
            vector: [0.5, 0.5],
            metadata: { name: 'Central Node', size: 20, color: '#ff0000' }
        },
        {
            id: 'node1',
            vector: [0.6, 0.5],
            metadata: { name: 'Node 1', size: 10, color: '#00ff00' }
        },
        {
            id: 'node2',
            vector: [0.4, 0.5],
            metadata: { name: 'Node 2', size: 10, color: '#0000ff' }
        },
        {
            id: 'node3',
            vector: [0.5, 0.6],
            metadata: { name: 'Node 3', size: 10, color: '#ffff00' }
        }
    ];
    const graph = (0, exporters_js_1.buildGraphFromEntries)(entries, {
        maxNeighbors: 3,
        threshold: 0.0,
        includeMetadata: true
    });
    const d3Data = (0, exporters_js_1.exportToD3)(graph, {
        includeMetadata: true
    });
    console.log('D3.js Data:');
    console.log(JSON.stringify(d3Data, null, 2));
    await (0, promises_1.writeFile)('examples/output/d3-graph.json', JSON.stringify(d3Data, null, 2));
    console.log('\nSaved to: examples/output/d3-graph.json');
    // Generate simple HTML visualization
    const html = `
<!DOCTYPE html>
<html>
<head>
  <title>D3.js Force Graph</title>
  <script src="https://d3js.org/d3.v7.min.js"></script>
  <style>
    body { margin: 0; font-family: Arial, sans-serif; }
    svg { border: 1px solid #ccc; }
    .links line { stroke: #999; stroke-opacity: 0.6; }
    .nodes circle { stroke: #fff; stroke-width: 1.5px; }
    .labels { font-size: 10px; pointer-events: none; }
  </style>
</head>
<body>
  <svg width="800" height="600"></svg>
  <script>
    const graphData = ${JSON.stringify(d3Data)};

    const svg = d3.select("svg"),
      width = +svg.attr("width"),
      height = +svg.attr("height");

    const simulation = d3.forceSimulation(graphData.nodes)
      .force("link", d3.forceLink(graphData.links).id(d => d.id).distance(100))
      .force("charge", d3.forceManyBody().strength(-300))
      .force("center", d3.forceCenter(width / 2, height / 2));

    const link = svg.append("g")
      .attr("class", "links")
      .selectAll("line")
      .data(graphData.links)
      .enter().append("line")
      .attr("stroke-width", d => Math.sqrt(d.value) * 2);

    const node = svg.append("g")
      .attr("class", "nodes")
      .selectAll("circle")
      .data(graphData.nodes)
      .enter().append("circle")
      .attr("r", d => d.size || 5)
      .attr("fill", d => d.color || "#69b3a2")
      .call(d3.drag()
        .on("start", dragstarted)
        .on("drag", dragged)
        .on("end", dragended));

    const label = svg.append("g")
      .attr("class", "labels")
      .selectAll("text")
      .data(graphData.nodes)
      .enter().append("text")
      .text(d => d.name)
      .attr("dx", 12)
      .attr("dy", 4);

    simulation.on("tick", () => {
      link.attr("x1", d => d.source.x)
          .attr("y1", d => d.source.y)
          .attr("x2", d => d.target.x)
          .attr("y2", d => d.target.y);
      node.attr("cx", d => d.x)
          .attr("cy", d => d.y);
      label.attr("x", d => d.x)
           .attr("y", d => d.y);
    });

    function dragstarted(event, d) {
      if (!event.active) simulation.alphaTarget(0.3).restart();
      d.fx = d.x;
      d.fy = d.y;
    }

    function dragged(event, d) {
      d.fx = event.x;
      d.fy = event.y;
    }

    function dragended(event, d) {
      if (!event.active) simulation.alphaTarget(0);
      d.fx = null;
      d.fy = null;
    }
  </script>
</body>
</html>`;
    await (0, promises_1.writeFile)('examples/output/d3-visualization.html', html);
    console.log('Created HTML visualization: examples/output/d3-visualization.html');
    console.log('Open this file in a web browser to see the interactive graph!');
}
// ============================================================================
// Example 6: Export to NetworkX for Python Analysis
// ============================================================================
async function example6_networkXExport() {
    console.log('\n=== Example 6: NetworkX Export ===\n');
    const entries = [];
    for (let i = 0; i < 10; i++) {
        entries.push({
            id: `node_${i}`,
            vector: Array(64).fill(0).map(() => Math.random()),
            metadata: { degree: i, centrality: Math.random() }
        });
    }
    const graph = (0, exporters_js_1.buildGraphFromEntries)(entries, {
        maxNeighbors: 3,
        threshold: 0.6
    });
    const nxData = (0, exporters_js_1.exportToNetworkX)(graph, {
        includeMetadata: true
    });
    await (0, promises_1.writeFile)('examples/output/networkx-graph.json', JSON.stringify(nxData, null, 2));
    console.log('NetworkX JSON saved to: examples/output/networkx-graph.json');
    // Generate Python script
    const pythonScript = `
import json
import networkx as nx
import matplotlib.pyplot as plt

# Load the graph
with open('networkx-graph.json', 'r') as f:
    data = json.load(f)

G = nx.node_link_graph(data)

# Calculate centrality measures
degree_centrality = nx.degree_centrality(G)
betweenness_centrality = nx.betweenness_centrality(G)

print(f"Graph has {G.number_of_nodes()} nodes and {G.number_of_edges()} edges")
print(f"\\nTop 5 nodes by degree centrality:")
sorted_nodes = sorted(degree_centrality.items(), key=lambda x: x[1], reverse=True)[:5]
for node, centrality in sorted_nodes:
    print(f"  {node}: {centrality:.4f}")

# Visualize
plt.figure(figsize=(12, 8))
pos = nx.spring_layout(G, k=0.5, iterations=50)
nx.draw(G, pos,
        node_color=[degree_centrality[node] for node in G.nodes()],
        node_size=[v * 1000 for v in degree_centrality.values()],
        cmap=plt.cm.plasma,
        with_labels=True,
        font_size=8,
        font_weight='bold',
        edge_color='gray',
        alpha=0.7)
plt.title('Network Graph Visualization')
plt.colorbar(plt.cm.ScalarMappable(cmap=plt.cm.plasma), label='Degree Centrality')
plt.savefig('network-visualization.png', dpi=300, bbox_inches='tight')
print("\\nVisualization saved to: network-visualization.png")
`;
    await (0, promises_1.writeFile)('examples/output/analyze_network.py', pythonScript);
    console.log('Python analysis script saved to: examples/output/analyze_network.py');
    console.log('\nTo analyze in Python:');
    console.log('  cd examples/output');
    console.log('  pip install networkx matplotlib');
    console.log('  python analyze_network.py');
}
// ============================================================================
// Example 7: Streaming Export for Large Graphs
// ============================================================================
async function example7_streamingExport() {
    console.log('\n=== Example 7: Streaming Export ===\n');
    // Simulate a large graph that doesn't fit in memory
    console.log('Creating streaming GraphML export...');
    const stream = (0, fs_1.createWriteStream)('examples/output/large-graph.graphml');
    const exporter = new exporters_js_1.GraphMLStreamExporter(stream, {
        graphName: 'Large Streaming Graph'
    });
    await exporter.start();
    // Add nodes in batches
    for (let i = 0; i < 1000; i++) {
        const node = {
            id: `node${i}`,
            label: `Node ${i}`,
            attributes: {
                batch: Math.floor(i / 100),
                value: Math.random()
            }
        };
        await exporter.addNode(node);
        if (i % 100 === 0) {
            console.log(`  Added ${i} nodes...`);
        }
    }
    console.log('  Added 1000 nodes');
    // Add edges
    let edgeCount = 0;
    for (let i = 0; i < 1000; i++) {
        for (let j = i + 1; j < Math.min(i + 5, 1000); j++) {
            const edge = {
                source: `node${i}`,
                target: `node${j}`,
                weight: Math.random()
            };
            await exporter.addEdge(edge);
            edgeCount++;
        }
    }
    console.log(`  Added ${edgeCount} edges`);
    await exporter.end();
    stream.close();
    console.log('\nStreaming export completed: examples/output/large-graph.graphml');
    console.log('This approach works for graphs with millions of nodes!');
}
// ============================================================================
// Example 8: Custom Graph Construction
// ============================================================================
async function example8_customGraph() {
    console.log('\n=== Example 8: Custom Graph Construction ===\n');
    // Build a custom graph structure manually
    const graph = {
        nodes: [
            { id: 'A', label: 'Root', attributes: { level: 0, type: 'root' } },
            { id: 'B', label: 'Child 1', attributes: { level: 1, type: 'child' } },
            { id: 'C', label: 'Child 2', attributes: { level: 1, type: 'child' } },
            { id: 'D', label: 'Leaf 1', attributes: { level: 2, type: 'leaf' } },
            { id: 'E', label: 'Leaf 2', attributes: { level: 2, type: 'leaf' } }
        ],
        edges: [
            { source: 'A', target: 'B', weight: 1.0, type: 'parent-child' },
            { source: 'A', target: 'C', weight: 1.0, type: 'parent-child' },
            { source: 'B', target: 'D', weight: 0.8, type: 'parent-child' },
            { source: 'C', target: 'E', weight: 0.9, type: 'parent-child' },
            { source: 'B', target: 'C', weight: 0.5, type: 'sibling' }
        ],
        metadata: {
            description: 'Hierarchical tree structure',
            created: new Date().toISOString()
        }
    };
    // Export to multiple formats
    const graphML = (0, exporters_js_1.exportToGraphML)(graph);
    const d3Data = (0, exporters_js_1.exportToD3)(graph);
    const neo4j = (0, exporters_js_1.exportToNeo4j)(graph);
    await (0, promises_1.writeFile)('examples/output/custom-graph.graphml', graphML);
    await (0, promises_1.writeFile)('examples/output/custom-graph-d3.json', JSON.stringify(d3Data, null, 2));
    await (0, promises_1.writeFile)('examples/output/custom-graph.cypher', neo4j);
    console.log('Custom graph exported to:');
    console.log('  - examples/output/custom-graph.graphml');
    console.log('  - examples/output/custom-graph-d3.json');
    console.log('  - examples/output/custom-graph.cypher');
}
// ============================================================================
// Run All Examples
// ============================================================================
async function runAllExamples() {
    console.log('╔═══════════════════════════════════════════════════════╗');
    console.log('║     ruvector Graph Export Examples                   ║');
    console.log('╚═══════════════════════════════════════════════════════╝');
    // Create output directory
    const fs = await Promise.resolve().then(() => __importStar(require('fs/promises')));
    try {
        await fs.mkdir('examples/output', { recursive: true });
    }
    catch (e) {
        // Directory already exists
    }
    try {
        await example1_basicExport();
        await example2_graphMLExport();
        await example3_gephiExport();
        await example4_neo4jExport();
        await example5_d3Export();
        await example6_networkXExport();
        await example7_streamingExport();
        await example8_customGraph();
        console.log('\n✅ All examples completed successfully!');
        console.log('\nGenerated files in examples/output/:');
        console.log('  - graph.graphml (GraphML format)');
        console.log('  - network.gexf (Gephi format)');
        console.log('  - import.cypher (Neo4j queries)');
        console.log('  - d3-graph.json (D3.js data)');
        console.log('  - d3-visualization.html (Interactive visualization)');
        console.log('  - networkx-graph.json (NetworkX format)');
        console.log('  - analyze_network.py (Python analysis script)');
        console.log('  - large-graph.graphml (Streaming export demo)');
        console.log('  - custom-graph.* (Custom graph exports)');
    }
    catch (error) {
        console.error('\n❌ Error running examples:', error);
        throw error;
    }
}
// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
    runAllExamples().catch(console.error);
}
//# sourceMappingURL=graph-export-examples.js.map