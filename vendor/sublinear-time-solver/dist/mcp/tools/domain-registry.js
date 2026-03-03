/**
 * Domain Registry Core System
 * Manages dynamic domain registration, validation, and lifecycle
 */
import { EventEmitter } from 'events';
// Built-in domain configurations (preserved from existing system)
const BUILTIN_DOMAINS = {
    physics: {
        keywords: ['quantum', 'particle', 'energy', 'field', 'force', 'wave', 'resonance', 'entanglement'],
        reasoning_style: 'mathematical_modeling',
        analogy_domains: ['information_theory', 'consciousness', 'computing'],
        priority: 90
    },
    biology: {
        keywords: ['cell', 'organism', 'evolution', 'genetic', 'ecosystem', 'neural', 'brain'],
        reasoning_style: 'emergent_systems',
        analogy_domains: ['computer_networks', 'social_systems', 'economics'],
        priority: 90
    },
    computer_science: {
        keywords: ['algorithm', 'data', 'network', 'system', 'computation', 'software', 'ai', 'machine', 'learning', 'neural', 'artificial'],
        reasoning_style: 'systematic_analysis',
        analogy_domains: ['biology', 'physics', 'cognitive_science'],
        priority: 90
    },
    consciousness: {
        keywords: ['consciousness', 'awareness', 'mind', 'experience', 'qualia', 'phi'],
        reasoning_style: 'phenomenological',
        analogy_domains: ['physics', 'information_theory', 'complexity_science'],
        priority: 90
    },
    temporal: {
        keywords: ['time', 'temporal', 'sequence', 'causality', 'evolution', 'dynamics'],
        reasoning_style: 'temporal_analysis',
        analogy_domains: ['physics', 'consciousness', 'systems_theory'],
        priority: 90
    },
    art: {
        keywords: ['art', 'artistic', 'painting', 'visual', 'aesthetic', 'creative', 'expression', 'pollock', 'drip', 'canvas', 'color', 'form', 'style', 'composition'],
        reasoning_style: 'aesthetic_synthesis',
        analogy_domains: ['mathematics', 'physics', 'psychology', 'philosophy'],
        priority: 85
    },
    music: {
        keywords: ['music', 'musical', 'sound', 'rhythm', 'melody', 'harmony', 'composition', 'jazz', 'improvisation', 'symphony', 'acoustic', 'tone', 'chord'],
        reasoning_style: 'harmonic_analysis',
        analogy_domains: ['mathematics', 'physics', 'emotion', 'language'],
        priority: 85
    },
    narrative: {
        keywords: ['story', 'narrative', 'plot', 'character', 'fiction', 'novel', 'literary', 'text', 'author', 'dialogue', 'scene', 'chapter'],
        reasoning_style: 'narrative_analysis',
        analogy_domains: ['psychology', 'philosophy', 'sociology', 'linguistics'],
        priority: 85
    },
    philosophy: {
        keywords: ['philosophy', 'philosophical', 'metaphysics', 'ontology', 'epistemology', 'ethics', 'logic', 'existence', 'reality', 'truth'],
        reasoning_style: 'conceptual_analysis',
        analogy_domains: ['logic', 'psychology', 'mathematics', 'consciousness'],
        priority: 85
    },
    emotion: {
        keywords: ['emotion', 'emotional', 'feeling', 'mood', 'sentiment', 'empathy', 'psychology', 'affect', 'resonance'],
        reasoning_style: 'empathetic_reasoning',
        analogy_domains: ['neuroscience', 'art', 'music', 'social_dynamics'],
        priority: 85
    },
    mathematics: {
        keywords: ['mathematical', 'equation', 'function', 'theorem', 'proof', 'geometry', 'algebra', 'calculus', 'topology', 'fractal', 'chaos', 'matrix', 'solving', 'optimization', 'linear', 'algorithm', 'sublinear', 'portfolio', 'finance', 'trading'],
        reasoning_style: 'formal_reasoning',
        analogy_domains: ['physics', 'art', 'music', 'nature'],
        priority: 90
    },
    finance: {
        keywords: ['finance', 'financial', 'trading', 'portfolio', 'investment', 'market', 'economic', 'risk', 'return', 'asset', 'optimization', 'allocation', 'hedge', 'quant', 'stock', 'stocks', 'crypto', 'cryptocurrency', 'bitcoin', 'bonds', 'equity', 'derivative', 'futures', 'options', 'forex', 'currency', 'commodity', 'etf', 'mutual', 'fund', 'capital', 'valuation', 'pricing', 'yield', 'dividend', 'volatility', 'sharpe', 'alpha', 'beta', 'correlation', 'covariance', 'diversification', 'arbitrage', 'liquidity', 'leverage', 'margin', 'short', 'long', 'bull', 'bear', 'momentum', 'trend', 'technical', 'fundamental', 'analysis', 'backtesting', 'monte', 'carlo', 'black', 'scholes', 'var', 'credit', 'default', 'swap', 'spread', 'duration', 'convexity'],
        reasoning_style: 'quantitative_analysis',
        analogy_domains: ['mathematics', 'computer_science', 'statistics', 'game_theory'],
        priority: 85
    }
};
export class DomainRegistry extends EventEmitter {
    domains = new Map();
    loadOrder = [];
    builtinDomains = new Set();
    constructor() {
        super();
        this.initializeBuiltinDomains();
    }
    initializeBuiltinDomains() {
        // Register all built-in domains as immutable defaults
        for (const [name, config] of Object.entries(BUILTIN_DOMAINS)) {
            const fullConfig = {
                name,
                version: '1.0.0',
                description: `Built-in ${name} domain`,
                keywords: config.keywords || [],
                reasoning_style: config.reasoning_style || 'systematic_analysis',
                analogy_domains: config.analogy_domains || [],
                semantic_clusters: [],
                cross_domain_mappings: [],
                inference_rules: [],
                priority: config.priority || 80,
                dependencies: [],
                metadata: { builtin: true, immutable: true }
            };
            const plugin = {
                config: fullConfig,
                enabled: true,
                registered_at: Date.now(),
                updated_at: Date.now(),
                usage_count: 0,
                performance_metrics: {
                    detection_accuracy: 0.9,
                    reasoning_time_avg: 0,
                    memory_usage: 0,
                    success_rate: 0.95,
                    last_measured: Date.now()
                },
                validation_status: {
                    valid: true,
                    score: 100,
                    issues: [],
                    tested_at: Date.now()
                }
            };
            this.domains.set(name, plugin);
            this.builtinDomains.add(name);
            this.loadOrder.push(name);
        }
    }
    async registerDomain(config) {
        const warnings = [];
        // Check if domain already exists
        if (this.domains.has(config.name)) {
            if (this.builtinDomains.has(config.name)) {
                throw new Error(`Cannot register domain '${config.name}': built-in domains are immutable`);
            }
            throw new Error(`Domain '${config.name}' already exists. Use updateDomain to modify existing domains.`);
        }
        // Validate dependencies
        for (const dep of config.dependencies) {
            if (!this.domains.has(dep)) {
                throw new Error(`Dependency '${dep}' not found for domain '${config.name}'`);
            }
        }
        // Check for keyword conflicts
        const keywordConflicts = this.checkKeywordConflicts(config);
        if (keywordConflicts.length > 0) {
            warnings.push(`Keyword conflicts detected with domains: ${keywordConflicts.join(', ')}`);
        }
        // Create domain plugin
        const plugin = {
            config: { ...config },
            enabled: true,
            registered_at: Date.now(),
            updated_at: Date.now(),
            usage_count: 0,
            performance_metrics: {
                detection_accuracy: 0,
                reasoning_time_avg: 0,
                memory_usage: 0,
                success_rate: 0,
                last_measured: Date.now()
            },
            validation_status: {
                valid: true,
                score: 85, // Default score for new domains
                issues: [],
                tested_at: Date.now()
            }
        };
        // Add to registry
        this.domains.set(config.name, plugin);
        this.insertInLoadOrder(config.name, config.priority);
        // Emit registration event
        this.emit('domainRegistered', { domain: config.name, config });
        return {
            success: true,
            id: config.name,
            warnings: warnings.length > 0 ? warnings : undefined
        };
    }
    async updateDomain(name, updates) {
        if (this.builtinDomains.has(name)) {
            throw new Error(`Cannot update built-in domain '${name}': built-in domains are immutable`);
        }
        const plugin = this.domains.get(name);
        if (!plugin) {
            throw new Error(`Domain '${name}' not found`);
        }
        const warnings = [];
        const oldConfig = { ...plugin.config };
        // Merge updates
        plugin.config = { ...plugin.config, ...updates };
        plugin.updated_at = Date.now();
        // Re-validate dependencies if they changed
        if (updates.dependencies) {
            for (const dep of updates.dependencies) {
                if (!this.domains.has(dep)) {
                    throw new Error(`Dependency '${dep}' not found for domain '${name}'`);
                }
            }
        }
        // Check for new keyword conflicts if keywords changed
        if (updates.keywords) {
            const keywordConflicts = this.checkKeywordConflicts(plugin.config, name);
            if (keywordConflicts.length > 0) {
                warnings.push(`Keyword conflicts detected with domains: ${keywordConflicts.join(', ')}`);
            }
        }
        // Update load order if priority changed
        if (updates.priority !== undefined) {
            this.removeFromLoadOrder(name);
            this.insertInLoadOrder(name, updates.priority);
        }
        // Emit update event
        this.emit('domainUpdated', { domain: name, oldConfig, newConfig: plugin.config });
        return {
            success: true,
            warnings: warnings.length > 0 ? warnings : undefined
        };
    }
    async unregisterDomain(name, options = {}) {
        if (this.builtinDomains.has(name)) {
            throw new Error(`Cannot unregister built-in domain '${name}': built-in domains are immutable`);
        }
        const plugin = this.domains.get(name);
        if (!plugin) {
            throw new Error(`Domain '${name}' not found`);
        }
        // Check for dependents unless force is true
        if (!options.force) {
            const dependents = this.findDependentDomains(name);
            if (dependents.length > 0) {
                throw new Error(`Cannot unregister domain '${name}': other domains depend on it: ${dependents.join(', ')}`);
            }
        }
        // Remove from registry
        this.domains.delete(name);
        this.removeFromLoadOrder(name);
        // Emit unregistration event
        this.emit('domainUnregistered', { domain: name, config: plugin.config });
        return { success: true };
    }
    enableDomain(name) {
        const plugin = this.domains.get(name);
        if (!plugin) {
            throw new Error(`Domain '${name}' not found`);
        }
        plugin.enabled = true;
        this.emit('domainEnabled', { domain: name });
        return { success: true };
    }
    disableDomain(name) {
        if (this.builtinDomains.has(name)) {
            throw new Error(`Cannot disable built-in domain '${name}': built-in domains cannot be disabled`);
        }
        const plugin = this.domains.get(name);
        if (!plugin) {
            throw new Error(`Domain '${name}' not found`);
        }
        plugin.enabled = false;
        this.emit('domainDisabled', { domain: name });
        return { success: true };
    }
    getDomain(name) {
        return this.domains.get(name) || null;
    }
    getAllDomains() {
        return Array.from(this.domains.values());
    }
    getEnabledDomains() {
        return Array.from(this.domains.values()).filter(d => d.enabled);
    }
    getLoadOrder() {
        return [...this.loadOrder];
    }
    isDomainEnabled(name) {
        const plugin = this.domains.get(name);
        return plugin ? plugin.enabled : false;
    }
    isBuiltinDomain(name) {
        return this.builtinDomains.has(name);
    }
    updatePerformanceMetrics(name, metrics) {
        const plugin = this.domains.get(name);
        if (plugin) {
            plugin.performance_metrics = { ...plugin.performance_metrics, ...metrics };
            plugin.performance_metrics.last_measured = Date.now();
        }
    }
    incrementUsage(name) {
        const plugin = this.domains.get(name);
        if (plugin) {
            plugin.usage_count++;
        }
    }
    checkKeywordConflicts(config, excludeDomain) {
        const conflicts = [];
        const newKeywords = new Set(config.keywords.map(k => k.toLowerCase()));
        for (const [domainName, plugin] of this.domains) {
            if (domainName === excludeDomain)
                continue;
            const existingKeywords = new Set(plugin.config.keywords.map(k => k.toLowerCase()));
            const overlap = [...newKeywords].filter(k => existingKeywords.has(k));
            if (overlap.length > 0) {
                conflicts.push(domainName);
            }
        }
        return conflicts;
    }
    findDependentDomains(domainName) {
        const dependents = [];
        for (const [name, plugin] of this.domains) {
            if (plugin.config.dependencies.includes(domainName)) {
                dependents.push(name);
            }
        }
        return dependents;
    }
    insertInLoadOrder(name, priority) {
        // Insert domain in priority order (higher priority first)
        let insertIndex = this.loadOrder.length;
        for (let i = 0; i < this.loadOrder.length; i++) {
            const existingDomain = this.domains.get(this.loadOrder[i]);
            if (existingDomain && existingDomain.config.priority < priority) {
                insertIndex = i;
                break;
            }
        }
        this.loadOrder.splice(insertIndex, 0, name);
    }
    removeFromLoadOrder(name) {
        const index = this.loadOrder.indexOf(name);
        if (index !== -1) {
            this.loadOrder.splice(index, 1);
        }
    }
    // Health check and status methods
    getSystemStatus() {
        const enabled = this.getEnabledDomains().length;
        const total = this.domains.size;
        return {
            total_domains: total,
            builtin_domains: this.builtinDomains.size,
            custom_domains: total - this.builtinDomains.size,
            enabled_domains: enabled,
            disabled_domains: total - enabled,
            load_order: this.getLoadOrder()
        };
    }
    validateSystemIntegrity() {
        const issues = [];
        // Check all built-in domains are present
        for (const builtinName of Object.keys(BUILTIN_DOMAINS)) {
            if (!this.domains.has(builtinName)) {
                issues.push(`Missing built-in domain: ${builtinName}`);
            }
        }
        // Check all dependencies are satisfied
        for (const [name, plugin] of this.domains) {
            for (const dep of plugin.config.dependencies) {
                if (!this.domains.has(dep)) {
                    issues.push(`Domain '${name}' has missing dependency: ${dep}`);
                }
            }
        }
        // Check load order consistency
        const expectedOrder = [...this.domains.keys()].sort((a, b) => {
            const priorityA = this.domains.get(a)?.config.priority || 0;
            const priorityB = this.domains.get(b)?.config.priority || 0;
            return priorityB - priorityA;
        });
        const actualOrder = this.loadOrder.slice();
        if (JSON.stringify(expectedOrder) !== JSON.stringify(actualOrder)) {
            issues.push('Load order is inconsistent with domain priorities');
        }
        return {
            valid: issues.length === 0,
            issues
        };
    }
}
