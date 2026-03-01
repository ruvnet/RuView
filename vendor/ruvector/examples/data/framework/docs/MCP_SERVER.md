# RuVector MCP (Model Context Protocol) Server

Comprehensive MCP server implementation for the RuVector data discovery framework, following the Anthropic MCP specification (2024-11-05).

## Overview

The RuVector MCP server exposes 22+ data sources across research, medical, economic, climate, and knowledge domains through a standardized JSON-RPC 2.0 interface. It supports both STDIO and SSE (Server-Sent Events) transports for integration with AI assistants and automation tools.

## Features

### Transport Layers
- **STDIO**: Standard input/output transport for CLI integration
- **SSE**: HTTP-based Server-Sent Events for web applications (requires `sse` feature)

### Data Sources (22 tools)

#### Research Tools
1. `search_openalex` - Search OpenAlex for research papers
2. `search_arxiv` - Search arXiv preprints
3. `search_semantic_scholar` - Search Semantic Scholar database
4. `get_citations` - Get paper citations
5. `search_crossref` - Search CrossRef DOI database
6. `search_biorxiv` - Search bioRxiv preprints
7. `search_medrxiv` - Search medRxiv medical preprints

#### Medical Tools
8. `search_pubmed` - Search PubMed literature
9. `search_clinical_trials` - Search ClinicalTrials.gov
10. `search_fda_events` - Search FDA adverse event reports

#### Economic Tools
11. `get_fred_series` - Get Federal Reserve Economic Data
12. `get_worldbank_indicator` - Get World Bank indicators

#### Climate Tools
13. `get_noaa_data` - Get NOAA climate data

#### Knowledge Tools
14. `search_wikipedia` - Search Wikipedia articles
15. `query_wikidata` - Query Wikidata SPARQL endpoint

#### Discovery Tools
16. `run_discovery` - Multi-source pattern discovery
17. `analyze_coherence` - Vector coherence analysis
18. `detect_patterns` - Pattern detection in signals
19. `export_graph` - Export graphs (GraphML, DOT, CSV)

### Resources

Access discovered data and analysis results:

- `discovery://patterns` - Current discovered patterns
- `discovery://graph` - Coherence graph structure
- `discovery://history` - Historical coherence data

### Pre-built Prompts

Ready-to-use discovery workflows:

1. **cross_domain_discovery** - Multi-source pattern finding
2. **citation_analysis** - Build and analyze citation networks
3. **trend_detection** - Temporal pattern analysis

## Installation

```bash
cd /home/user/ruvector/examples/data/framework
cargo build --bin mcp_discovery --release
```

For SSE support:
```bash
cargo build --bin mcp_discovery --release --features sse
```

## Usage

### STDIO Mode (Default)

```bash
# Run the server
cargo run --bin mcp_discovery

# Or with compiled binary
./target/release/mcp_discovery
```

### SSE Mode (HTTP Streaming)

```bash
# Run on port 3000
cargo run --bin mcp_discovery -- --sse --port 3000

# Custom endpoint
cargo run --bin mcp_discovery -- --sse --endpoint 0.0.0.0 --port 8080
```

### Configuration Options

```bash
mcp_discovery [OPTIONS]

OPTIONS:
    --sse                       Use SSE transport instead of STDIO
    --port <PORT>              Port for SSE endpoint (default: 3000)
    --endpoint <ENDPOINT>      Endpoint address (default: 127.0.0.1)
    -c, --config <FILE>        Configuration file path
    --min-edge-weight <F64>    Minimum edge weight (default: 0.5)
    --similarity-threshold <F64> Similarity threshold (default: 0.7)
    --cross-domain            Enable cross-domain discovery (default: true)
    --window-seconds <I64>     Temporal window size (default: 3600)
    --hnsw-m <USIZE>          HNSW M parameter (default: 16)
    --hnsw-ef-construction <USIZE> HNSW ef_construction (default: 200)
    --dimension <USIZE>       Vector dimension (default: 384)
    -v, --verbose             Enable verbose logging
```

### Configuration File Example

```json
{
  "min_edge_weight": 0.5,
  "similarity_threshold": 0.7,
  "mincut_sensitivity": 0.1,
  "cross_domain": true,
  "window_seconds": 3600,
  "hnsw_m": 16,
  "hnsw_ef_construction": 200,
  "hnsw_ef_search": 50,
  "dimension": 384,
  "batch_size": 1000,
  "checkpoint_interval": 10000,
  "parallel_workers": 4
}
```

## MCP Protocol

### Initialize

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {}
  }
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2024-11-05",
    "serverInfo": {
      "name": "ruvector-discovery-mcp",
      "version": "1.0.0"
    },
    "capabilities": {
      "tools": { "list_changed": false },
      "resources": { "list_changed": false, "subscribe": false },
      "prompts": { "list_changed": false }
    }
  }
}
```

### List Tools

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list"
}
```

### Call Tool

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "search_openalex",
    "arguments": {
      "query": "machine learning",
      "limit": 10
    }
  }
}
```

### Read Resource

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "resources/read",
  "params": {
    "uri": "discovery://patterns"
  }
}
```

### Get Prompt

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "prompts/get",
  "params": {
    "name": "cross_domain_discovery",
    "arguments": {
      "domains": "research,medical,climate",
      "query": "COVID-19 impact"
    }
  }
}
```

## Tool Reference

### search_openalex

Search OpenAlex for scholarly works.

**Parameters:**
- `query` (string, required): Search query
- `limit` (integer, optional): Maximum results (default: 10)

**Example:**
```json
{
  "query": "vector databases",
  "limit": 5
}
```

### search_arxiv

Search arXiv preprint repository.

**Parameters:**
- `query` (string, required): Search query
- `category` (string, optional): arXiv category (e.g., "cs.AI", "physics.gen-ph")
- `limit` (integer, optional): Maximum results (default: 10)

### get_citations

Get citations for a paper.

**Parameters:**
- `paper_id` (string, required): Paper ID or DOI

### run_discovery

Run multi-source discovery.

**Parameters:**
- `sources` (array, required): Data sources to query
- `query` (string, required): Discovery query

**Example:**
```json
{
  "sources": ["openalex", "semantic_scholar", "pubmed"],
  "query": "CRISPR gene editing"
}
```

### export_graph

Export coherence graph.

**Parameters:**
- `format` (string, required): Format ("graphml", "dot", or "csv")

## Rate Limiting

Default rate limit: 100 requests per minute per tool.

## Error Codes

Standard JSON-RPC 2.0 error codes:

- `-32700` Parse error
- `-32600` Invalid request
- `-32601` Method not found
- `-32602` Invalid params
- `-32603` Internal error

## Architecture

```
┌─────────────────────────────────────────┐
│         MCP Discovery Server            │
├─────────────────────────────────────────┤
│  JSON-RPC 2.0 Message Handler           │
├─────────────────┬───────────────────────┤
│  STDIO Transport │ SSE Transport (HTTP)  │
├─────────────────┴───────────────────────┤
│      Data Source Clients (22+)          │
│  ┌────────────┬──────────┬──────────┐   │
│  │  Research  │ Medical  │ Economic │   │
│  │  OpenAlex  │ PubMed   │   FRED   │   │
│  │  ArXiv     │ Clinical │ WorldBank│   │
│  │  Scholar   │   FDA    │          │   │
│  └────────────┴──────────┴──────────┘   │
├─────────────────────────────────────────┤
│    Native Discovery Engine              │
│  ┌────────────────────────────────────┐ │
│  │  Vector Storage (HNSW)             │ │
│  │  Graph Coherence (Min-Cut)         │ │
│  │  Pattern Detection                 │ │
│  └────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

## Integration Examples

### Claude Desktop App

Add to Claude Desktop config:

```json
{
  "mcpServers": {
    "ruvector-discovery": {
      "command": "/path/to/mcp_discovery",
      "args": []
    }
  }
}
```

### Python Client

```python
import json
import subprocess

# Start MCP server
proc = subprocess.Popen(
    ['./mcp_discovery'],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True
)

# Send initialize
request = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {}
}
proc.stdin.write(json.dumps(request) + '\n')
proc.stdin.flush()

# Read response
response = json.loads(proc.stdout.readline())
print(response)

# Call tool
request = {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
        "name": "search_openalex",
        "arguments": {"query": "vector search", "limit": 5}
    }
}
proc.stdin.write(json.dumps(request) + '\n')
proc.stdin.flush()

# Read results
response = json.loads(proc.stdout.readline())
print(response)
```

## Development

### Project Structure

```
framework/
├── src/
│   ├── mcp_server.rs        # MCP server implementation
│   ├── bin/
│   │   └── mcp_discovery.rs # Binary entry point
│   ├── api_clients.rs       # OpenAlex, NOAA clients
│   ├── arxiv_client.rs      # ArXiv client
│   ├── semantic_scholar.rs  # Semantic Scholar client
│   ├── medical_clients.rs   # PubMed, ClinicalTrials, FDA
│   ├── economic_clients.rs  # FRED, WorldBank
│   ├── wiki_clients.rs      # Wikipedia, Wikidata
│   └── ruvector_native.rs   # Discovery engine
└── docs/
    └── MCP_SERVER.md        # This file
```

### Adding New Tools

1. Add client to `DataSourceClients`
2. Create tool definition in `tool_*` methods
3. Implement execution in `execute_*` methods
4. Update `handle_tool_call` dispatcher

### Testing

```bash
# Unit tests
cargo test --lib

# Integration test
echo '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | cargo run --bin mcp_discovery
```

## Known Limitations

- Client constructors require Result handling (some need API keys)
- SSE transport requires `sse` feature flag
- Rate limiting is per-session, not persistent
- No authentication/authorization (local use only)

## Troubleshooting

### "SSE transport requires the 'sse' feature"

Rebuild with SSE support:
```bash
cargo build --bin mcp_discovery --features sse
```

### Client initialization errors

Some clients require API keys via environment variables:
- `FRED_API_KEY` - Federal Reserve Economic Data
- `NOAA_API_TOKEN` - NOAA Climate Data
- `SEMANTIC_SCHOLAR_API_KEY` - Semantic Scholar (optional)

Set these before running:
```bash
export FRED_API_KEY="your_key"
export NOAA_API_TOKEN="your_token"
./mcp_discovery
```

## License

Part of the RuVector project. See main repository for license information.

## Contributing

See main RuVector repository for contribution guidelines.

## References

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [JSON-RPC 2.0](https://www.jsonrpc.org/specification)
- [RuVector Documentation](https://github.com/ruvnet/ruvector)
