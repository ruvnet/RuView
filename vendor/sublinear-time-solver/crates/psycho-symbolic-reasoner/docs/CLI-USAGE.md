# Psycho-Symbolic Reasoner CLI Usage Guide

## Installation

```bash
# Install globally
npm install -g psycho-symbolic-reasoner

# Or use with npx (recommended)
npx psycho-symbolic-reasoner --help
```

## Basic Usage

### Start the MCP Server

```bash
# Start with default STDIO transport
npx psycho-symbolic-reasoner start

# Start with HTTP transport
npx psycho-symbolic-reasoner start --transport http --port 3000

# Start with configuration file
npx psycho-symbolic-reasoner start --config ./my-config.json
```

### Command-Line Options

#### Global Options

- `--help, -h`: Show help information
- `--version, -V`: Show version information

#### Start Command Options

- `--knowledge-base <file>, -k`: Load initial graph data from file
- `--transport <type>, -t`: Transport type (stdio, sse, http) [default: stdio]
- `--port <number>, -p`: Port number for HTTP/SSE transport [default: 3000]
- `--host <host>, -H`: Host address for HTTP/SSE transport [default: localhost]
- `--config <file>, -c`: Configuration file path
- `--log-level <level>, -l`: Log level (error, warn, info, debug, silly) [default: info]
- `--log-file <file>, -f`: Log file path
- `--verbose, -v`: Enable verbose logging (debug level)
- `--quiet, -q`: Disable console logging

## Transport Types

### STDIO Transport (Default)

Best for MCP client integration:

```bash
npx psycho-symbolic-reasoner start --transport stdio
```

### HTTP Transport

For web applications and REST API access:

```bash
npx psycho-symbolic-reasoner start --transport http --port 3000 --host 0.0.0.0
```

Access endpoints:
- Health check: `GET http://localhost:3000/health`
- MCP endpoint: `POST http://localhost:3000/mcp`

### SSE Transport

For real-time web applications:

```bash
npx psycho-symbolic-reasoner start --transport sse --port 3000
```

## Configuration Management

### Generate Sample Configuration

```bash
npx psycho-symbolic-reasoner config --generate > my-config.json
```

### Validate Configuration

```bash
npx psycho-symbolic-reasoner config --validate ./my-config.json
```

### Configuration File Locations

The CLI automatically searches for configuration files in this order:

1. `--config` specified file
2. `psycho-symbolic-reasoner.config.json`
3. `psycho-symbolic-reasoner.config.yaml`
4. `psycho-symbolic-reasoner.config.yml`
5. `.psycho-symbolic-reasoner.json`
6. `.psycho-symbolic-reasoner.yaml`
7. `.psycho-symbolic-reasoner.yml`

### Sample Configuration File

```json
{
  "server": {
    "transport": "http",
    "port": 3000,
    "host": "localhost",
    "cors": true,
    "maxConnections": 100,
    "timeout": 30000
  },
  "knowledgeBase": {
    "file": "./knowledge.json",
    "autoSave": true,
    "saveInterval": 30000,
    "format": "json",
    "compression": false
  },
  "logging": {
    "level": "info",
    "file": "./logs/app.log",
    "console": true,
    "json": false,
    "timestamp": true,
    "maxSize": "10m",
    "maxFiles": 5
  },
  "performance": {
    "maxMemoryUsage": "512m",
    "gcInterval": 60000,
    "enableProfiling": false,
    "metricsInterval": 10000
  },
  "security": {
    "enableAuth": false,
    "rateLimit": {
      "enabled": true,
      "windowMs": 60000,
      "maxRequests": 100
    },
    "allowedOrigins": ["*"]
  }
}
```

## Health Monitoring

### Check Server Health

```bash
npx psycho-symbolic-reasoner health --url http://localhost:3000
```

### Detailed Health Check

```bash
npx psycho-symbolic-reasoner health --url http://localhost:3000 --detailed
```

## Examples

### Development Setup

```bash
# Start with debug logging and file output
npx psycho-symbolic-reasoner start \\
  --transport http \\
  --port 3000 \\
  --log-level debug \\
  --log-file ./logs/debug.log \\
  --knowledge-base ./data/initial-graph.json
```

### Production Setup

```bash
# Start with production configuration
npx psycho-symbolic-reasoner start \\
  --config ./production.config.json \\
  --quiet
```

### Load Knowledge Base

```bash
# Start with pre-loaded knowledge
npx psycho-symbolic-reasoner start \\
  --knowledge-base ./knowledge/domain-knowledge.json \\
  --transport http \\
  --port 8080
```

### MCP Client Integration

```bash
# For Claude Desktop or other MCP clients
npx psycho-symbolic-reasoner start --transport stdio
```

## Environment Variables

You can also configure the application using environment variables:

- `PSR_TRANSPORT`: Transport type
- `PSR_PORT`: Server port
- `PSR_HOST`: Server host
- `PSR_LOG_LEVEL`: Logging level
- `PSR_CONFIG_FILE`: Configuration file path

Example:

```bash
export PSR_TRANSPORT=http
export PSR_PORT=3000
export PSR_LOG_LEVEL=debug
npx psycho-symbolic-reasoner start
```

## Graceful Shutdown

The server handles graceful shutdown on:

- `SIGINT` (Ctrl+C)
- `SIGTERM`
- `SIGQUIT`

All active connections are closed properly and logs are flushed before exit.

## Troubleshooting

### Common Issues

1. **Port already in use**
   ```bash
   # Use a different port
   npx psycho-symbolic-reasoner start --port 3001
   ```

2. **Permission denied**
   ```bash
   # Use a port > 1024 or run with sudo
   npx psycho-symbolic-reasoner start --port 8080
   ```

3. **Configuration errors**
   ```bash
   # Validate your configuration
   npx psycho-symbolic-reasoner config --validate ./my-config.json
   ```

4. **Memory issues**
   ```bash
   # Increase memory limit
   npx psycho-symbolic-reasoner start --config config-with-higher-memory.json
   ```

### Debug Mode

Enable debug logging to troubleshoot issues:

```bash
npx psycho-symbolic-reasoner start --verbose --log-file debug.log
```

### Health Checks

Monitor server health:

```bash
# Basic health check
curl http://localhost:3000/health

# From CLI
npx psycho-symbolic-reasoner health --detailed
```

## Integration with MCP Clients

### Claude Desktop

Add to your MCP configuration:

```json
{
  "mcpServers": {
    "psycho-symbolic-reasoner": {
      "command": "npx",
      "args": ["psycho-symbolic-reasoner", "start"],
      "env": {}
    }
  }
}
```

### Custom MCP Client

```typescript
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const transport = new StdioClientTransport({
  command: 'npx',
  args: ['psycho-symbolic-reasoner', 'start']
});

const client = new Client({
  name: 'my-client',
  version: '1.0.0'
}, {
  capabilities: {}
});

await client.connect(transport);
```

## Performance Tuning

### Memory Optimization

```json
{
  "performance": {
    "maxMemoryUsage": "1g",
    "gcInterval": 30000,
    "enableProfiling": true
  }
}
```

### Connection Limits

```json
{
  "server": {
    "maxConnections": 1000,
    "timeout": 60000
  }
}
```

### Rate Limiting

```json
{
  "security": {
    "rateLimit": {
      "enabled": true,
      "windowMs": 60000,
      "maxRequests": 1000
    }
  }
}
```