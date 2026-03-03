/**
 * Enhanced Psycho-Symbolic Reasoning with Learning Integration
 * Fixes novel knowledge integration and adds cross-tool learning
 */
import * as crypto from 'crypto';
import { ReasoningCache } from './reasoning-cache.js';
// Enhanced knowledge base with learning capabilities
class LearningKnowledgeBase {
    triples = new Map();
    concepts = new Map();
    predicateIndex = new Map();
    semanticIndex = new Map();
    learningEvents = [];
    constructor() {
        this.initializeBaseKnowledge();
    }
    initializeBaseKnowledge() {
        // Enhanced core knowledge with learning metadata
        this.addLearningTriple('consciousness', 'emerges_from', 'neural_networks', 0.85, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('consciousness', 'requires', 'integration', 0.9, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('consciousness', 'exhibits', 'phi_value', 0.95, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('neural_networks', 'process', 'information', 1.0, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('neural_networks', 'contain', 'neurons', 1.0, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('neurons', 'connect_via', 'synapses', 1.0, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('synapses', 'enable', 'plasticity', 0.9, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('plasticity', 'allows', 'learning', 0.95, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('learning', 'modifies', 'weights', 1.0, {
            type: 'foundational',
            learning_source: 'initialization'
        });
        this.addLearningTriple('phi_value', 'measures', 'integrated_information', 1.0, {
            type: 'foundational',
            learning_source: 'initialization'
        });
    }
    addLearningTriple(subject, predicate, object, confidence, metadata = {}) {
        const id = crypto.createHash('md5').update(`${subject}_${predicate}_${object}`).digest('hex').substring(0, 16);
        const triple = {
            subject,
            predicate,
            object,
            confidence,
            metadata,
            timestamp: Date.now(),
            usage_count: 0,
            learning_source: metadata.learning_source || 'user_input',
            related_concepts: this.findRelatedConcepts(subject, object)
        };
        this.triples.set(id, triple);
        this.updateIndices(id, triple);
        return { id, status: 'added', triple };
    }
    findRelatedConcepts(subject, object) {
        const related = [];
        // Find concepts that share predicates
        for (const [id, triple] of this.triples) {
            if (triple.subject === subject || triple.object === subject) {
                related.push(triple.subject, triple.object);
            }
            if (triple.subject === object || triple.object === object) {
                related.push(triple.subject, triple.object);
            }
        }
        return [...new Set(related)].filter(c => c !== subject && c !== object);
    }
    updateIndices(id, triple) {
        // Update concept indices
        [triple.subject, triple.object].forEach(concept => {
            if (!this.concepts.has(concept))
                this.concepts.set(concept, new Set());
            this.concepts.get(concept).add(id);
        });
        // Update predicate index
        if (!this.predicateIndex.has(triple.predicate)) {
            this.predicateIndex.set(triple.predicate, new Set());
        }
        this.predicateIndex.get(triple.predicate).add(id);
        // Update semantic index
        this.updateSemanticIndex(triple);
    }
    updateSemanticIndex(triple) {
        const concepts = [triple.subject, triple.object];
        concepts.forEach(concept => {
            if (!this.semanticIndex.has(concept)) {
                this.semanticIndex.set(concept, []);
            }
            // Add related concepts for semantic similarity
            if (triple.related_concepts) {
                this.semanticIndex.get(concept).push(...triple.related_concepts);
            }
        });
    }
    // Fix: Implement missing getAllTriples method
    getAllTriples() {
        return Array.from(this.triples.values());
    }
    // Enhanced semantic search with learning integration
    semanticSearch(query, limit = 10) {
        const results = [];
        const queryLower = query.toLowerCase();
        const queryTerms = queryLower.split(/\s+/);
        for (const [id, triple] of this.triples) {
            let relevance = 0;
            // Direct text matching
            if (triple.subject.toLowerCase().includes(queryLower))
                relevance += 2.0;
            if (triple.object.toLowerCase().includes(queryLower))
                relevance += 2.0;
            if (triple.predicate.toLowerCase().includes(queryLower))
                relevance += 1.0;
            // Term-based matching
            queryTerms.forEach(term => {
                if (term.length > 2) {
                    if (triple.subject.toLowerCase().includes(term))
                        relevance += 0.8;
                    if (triple.object.toLowerCase().includes(term))
                        relevance += 0.8;
                    if (triple.predicate.toLowerCase().includes(term))
                        relevance += 0.4;
                }
            });
            // Semantic similarity bonus
            if (triple.related_concepts) {
                triple.related_concepts.forEach(concept => {
                    if (queryLower.includes(concept.toLowerCase()))
                        relevance += 0.3;
                });
            }
            // Usage-based relevance boost
            relevance += Math.log(triple.usage_count + 1) * 0.1;
            // Confidence weighting
            relevance *= triple.confidence;
            if (relevance > 0.1) {
                results.push({
                    ...triple,
                    relevance,
                    id
                });
            }
        }
        // Sort by relevance and usage
        return results
            .sort((a, b) => {
            const scoreA = a.relevance + (a.usage_count * 0.01);
            const scoreB = b.relevance + (b.usage_count * 0.01);
            return scoreB - scoreA;
        })
            .slice(0, limit);
    }
    // Track triple usage for learning
    markTripleUsed(tripleId) {
        const triple = this.triples.get(tripleId);
        if (triple) {
            triple.usage_count++;
        }
    }
    // Learning from tool interactions
    recordLearningEvent(event) {
        this.learningEvents.push(event);
        // Auto-generate knowledge from successful patterns
        if (event.confidence > 0.8) {
            this.generateKnowledgeFromEvent(event);
        }
        // Keep only recent events (last 1000)
        if (this.learningEvents.length > 1000) {
            this.learningEvents = this.learningEvents.slice(-1000);
        }
    }
    generateKnowledgeFromEvent(event) {
        // Generate knowledge triples from successful tool interactions
        if (event.concepts.length >= 2) {
            for (let i = 0; i < event.concepts.length - 1; i++) {
                const subject = event.concepts[i];
                const object = event.concepts[i + 1];
                // Create relationship based on tool and action
                let predicate = 'relates_to';
                if (event.tool === 'consciousness')
                    predicate = 'influences_consciousness';
                if (event.tool === 'scheduler')
                    predicate = 'schedules_with';
                if (event.tool === 'neural')
                    predicate = 'processes_through';
                this.addLearningTriple(subject, predicate, object, event.confidence * 0.7, {
                    type: 'learned_from_interaction',
                    learning_source: `${event.tool}_${event.action}`,
                    original_event: event
                });
            }
        }
    }
    // Get learning insights
    getLearningInsights() {
        const recentEvents = this.learningEvents.slice(-100);
        const conceptFrequency = new Map();
        const toolUsage = new Map();
        recentEvents.forEach(event => {
            event.concepts.forEach(concept => {
                conceptFrequency.set(concept, (conceptFrequency.get(concept) || 0) + 1);
            });
            toolUsage.set(event.tool, (toolUsage.get(event.tool) || 0) + 1);
        });
        return {
            total_events: this.learningEvents.length,
            recent_events: recentEvents.length,
            top_concepts: Array.from(conceptFrequency.entries())
                .sort((a, b) => b[1] - a[1])
                .slice(0, 10),
            tool_usage: Array.from(toolUsage.entries()),
            learned_triples: this.getAllTriples().filter(t => t.learning_source !== 'initialization').length
        };
    }
}
// Cross-tool learning coordinator
class CrossToolLearningCoordinator {
    knowledgeBase;
    toolInteractions = new Map();
    constructor(knowledgeBase) {
        this.knowledgeBase = knowledgeBase;
    }
    // Record interaction with other tools
    recordToolInteraction(toolName, query, result, concepts) {
        const interaction = {
            tool: toolName,
            query,
            result,
            concepts,
            timestamp: Date.now(),
            success: result.confidence > 0.7
        };
        if (!this.toolInteractions.has(toolName)) {
            this.toolInteractions.set(toolName, []);
        }
        this.toolInteractions.get(toolName).push(interaction);
        // Learn from successful interactions
        if (interaction.success) {
            this.knowledgeBase.recordLearningEvent({
                tool: toolName,
                action: 'query',
                concepts,
                patterns: result.patterns || [],
                outcome: result.answer || 'success',
                timestamp: Date.now(),
                confidence: result.confidence
            });
        }
    }
    // Get cross-tool insights for enhanced reasoning
    getCrossToolInsights(concepts) {
        const insights = [];
        // Find related tool interactions
        for (const [tool, interactions] of this.toolInteractions) {
            const relevantInteractions = interactions.filter(interaction => concepts.some(concept => interaction.concepts.includes(concept) ||
                interaction.query.toLowerCase().includes(concept.toLowerCase())));
            if (relevantInteractions.length > 0) {
                insights.push(`${tool} tool has processed similar concepts with ${relevantInteractions.length} relevant interactions`);
                // Extract patterns from successful interactions
                const successfulInteractions = relevantInteractions.filter(i => i.success);
                if (successfulInteractions.length > 0) {
                    insights.push(`${tool} successfully handled ${successfulInteractions.length} similar queries`);
                }
            }
        }
        return insights;
    }
}
// Enhanced psycho-symbolic reasoning with learning
export class LearningPsychoSymbolicTools {
    knowledgeBase;
    learningCoordinator;
    performanceCache;
    reasoningCache = new Map();
    constructor() {
        this.knowledgeBase = new LearningKnowledgeBase();
        this.learningCoordinator = new CrossToolLearningCoordinator(this.knowledgeBase);
        this.performanceCache = new ReasoningCache();
    }
    getTools() {
        return [
            {
                name: 'psycho_symbolic_reason',
                description: 'Enhanced psycho-symbolic reasoning with learning integration and novel knowledge support',
                inputSchema: {
                    type: 'object',
                    properties: {
                        query: { type: 'string', description: 'The reasoning query' },
                        context: { type: 'object', description: 'Additional context', default: {} },
                        depth: { type: 'number', description: 'Maximum reasoning depth', default: 6 },
                        use_cache: { type: 'boolean', description: 'Enable intelligent caching', default: true },
                        learn_from_query: { type: 'boolean', description: 'Learn from this query for future use', default: true }
                    },
                    required: ['query']
                }
            },
            {
                name: 'knowledge_graph_query',
                description: 'Enhanced knowledge graph query with learning-based relevance',
                inputSchema: {
                    type: 'object',
                    properties: {
                        query: { type: 'string', description: 'Natural language query' },
                        filters: { type: 'object', description: 'Query filters', default: {} },
                        limit: { type: 'number', description: 'Max results', default: 15 }
                    },
                    required: ['query']
                }
            },
            {
                name: 'add_knowledge',
                description: 'Add knowledge with learning metadata and semantic indexing',
                inputSchema: {
                    type: 'object',
                    properties: {
                        subject: { type: 'string' },
                        predicate: { type: 'string' },
                        object: { type: 'string' },
                        confidence: { type: 'number', default: 1.0 },
                        metadata: { type: 'object', default: {} }
                    },
                    required: ['subject', 'predicate', 'object']
                }
            },
            {
                name: 'learning_status',
                description: 'Get learning system status and insights',
                inputSchema: {
                    type: 'object',
                    properties: {
                        detailed: { type: 'boolean', description: 'Include detailed learning metrics', default: false }
                    }
                }
            }
        ];
    }
    async handleToolCall(name, args) {
        switch (name) {
            case 'psycho_symbolic_reason':
                return this.performLearningReasoning(args.query, args.context || {}, args.depth || 6, args.use_cache !== false, args.learn_from_query !== false);
            case 'knowledge_graph_query':
                return this.enhancedKnowledgeQuery(args.query, args.filters || {}, args.limit || 15);
            case 'add_knowledge':
                return this.knowledgeBase.addLearningTriple(args.subject, args.predicate, args.object, args.confidence || 1.0, { ...args.metadata, learning_source: 'user_input' });
            case 'learning_status':
                return this.getLearningStatus(args.detailed || false);
            default:
                throw new Error(`Unknown tool: ${name}`);
        }
    }
    async performLearningReasoning(query, context, maxDepth, useCache, learnFromQuery) {
        const startTime = performance.now();
        // Extract concepts early for learning
        const entities = this.extractEntitiesAndConcepts(query);
        const patterns = this.identifyCognitivePatterns(query);
        // Check cache
        if (useCache) {
            const cached = this.performanceCache.get(query, context, maxDepth);
            if (cached) {
                return {
                    ...cached.result,
                    cached: true,
                    cache_hit: true,
                    compute_time: performance.now() - startTime,
                    cache_metrics: this.performanceCache.getMetrics()
                };
            }
        }
        const reasoningSteps = [];
        const insights = new Set();
        // Step 1: Enhanced Pattern Recognition
        reasoningSteps.push({
            type: 'pattern_identification',
            patterns,
            confidence: 0.9,
            description: `Identified ${patterns.join(', ')} reasoning patterns`
        });
        // Step 2: Enhanced Entity Extraction with Learning
        reasoningSteps.push({
            type: 'entity_extraction',
            entities: entities.entities,
            concepts: entities.concepts,
            relationships: entities.relationships,
            confidence: 0.85
        });
        // Step 3: Cross-Tool Learning Insights
        const crossToolInsights = this.learningCoordinator.getCrossToolInsights(entities.concepts);
        if (crossToolInsights.length > 0) {
            crossToolInsights.forEach(insight => insights.add(insight));
            reasoningSteps.push({
                type: 'cross_tool_learning',
                insights: crossToolInsights,
                confidence: 0.8,
                description: 'Insights from related tool interactions'
            });
        }
        // Step 4: Enhanced Knowledge Traversal with Novel Concept Support
        const graphInsights = await this.enhancedKnowledgeTraversal(entities.concepts, maxDepth);
        reasoningSteps.push({
            type: 'enhanced_knowledge_traversal',
            paths: graphInsights.paths,
            discoveries: graphInsights.discoveries,
            novel_concepts: graphInsights.novel_concepts,
            confidence: graphInsights.confidence
        });
        graphInsights.discoveries.forEach(d => insights.add(d));
        // Step 5: Learning from Domain Analysis
        const domainInsights = this.generateLearningDomainInsights(query, patterns, entities.concepts);
        domainInsights.forEach(insight => insights.add(insight));
        reasoningSteps.push({
            type: 'learning_domain_analysis',
            insights: domainInsights,
            confidence: 0.8,
            description: 'Generated domain insights with learning integration'
        });
        // Step 6: Synthesis
        const synthesis = this.synthesizeLearningAnswer(query, Array.from(insights), reasoningSteps, patterns, entities.concepts);
        // Record learning event
        if (learnFromQuery) {
            this.knowledgeBase.recordLearningEvent({
                tool: 'psycho_symbolic_reasoner',
                action: 'reason',
                concepts: entities.concepts,
                patterns,
                outcome: synthesis.answer,
                timestamp: Date.now(),
                confidence: synthesis.confidence
            });
        }
        const result = {
            answer: synthesis.answer,
            confidence: synthesis.confidence,
            reasoning: reasoningSteps,
            insights: Array.from(insights),
            patterns,
            depth: maxDepth,
            entities: entities.entities,
            concepts: entities.concepts,
            triples_examined: graphInsights.triples_examined,
            novel_concepts_processed: graphInsights.novel_concepts?.length || 0,
            learning_insights: crossToolInsights.length
        };
        // Cache result
        if (useCache) {
            this.performanceCache.set(query, context, maxDepth, result, performance.now() - startTime);
        }
        return {
            ...result,
            cached: false,
            cache_hit: false,
            compute_time: performance.now() - startTime,
            cache_metrics: useCache ? this.performanceCache.getMetrics() : null
        };
    }
    identifyCognitivePatterns(query) {
        const patterns = [];
        const lowerQuery = query.toLowerCase();
        const patternMap = {
            'causal': ['why', 'cause', 'because', 'result', 'effect', 'lead to'],
            'procedural': ['how', 'process', 'step', 'method', 'way', 'approach', 'design', 'implement'],
            'hypothetical': ['what if', 'suppose', 'imagine', 'could', 'would', 'might'],
            'comparative': ['compare', 'difference', 'similar', 'versus', 'than', 'like'],
            'definitional': ['what is', 'define', 'meaning', 'definition'],
            'evaluative': ['best', 'worst', 'better', 'optimal', 'evaluate'],
            'temporal': ['when', 'time', 'before', 'after', 'during', 'temporal'],
            'spatial': ['where', 'location', 'position', 'space'],
            'quantitative': ['how many', 'how much', 'count', 'measure', 'amount'],
            'existential': ['exist', 'there is', 'there are', 'presence'],
            'universal': ['all', 'every', 'always', 'never', 'none'],
            'lateral': ['lateral', 'unconventional', 'creative', 'alternative', 'non-obvious', 'hidden'],
            'systems': ['system', 'interaction', 'complexity', 'emergence', 'holistic'],
            'exploratory': ['explore', 'discover', 'investigate', 'consider', 'edge case']
        };
        for (const [pattern, keywords] of Object.entries(patternMap)) {
            if (keywords.some(keyword => lowerQuery.includes(keyword))) {
                patterns.push(pattern);
            }
        }
        if (patterns.length === 0) {
            patterns.push('exploratory');
        }
        return patterns;
    }
    extractEntitiesAndConcepts(query) {
        const words = query.split(/\s+/);
        const entities = [];
        const concepts = [];
        const relationships = [];
        // Extract technical terms and concepts
        const technicalTerms = [
            'api', 'rest', 'graphql', 'user', 'management', 'authentication',
            'authorization', 'database', 'cache', 'security', 'performance',
            'scalability', 'microservice', 'distributed', 'system', 'design',
            'endpoint', 'resource', 'crud', 'http', 'json', 'xml', 'oauth',
            'jwt', 'session', 'token', 'password', 'encryption', 'hash',
            'consciousness', 'neural', 'quantum', 'temporal', 'resonance',
            'emergence', 'integration', 'plasticity', 'learning'
        ];
        // Extract named entities
        for (let i = 0; i < words.length; i++) {
            const word = words[i];
            const wordLower = word.toLowerCase();
            if (/^[A-Z]/.test(word) && i > 0 && !['The', 'A', 'An', 'What', 'How', 'Why', 'When', 'Where'].includes(word)) {
                entities.push(wordLower);
            }
            if (technicalTerms.includes(wordLower) || word.length > 5) {
                concepts.push(wordLower);
            }
        }
        // Extract key concepts from knowledge base - FIXED
        const queryLower = query.toLowerCase();
        const allTriples = this.knowledgeBase.getAllTriples(); // Now this method exists!
        for (const triple of allTriples) {
            [triple.subject, triple.object].forEach(concept => {
                if (queryLower.includes(concept.toLowerCase())) {
                    concepts.push(concept);
                }
            });
        }
        // Extract relationships
        const relationshipPatterns = [
            'is', 'are', 'was', 'were', 'has', 'have', 'had',
            'can', 'could', 'will', 'would', 'should',
            'design', 'implement', 'create', 'build', 'develop',
            'requires', 'needs', 'uses', 'enables', 'prevents',
            'increases', 'decreases', 'affects', 'influences'
        ];
        for (const word of words) {
            const wordLower = word.toLowerCase();
            if (relationshipPatterns.includes(wordLower)) {
                relationships.push(wordLower);
            }
        }
        return {
            entities: [...new Set(entities)],
            concepts: [...new Set(concepts)],
            relationships: [...new Set(relationships)]
        };
    }
    async enhancedKnowledgeTraversal(concepts, maxDepth) {
        const paths = [];
        const discoveries = [];
        const novel_concepts = [];
        let triples_examined = 0;
        for (const concept of concepts) {
            // Semantic search with learning
            const results = this.knowledgeBase.semanticSearch(concept, 10);
            triples_examined += results.length;
            if (results.length === 0) {
                // This is a novel concept
                novel_concepts.push(concept);
                discoveries.push(`Novel concept detected: ${concept} - generating creative associations`);
                // Generate creative associations for novel concepts
                const creativeAssociations = this.generateCreativeAssociations(concept);
                discoveries.push(...creativeAssociations);
            }
            else {
                // Mark used triples for learning
                results.forEach(result => {
                    this.knowledgeBase.markTripleUsed(result.id);
                    discoveries.push(`${result.subject} ${result.predicate} ${result.object}`);
                    paths.push([result.subject, result.object]);
                });
            }
        }
        return {
            paths,
            discoveries,
            novel_concepts,
            confidence: discoveries.length > 0 ? 0.9 : 0.3,
            triples_examined
        };
    }
    generateCreativeAssociations(concept) {
        const associations = [];
        const conceptLower = concept.toLowerCase();
        // Pattern-based associations
        if (conceptLower.includes('quantum')) {
            associations.push(`${concept} exhibits quantum-like properties with probabilistic behaviors`);
            associations.push(`${concept} demonstrates non-local correlations similar to entanglement`);
        }
        if (conceptLower.includes('neural') || conceptLower.includes('network')) {
            associations.push(`${concept} functions as a distributed information processing system`);
            associations.push(`${concept} exhibits emergent properties through interconnected components`);
        }
        if (conceptLower.includes('temporal') || conceptLower.includes('time')) {
            associations.push(`${concept} creates temporal dynamics affecting system evolution`);
            associations.push(`${concept} enables time-based pattern recognition and prediction`);
        }
        // Morphological associations
        if (conceptLower.endsWith('ium') || conceptLower.endsWith('ium_crystals')) {
            associations.push(`${concept} acts as a resonant medium for information transfer`);
            associations.push(`${concept} exhibits crystalline structure enabling coherent oscillations`);
        }
        // Generic creative associations
        associations.push(`${concept} emerges through self-organizing complexity dynamics`);
        associations.push(`${concept} demonstrates adaptive behavior in response to environmental changes`);
        return associations;
    }
    generateLearningDomainInsights(query, patterns, concepts) {
        const insights = [];
        const queryLower = query.toLowerCase();
        // Learning-enhanced domain insights
        if (concepts.some(c => ['consciousness', 'neural', 'quantum'].includes(c))) {
            insights.push('Consciousness emerges through quantum-neural information integration');
            insights.push('Neural plasticity enables adaptive consciousness formation');
        }
        if (patterns.includes('temporal') || concepts.some(c => c.includes('temporal'))) {
            insights.push('Temporal dynamics create causal chains in complex systems');
            insights.push('Time-based resonance patterns enable cross-domain synchronization');
        }
        if (patterns.includes('creative') || patterns.includes('exploratory')) {
            insights.push('Creative synthesis requires breaking conventional categorical boundaries');
            insights.push('Novel concepts emerge at the intersection of established domains');
        }
        // Novel concept handling
        const novelConcepts = concepts.filter(c => !['consciousness', 'neural', 'quantum', 'system', 'information'].includes(c));
        if (novelConcepts.length > 0) {
            insights.push(`Novel concept integration suggests emergent properties beyond current knowledge`);
            insights.push(`Interdisciplinary synthesis reveals hidden connections between ${novelConcepts.join(' and ')}`);
        }
        return insights;
    }
    synthesizeLearningAnswer(query, insights, reasoningSteps, patterns, concepts) {
        let answer = '';
        let confidence = 0.8;
        if (insights.length === 0) {
            answer = 'This query involves novel concepts that require creative synthesis across multiple domains. The system is learning from this interaction to improve future responses.';
            confidence = 0.6;
        }
        else if (patterns.includes('creative') || patterns.includes('exploratory')) {
            answer = `Through learning-enhanced analysis: ${insights.slice(0, 4).join('. ')}.`;
            confidence = 0.85;
        }
        else {
            answer = `Based on integrated knowledge and learning: ${insights.slice(0, 5).join('. ')}.`;
        }
        return { answer, confidence };
    }
    enhancedKnowledgeQuery(query, filters, limit) {
        const results = this.knowledgeBase.semanticSearch(query, limit);
        return {
            query,
            results: results.map(r => ({
                subject: r.subject,
                predicate: r.predicate,
                object: r.object,
                confidence: r.confidence,
                relevance: r.relevance,
                usage_count: r.usage_count,
                learning_source: r.learning_source
            })),
            total: results.length,
            totalAvailable: this.knowledgeBase.getAllTriples().length
        };
    }
    getLearningStatus(detailed) {
        const insights = this.knowledgeBase.getLearningInsights();
        if (detailed) {
            return {
                ...insights,
                cache_metrics: this.performanceCache.getMetrics(),
                knowledge_base_size: this.knowledgeBase.getAllTriples().length,
                novel_concepts_learned: insights.learned_triples
            };
        }
        return {
            learning_active: true,
            total_knowledge: this.knowledgeBase.getAllTriples().length,
            learned_concepts: insights.learned_triples,
            recent_interactions: insights.recent_events
        };
    }
}
