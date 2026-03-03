# Psycho-Symbolic Reasoner API Documentation

## Table of Contents

- [Core API](#core-api)
- [MCP Tools](#mcp-tools)
- [CLI Commands](#cli-commands)
- [REST API](#rest-api)
- [WebSocket API](#websocket-api)
- [Configuration](#configuration)
- [Error Handling](#error-handling)
- [Examples](#examples)

## Core API

### PsychoSymbolicReasoner

The main class for interacting with the reasoning framework.

```typescript
import { PsychoSymbolicReasoner } from 'psycho-symbolic-reasoner';

class PsychoSymbolicReasoner {
  constructor(options?: ReasonerOptions);

  // Core methods
  async initialize(): Promise<void>;
  async loadKnowledgeBase(path: string): Promise<void>;
  async queryGraph(query: GraphQuery): Promise<GraphResult>;
  async extractSentiment(text: string): Promise<SentimentResult>;
  async extractPreferences(text: string): Promise<PreferenceResult>;
  async createPlan(request: PlanRequest): Promise<PlanResult>;

  // Utility methods
  isReady(): boolean;
  getStats(): ReasonerStats;
  dispose(): Promise<void>;
}
```

#### Configuration Options

```typescript
interface ReasonerOptions {
  enableGraphReasoning?: boolean;
  enableAffectExtraction?: boolean;
  enablePlanning?: boolean;
  knowledgeBasePath?: string;
  planningRulesPath?: string;
  logLevel?: 'debug' | 'info' | 'warn' | 'error';
  wasmPath?: string;
}
```

### Graph Reasoning

#### GraphQuery

```typescript
interface GraphQuery {
  pattern?: string;                    // SPARQL-like pattern
  constraints?: string[];              // Additional constraints
  maxResults?: number;                 // Limit results (default: 100)
  includeInference?: boolean;          // Apply inference rules
  timeout?: number;                    // Query timeout in ms
}

// Examples
const query1: GraphQuery = {
  pattern: "?person likes ?activity",
  constraints: ["?activity hasProperty relaxing"],
  maxResults: 10
};

const query2: GraphQuery = {
  pattern: "?technique helps ?condition",
  constraints: ["?condition = anxiety"],
  includeInference: true
};
```

#### GraphResult

```typescript
interface GraphResult {
  results: GraphBinding[];
  inferredFacts?: GraphBinding[];
  executionTime: number;
  totalResults: number;
}

interface GraphBinding {
  [variable: string]: GraphNode;
}

interface GraphNode {
  id: string;
  type: string;
  properties: Record<string, any>;
  relationships: GraphEdge[];
}
```

### Sentiment Analysis

#### SentimentResult

```typescript
interface SentimentResult {
  score: number;                       // -1.0 to 1.0
  confidence: number;                  // 0.0 to 1.0
  primaryEmotion: string;             // Detected primary emotion
  emotions: EmotionScore[];           // All detected emotions
  intensity: 'low' | 'medium' | 'high';
  context?: SentimentContext;
}

interface EmotionScore {
  emotion: string;
  score: number;
  confidence: number;
}

interface SentimentContext {
  valence: number;                     // Positive/negative dimension
  arousal: number;                     // Energy/activation level
  dominance: number;                   // Control/submission dimension
}
```

### Preference Extraction

#### PreferenceResult

```typescript
interface PreferenceResult {
  preferences: Preference[];
  confidence: number;
  categories: string[];
  context: PreferenceContext;
}

interface Preference {
  type: 'like' | 'dislike' | 'neutral';
  subject: string;
  object: string;
  strength: number;                    // 0.0 to 1.0
  confidence: number;
  context?: string;
}

interface PreferenceContext {
  domain: string;
  timeframe?: string;
  conditions?: string[];
}
```

### Planning

#### PlanRequest

```typescript
interface PlanRequest {
  goal: Goal;
  currentState: State;
  preferences?: Preference[];
  constraints?: Constraint[];
  maxSteps?: number;
  timeout?: number;
}

interface Goal {
  description: string;
  type: 'achievement' | 'maintenance' | 'avoidance';
  priority: number;                    // 0.0 to 1.0
  deadline?: Date;
  successCriteria: SuccessCriterion[];
}

interface State {
  facts: Fact[];
  context: Record<string, any>;
  resources: Resource[];
}
```

#### PlanResult

```typescript
interface PlanResult {
  plan: Action[];
  alternativePlans?: Action[][];
  confidence: number;
  estimatedDuration: number;
  estimatedCost: number;
  riskAssessment: RiskAssessment;
  explanation: string;
}

interface Action {
  id: string;
  name: string;
  description: string;
  preconditions: Condition[];
  effects: Effect[];
  duration: number;
  cost: number;
  priority: number;
}
```

## MCP Tools

The psycho-symbolic reasoner exposes several tools through the Model Context Protocol.

### Available Tools

#### queryGraph

Performs symbolic graph reasoning queries.

**Parameters:**
```json
{
  "query": "string",
  "maxResults": "number (optional)",
  "includeInference": "boolean (optional)"
}
```

**Example:**
```json
{
  "query": "find relaxation techniques for stressed users",
  "maxResults": 5,
  "includeInference": true
}
```

#### extractSentiment

Analyzes sentiment and emotional context from text.

**Parameters:**
```json
{
  "text": "string",
  "includeEmotions": "boolean (optional)",
  "includeContext": "boolean (optional)"
}
```

**Example:**
```json
{
  "text": "I'm feeling overwhelmed with work deadlines",
  "includeEmotions": true,
  "includeContext": true
}
```

#### extractPreferences

Identifies user preferences from text or behavioral data.

**Parameters:**
```json
{
  "text": "string",
  "domain": "string (optional)",
  "includeImplicit": "boolean (optional)"
}
```

**Example:**
```json
{
  "text": "I prefer working in the morning when it's quiet",
  "domain": "work_habits",
  "includeImplicit": true
}
```

#### createPlan

Generates goal-oriented plans based on current state and preferences.

**Parameters:**
```json
{
  "goal": "string",
  "currentState": "object",
  "preferences": "array (optional)",
  "constraints": "array (optional)"
}
```

**Example:**
```json
{
  "goal": "reduce stress and improve work-life balance",
  "currentState": {
    "energy": "low",
    "workload": "high",
    "timeAvailable": "limited"
  },
  "preferences": [
    {
      "type": "like",
      "subject": "user",
      "object": "short_activities"
    }
  ]
}
```

#### analyzeContext

Performs contextual analysis combining multiple reasoning modes.

**Parameters:**
```json
{
  "text": "string",
  "context": "object (optional)",
  "analysisTypes": "array (optional)"
}
```

**Example:**
```json
{
  "text": "User seems frustrated with current workflow",
  "context": {
    "domain": "workplace",
    "userHistory": ["previous_complaints", "low_satisfaction"]
  },
  "analysisTypes": ["sentiment", "preferences", "recommendations"]
}
```

## CLI Commands

### Global Commands

```bash
# Display help
psycho-symbolic-reasoner --help

# Check version
psycho-symbolic-reasoner --version

# Start MCP server
psycho-symbolic-reasoner serve [options]

# Initialize knowledge base
psycho-symbolic-reasoner init [options]
```

### Analysis Commands

```bash
# Analyze sentiment
psycho-symbolic-reasoner analyze sentiment \
  --text "I'm excited about this project!" \
  --output json

# Extract preferences
psycho-symbolic-reasoner analyze preferences \
  --text "I like working early in the morning" \
  --domain work_habits

# Query knowledge graph
psycho-symbolic-reasoner query \
  --pattern "?person needs ?support" \
  --constraints "?person hasEmotion stressed" \
  --max-results 10
```

### Planning Commands

```bash
# Create a plan
psycho-symbolic-reasoner plan \
  --goal "improve sleep quality" \
  --state ./current-state.json \
  --preferences ./user-preferences.json \
  --output plan.json

# Validate a plan
psycho-symbolic-reasoner validate-plan \
  --plan ./plan.json \
  --rules ./planning-rules.json
```

### Data Management

```bash
# Load knowledge base
psycho-symbolic-reasoner load-kb \
  --file ./knowledge-base.json \
  --format json

# Export data
psycho-symbolic-reasoner export \
  --type preferences \
  --format csv \
  --output user-preferences.csv

# Import external data
psycho-symbolic-reasoner import \
  --file external-data.rdf \
  --format rdf
```

## REST API

When running in server mode, the reasoner exposes a REST API.

### Base URL

```
http://localhost:3000/api/v1
```

### Authentication

```http
Authorization: Bearer <token>
```

### Endpoints

#### POST /analyze/sentiment

Analyze sentiment in text.

```http
POST /api/v1/analyze/sentiment
Content-Type: application/json

{
  "text": "I'm feeling great today!",
  "includeEmotions": true
}
```

#### POST /analyze/preferences

Extract preferences from text.

```http
POST /api/v1/analyze/preferences
Content-Type: application/json

{
  "text": "I prefer quiet environments for concentration",
  "domain": "work_environment"
}
```

#### POST /query/graph

Query the knowledge graph.

```http
POST /api/v1/query/graph
Content-Type: application/json

{
  "pattern": "?activity helps ?condition",
  "constraints": ["?condition = anxiety"],
  "maxResults": 5
}
```

#### POST /planning/create

Create a new plan.

```http
POST /api/v1/planning/create
Content-Type: application/json

{
  "goal": {
    "description": "improve focus during work",
    "type": "achievement",
    "priority": 0.8
  },
  "currentState": {
    "energy": "medium",
    "environment": "noisy",
    "timeAvailable": "2 hours"
  }
}
```

#### GET /stats

Get system statistics.

```http
GET /api/v1/stats
```

Response:
```json
{
  "uptime": 3600,
  "queriesProcessed": 1250,
  "averageResponseTime": 45,
  "memoryUsage": {
    "used": "125MB",
    "total": "512MB"
  },
  "knowledgeBase": {
    "nodes": 5000,
    "edges": 12000,
    "rules": 150
  }
}
```

## WebSocket API

Real-time communication for streaming analysis and interactive planning.

### Connection

```javascript
const ws = new WebSocket('ws://localhost:3000/ws');
```

### Message Format

```json
{
  "id": "unique-request-id",
  "type": "request|response|error|event",
  "method": "analyze|plan|query|subscribe",
  "data": {}
}
```

### Real-time Sentiment Analysis

```javascript
// Request
ws.send(JSON.stringify({
  id: "sentiment-1",
  type: "request",
  method: "analyze",
  data: {
    type: "sentiment",
    text: "I'm feeling overwhelmed",
    stream: true
  }
}));

// Response
{
  "id": "sentiment-1",
  "type": "response",
  "data": {
    "score": -0.3,
    "confidence": 0.85,
    "primaryEmotion": "stress"
  }
}
```

### Live Planning Updates

```javascript
// Subscribe to planning updates
ws.send(JSON.stringify({
  id: "plan-updates",
  type: "request",
  method: "subscribe",
  data: {
    topic: "planning",
    planId: "plan-123"
  }
}));

// Receive updates
{
  "id": "plan-updates",
  "type": "event",
  "data": {
    "planId": "plan-123",
    "status": "executing",
    "currentStep": 3,
    "totalSteps": 10,
    "progress": 0.3
  }
}
```

## Configuration

### Environment Variables

```bash
# Server configuration
PSR_PORT=3000
PSR_HOST=localhost
PSR_LOG_LEVEL=info

# WASM configuration
PSR_WASM_PATH=./wasm
PSR_ENABLE_WASM_CACHE=true

# Knowledge base
PSR_KB_PATH=./knowledge-base.json
PSR_KB_AUTO_RELOAD=true

# Security
PSR_API_KEY=your-api-key
PSR_ENABLE_CORS=true
PSR_RATE_LIMIT=100

# Performance
PSR_MAX_CONCURRENT_QUERIES=10
PSR_QUERY_TIMEOUT=30000
PSR_CACHE_SIZE=1000
```

### Configuration File

```json
{
  "server": {
    "port": 3000,
    "host": "localhost",
    "cors": {
      "enabled": true,
      "origins": ["http://localhost:3000"]
    }
  },
  "reasoning": {
    "graph": {
      "enableInference": true,
      "maxQueryTime": 30000,
      "cacheResults": true
    },
    "sentiment": {
      "model": "advanced",
      "includeEmotions": true,
      "confidenceThreshold": 0.7
    },
    "planning": {
      "maxSteps": 20,
      "timeout": 60000,
      "enableOptimization": true
    }
  },
  "wasm": {
    "path": "./wasm",
    "enableCache": true,
    "memoryLimit": "512MB"
  },
  "logging": {
    "level": "info",
    "format": "json",
    "outputs": ["console", "file"]
  }
}
```

## Error Handling

### Error Types

```typescript
enum ErrorType {
  INITIALIZATION_ERROR = 'initialization_error',
  VALIDATION_ERROR = 'validation_error',
  QUERY_ERROR = 'query_error',
  PLANNING_ERROR = 'planning_error',
  WASM_ERROR = 'wasm_error',
  TIMEOUT_ERROR = 'timeout_error',
  RESOURCE_ERROR = 'resource_error'
}
```

### Error Response Format

```json
{
  "error": {
    "type": "validation_error",
    "message": "Invalid query pattern",
    "code": "PSR_001",
    "details": {
      "field": "pattern",
      "expected": "SPARQL-like pattern",
      "received": "invalid syntax"
    },
    "timestamp": "2024-09-20T10:30:00Z",
    "requestId": "req-123"
  }
}
```

### Error Handling in Code

```typescript
try {
  const result = await reasoner.queryGraph(query);
  console.log(result);
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Invalid input:', error.message);
  } else if (error instanceof TimeoutError) {
    console.error('Query timed out:', error.timeout);
  } else {
    console.error('Unexpected error:', error);
  }
}
```

## Examples

### Basic Usage

```typescript
import { PsychoSymbolicReasoner } from 'psycho-symbolic-reasoner';

async function example() {
  const reasoner = new PsychoSymbolicReasoner({
    enableGraphReasoning: true,
    enableAffectExtraction: true,
    enablePlanning: true
  });

  await reasoner.initialize();
  await reasoner.loadKnowledgeBase('./therapy-kb.json');

  // Analyze user input
  const sentiment = await reasoner.extractSentiment(
    "I've been feeling anxious about upcoming deadlines"
  );

  // Extract preferences
  const preferences = await reasoner.extractPreferences(
    "I find deep breathing exercises really helpful"
  );

  // Create a plan
  const plan = await reasoner.createPlan({
    goal: {
      description: "reduce anxiety",
      type: "achievement",
      priority: 0.9
    },
    currentState: {
      facts: [
        { predicate: "hasEmotion", object: "anxiety" },
        { predicate: "hasDeadline", object: "upcoming" }
      ],
      context: { timeAvailable: "30 minutes" }
    },
    preferences: preferences.preferences
  });

  console.log('Plan:', plan.plan);
}
```

### MCP Integration

```typescript
import { FastMCP } from 'fastmcp';
import { createPsychoSymbolicTools } from 'psycho-symbolic-reasoner/mcp';

const server = new FastMCP({
  name: "TherapyAssistant",
  version: "1.0.0"
});

const tools = await createPsychoSymbolicTools({
  knowledgeBase: './therapy-knowledge.json'
});

tools.forEach(tool => server.addTool(tool));

await server.start({ transportType: "stdio" });
```

### Advanced Graph Querying

```typescript
// Complex reasoning query
const result = await reasoner.queryGraph({
  pattern: `
    ?person hasEmotion ?emotion .
    ?emotion hasIntensity ?intensity .
    ?technique helps ?emotion .
    ?technique hasType ?type .
  `,
  constraints: [
    "?intensity > 0.7",
    "?type IN ['breathing', 'mindfulness', 'cognitive']"
  ],
  includeInference: true,
  maxResults: 10
});

console.log('Recommended techniques:', result.results);
```

For more detailed examples, see the [`examples/`](../examples/) directory.