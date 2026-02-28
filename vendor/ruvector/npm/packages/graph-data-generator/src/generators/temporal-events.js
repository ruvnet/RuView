"use strict";
/**
 * Temporal events generator for time-series graph data
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.TemporalEventsGenerator = void 0;
exports.createTemporalEventsGenerator = createTemporalEventsGenerator;
class TemporalEventsGenerator {
    constructor(client) {
        this.client = client;
    }
    /**
     * Generate temporal event graph data
     */
    async generate(options) {
        const startTime = Date.now();
        // Generate events
        const events = await this.generateEvents(options);
        // Generate entities involved in events
        const entities = await this.generateEntities(events, options);
        // Convert to graph structure
        const eventNodes = events.map(event => ({
            id: event.id,
            labels: ['Event', event.type],
            properties: {
                type: event.type,
                timestamp: event.timestamp.toISOString(),
                ...event.properties
            }
        }));
        const entityNodes = entities.map(entity => ({
            id: entity.id,
            labels: ['Entity', entity.type],
            properties: entity.properties
        }));
        const edges = [];
        let edgeId = 0;
        // Create edges between events and entities
        for (const event of events) {
            for (const entityId of event.entities) {
                edges.push({
                    id: `edge_${edgeId++}`,
                    type: 'INVOLVES',
                    source: event.id,
                    target: entityId,
                    properties: {
                        timestamp: event.timestamp.toISOString()
                    }
                });
            }
            // Create edges for relationships
            if (event.relationships) {
                for (const rel of event.relationships) {
                    edges.push({
                        id: `edge_${edgeId++}`,
                        type: rel.type,
                        source: event.id,
                        target: rel.target,
                        properties: {
                            timestamp: event.timestamp.toISOString()
                        }
                    });
                }
            }
        }
        // Create temporal sequences (NEXT relationships)
        const sortedEvents = [...events].sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());
        for (let i = 0; i < sortedEvents.length - 1; i++) {
            edges.push({
                id: `edge_${edgeId++}`,
                type: 'NEXT',
                source: sortedEvents[i].id,
                target: sortedEvents[i + 1].id,
                properties: {
                    time_diff_ms: sortedEvents[i + 1].timestamp.getTime() - sortedEvents[i].timestamp.getTime()
                }
            });
        }
        const data = {
            nodes: [...eventNodes, ...entityNodes],
            edges,
            metadata: {
                domain: 'temporal-events',
                generated_at: new Date(),
                total_nodes: eventNodes.length + entityNodes.length,
                total_edges: edges.length
            }
        };
        return {
            data,
            metadata: {
                generated_at: new Date(),
                model: this.client.getConfig().model || 'moonshot/kimi-k2-instruct',
                duration: Date.now() - startTime
            }
        };
    }
    /**
     * Generate temporal events
     */
    async generateEvents(options) {
        const startDate = new Date(options.startDate);
        const endDate = new Date(options.endDate);
        const daysDiff = Math.ceil((endDate.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24));
        const totalEvents = (options.eventsPerDay || 10) * daysDiff;
        const systemPrompt = 'You are an expert at generating realistic temporal event sequences. Create events that follow logical patterns and causality.';
        const userPrompt = `Generate ${totalEvents} temporal events between ${startDate.toISOString()} and ${endDate.toISOString()}.

Event types to include: ${options.eventTypes.join(', ')}

Each event should have:
- id: unique event ID (format: event_XXXXX)
- type: one of the specified event types
- timestamp: ISO 8601 timestamp within the date range
- entities: array of entity IDs involved (format: entity_XXXXX)
- properties: relevant properties for the event
- relationships: (optional) array of relationships to other events/entities

Create realistic temporal patterns (e.g., business hours for work events, clustering of related events).

Return a JSON array sorted by timestamp.

Example format:
\`\`\`json
[
  {
    "id": "event_00001",
    "type": "login",
    "timestamp": "2024-01-15T09:23:15Z",
    "entities": ["entity_user_001"],
    "properties": {
      "ip_address": "192.168.1.100",
      "device": "desktop",
      "success": true
    },
    "relationships": [
      {
        "type": "TRIGGERED_BY",
        "target": "entity_user_001"
      }
    ]
  }
]
\`\`\``;
        const events = await this.client.generateStructured(systemPrompt, userPrompt, {
            temperature: 0.8,
            maxTokens: Math.min(8000, totalEvents * 150)
        });
        // Convert timestamp strings to Date objects
        return events.map(event => ({
            ...event,
            timestamp: new Date(event.timestamp)
        }));
    }
    /**
     * Generate entities from events
     */
    async generateEntities(events, options) {
        const uniqueEntityIds = new Set();
        events.forEach(event => {
            event.entities.forEach(entityId => uniqueEntityIds.add(entityId));
        });
        const entityIds = Array.from(uniqueEntityIds);
        const entityCount = options.entities || entityIds.length;
        const systemPrompt = 'You are an expert at creating entity profiles for event-driven systems.';
        const sampleIds = entityIds.slice(0, 50).join(', ');
        const userPrompt = `Generate ${entityCount} entity profiles for entities involved in temporal events.

Sample entity IDs that must be included: ${sampleIds}

Each entity should have:
- id: the entity ID
- type: entity type (User, System, Device, Service, etc.)
- properties: relevant properties for the entity

Return a JSON array of entities.

Example format:
\`\`\`json
[
  {
    "id": "entity_user_001",
    "type": "User",
    "properties": {
      "name": "Alice Smith",
      "email": "alice@example.com",
      "role": "developer",
      "created_at": "2023-01-15T00:00:00Z"
    }
  }
]
\`\`\``;
        return this.client.generateStructured(systemPrompt, userPrompt, {
            temperature: 0.7,
            maxTokens: Math.min(8000, entityCount * 100)
        });
    }
    /**
     * Analyze temporal patterns
     */
    async analyzeTemporalPatterns(events) {
        const eventsPerHour = {};
        const eventTypeDistribution = {};
        const timeDiffs = [];
        const sortedEvents = [...events].sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());
        for (let i = 0; i < sortedEvents.length; i++) {
            const event = sortedEvents[i];
            const hour = event.timestamp.toISOString().substring(0, 13);
            eventsPerHour[hour] = (eventsPerHour[hour] || 0) + 1;
            eventTypeDistribution[event.type] = (eventTypeDistribution[event.type] || 0) + 1;
            if (i > 0) {
                timeDiffs.push(event.timestamp.getTime() - sortedEvents[i - 1].timestamp.getTime());
            }
        }
        const avgTimeBetweenEvents = timeDiffs.length > 0
            ? timeDiffs.reduce((a, b) => a + b, 0) / timeDiffs.length
            : 0;
        return {
            eventsPerHour,
            eventTypeDistribution,
            avgTimeBetweenEvents
        };
    }
}
exports.TemporalEventsGenerator = TemporalEventsGenerator;
/**
 * Create a temporal events generator
 */
function createTemporalEventsGenerator(client) {
    return new TemporalEventsGenerator(client);
}
//# sourceMappingURL=temporal-events.js.map