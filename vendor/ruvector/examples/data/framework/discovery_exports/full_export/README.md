# RuVector Discovery Export

Exported: 2026-01-03T19:19:17.360407287+00:00

## Files

- `graph.graphml` - Full graph in GraphML format (import into Gephi)
- `graph.dot` - Full graph in DOT format (render with Graphviz)
- `patterns.csv` - Discovered patterns
- `patterns_evidence.csv` - Patterns with detailed evidence
- `coherence.csv` - Coherence history over time

## Visualization

### Gephi (GraphML)
1. Open Gephi
2. File → Open → graph.graphml
3. Layout → Force Atlas 2 or Fruchterman Reingold
4. Color nodes by 'domain' attribute

### Graphviz (DOT)
```bash
# PNG output
dot -Tpng graph.dot -o graph.png

# SVG output (vector, scalable)
neato -Tsvg graph.dot -o graph.svg

# Interactive SVG
fdp -Tsvg graph.dot -o graph_interactive.svg
```

## Statistics

- Nodes: 60
- Edges: 1027
- Cross-domain edges: 655
- Patterns detected: 0
- Coherence snapshots: 0
