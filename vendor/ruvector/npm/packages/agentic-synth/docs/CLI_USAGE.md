# Agentic Synth CLI Usage Guide

## Overview

The `agentic-synth` CLI provides a command-line interface for AI-powered synthetic data generation. It supports multiple model providers, custom schemas, and various output formats.

## Installation

```bash
npm install agentic-synth
# or
npm install -g agentic-synth
```

## Configuration

### Environment Variables

Set your API key before using the CLI:

```bash
# For Google Gemini (default)
export GEMINI_API_KEY="your-api-key-here"

# For OpenRouter
export OPENROUTER_API_KEY="your-api-key-here"
```

### Configuration File

Create a `config.json` file for persistent settings:

```json
{
  "provider": "gemini",
  "model": "gemini-2.0-flash-exp",
  "apiKey": "your-api-key",
  "cacheStrategy": "memory",
  "cacheTTL": 3600,
  "maxRetries": 3,
  "timeout": 30000
}
```

## Commands

### Generate Data

Generate synthetic structured data based on a schema.

```bash
agentic-synth generate [options]
```

#### Options

- `-c, --count <number>` - Number of records to generate (default: 10)
- `-s, --schema <path>` - Path to JSON schema file
- `-o, --output <path>` - Output file path (JSON format)
- `--seed <value>` - Random seed for reproducibility
- `-p, --provider <provider>` - Model provider: `gemini` or `openrouter` (default: gemini)
- `-m, --model <model>` - Specific model name to use
- `--format <format>` - Output format: `json`, `csv`, or `array` (default: json)
- `--config <path>` - Path to config file with provider settings

#### Examples

**Basic generation (10 records):**
```bash
agentic-synth generate
```

**Generate with custom count:**
```bash
agentic-synth generate --count 100
```

**Generate with schema:**
```bash
agentic-synth generate --schema examples/user-schema.json --count 50
```

**Generate to file:**
```bash
agentic-synth generate --schema examples/user-schema.json --output data/users.json --count 100
```

**Generate with seed for reproducibility:**
```bash
agentic-synth generate --schema examples/user-schema.json --seed 12345 --count 20
```

**Use OpenRouter provider:**
```bash
agentic-synth generate --provider openrouter --model anthropic/claude-3.5-sonnet --count 30
```

**Use config file:**
```bash
agentic-synth generate --config config.json --schema examples/user-schema.json --count 50
```

#### Sample Schema

Create a JSON schema file (e.g., `user-schema.json`):

```json
{
  "type": "object",
  "properties": {
    "id": {
      "type": "string",
      "description": "Unique user identifier (UUID)"
    },
    "name": {
      "type": "string",
      "description": "Full name of the user"
    },
    "email": {
      "type": "string",
      "format": "email",
      "description": "Valid email address"
    },
    "age": {
      "type": "number",
      "minimum": 18,
      "maximum": 100,
      "description": "User age between 18 and 100"
    },
    "role": {
      "type": "string",
      "enum": ["admin", "user", "moderator"],
      "description": "User role in the system"
    }
  },
  "required": ["id", "name", "email"]
}
```

### Display Configuration

View current configuration settings.

```bash
agentic-synth config [options]
```

#### Options

- `-f, --file <path>` - Load and display config from file
- `-t, --test` - Test configuration by initializing AgenticSynth

#### Examples

**Show default configuration:**
```bash
agentic-synth config
```

**Load and display config file:**
```bash
agentic-synth config --file config.json
```

**Test configuration:**
```bash
agentic-synth config --test
```

### Validate Configuration

Validate configuration and dependencies.

```bash
agentic-synth validate [options]
```

#### Options

- `-f, --file <path>` - Config file path to validate

#### Examples

**Validate default configuration:**
```bash
agentic-synth validate
```

**Validate config file:**
```bash
agentic-synth validate --file config.json
```

## Output Format

### JSON Output (default)

```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "John Doe",
    "email": "john.doe@example.com",
    "age": 32,
    "role": "user"
  },
  {
    "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
    "name": "Jane Smith",
    "email": "jane.smith@example.com",
    "age": 28,
    "role": "admin"
  }
]
```

### Metadata

The CLI displays metadata after generation:

```
Metadata:
  Provider: gemini
  Model: gemini-2.0-flash-exp
  Cached: false
  Duration: 1247ms
  Generated: 2025-11-22T10:30:45.123Z
```

## Error Handling

The CLI provides clear error messages:

```bash
# Missing schema file
agentic-synth generate --schema missing.json
# Error: Schema file not found: missing.json

# Invalid count
agentic-synth generate --count -5
# Error: Count must be a positive integer

# Missing API key
agentic-synth generate
# Error: API key not found. Set GEMINI_API_KEY or OPENROUTER_API_KEY environment variable
```

## Debug Mode

Enable debug mode for detailed error information:

```bash
DEBUG=1 agentic-synth generate --schema examples/user-schema.json
```

## Common Workflows

### 1. Quick Test Generation

```bash
agentic-synth generate --count 5
```

### 2. Production Data Generation

```bash
agentic-synth generate \
  --schema schemas/product-schema.json \
  --output data/products.json \
  --count 1000 \
  --seed 42 \
  --provider gemini
```

### 3. Multiple Datasets

```bash
# Users
agentic-synth generate --schema schemas/user.json --output data/users.json --count 100

# Products
agentic-synth generate --schema schemas/product.json --output data/products.json --count 500

# Orders
agentic-synth generate --schema schemas/order.json --output data/orders.json --count 200
```

### 4. Reproducible Generation

```bash
# Generate with same seed for consistent results
agentic-synth generate --schema examples/user-schema.json --seed 12345 --count 50 --output data/users-v1.json
agentic-synth generate --schema examples/user-schema.json --seed 12345 --count 50 --output data/users-v2.json

# Both files will contain identical data
```

## Tips & Best Practices

1. **Use schemas** - Provide detailed JSON schemas for better quality data
2. **Set seeds** - Use `--seed` for reproducible results in testing
3. **Start small** - Test with small counts before generating large datasets
4. **Cache strategy** - Configure caching to improve performance for repeated generations
5. **Provider selection** - Choose the appropriate provider based on your needs:
   - Gemini: Fast, cost-effective, good for structured data
   - OpenRouter: Access to multiple models including Claude, GPT-4, etc.

## Troubleshooting

### Command not found

```bash
# If globally installed
npm install -g agentic-synth

# If locally installed, use npx
npx agentic-synth generate
```

### API Key Issues

```bash
# Verify environment variables
agentic-synth config

# Check output shows:
# Environment Variables:
#   GEMINI_API_KEY: ✓ Set
```

### Build Issues

```bash
# Rebuild the package
cd packages/agentic-synth
npm run build
```

## API Integration

The CLI uses the same API as the programmatic interface. For advanced usage, see the [API documentation](./API.md).

## Support

- GitHub Issues: https://github.com/ruvnet/ruvector
- Documentation: https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth
