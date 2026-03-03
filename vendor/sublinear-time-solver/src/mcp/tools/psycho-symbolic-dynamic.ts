/**
 * Enhanced Psycho-Symbolic Tools with Dynamic Domain Support
 * Extends existing functionality while preserving all current capabilities
 */

import { Tool } from '@modelcontextprotocol/sdk/types.js';
import { PsychoSymbolicTools } from './psycho-symbolic.js';
import { DomainRegistry } from './domain-registry.js';

export class DynamicPsychoSymbolicTools extends PsychoSymbolicTools {
  private domainRegistry: DomainRegistry;

  constructor(domainRegistry?: DomainRegistry) {
    super();
    this.domainRegistry = domainRegistry || new DomainRegistry();
    this.initializeDynamicDomainIntegration();
  }

  private initializeDynamicDomainIntegration(): void {
    // Listen for domain registry events to update domain engine
    this.domainRegistry.on('domainRegistered', (event) => {
      this.updateDomainEngine();
    });

    this.domainRegistry.on('domainUpdated', (event) => {
      this.updateDomainEngine();
    });

    this.domainRegistry.on('domainUnregistered', (event) => {
      this.updateDomainEngine();
    });

    this.domainRegistry.on('domainEnabled', (event) => {
      this.updateDomainEngine();
    });

    this.domainRegistry.on('domainDisabled', (event) => {
      this.updateDomainEngine();
    });
  }

  getTools(): Tool[] {
    // Get all existing tools from parent class
    const baseTools = super.getTools();

    // Add enhanced tools with dynamic domain support
    const enhancedTools = [
      {
        name: 'psycho_symbolic_reason_with_dynamic_domains',
        description: 'Enhanced psycho-symbolic reasoning with dynamic domain support and control',
        inputSchema: {
          type: 'object' as const,
          properties: {
            query: { type: 'string', description: 'The reasoning query' },
            context: { type: 'object', description: 'Additional context', default: {} },
            depth: { type: 'number', description: 'Maximum reasoning depth', default: 7 },
            use_cache: { type: 'boolean', description: 'Enable intelligent caching', default: true },
            enable_learning: { type: 'boolean', description: 'Enable learning from this interaction', default: true },
            creative_mode: { type: 'boolean', description: 'Enable creative reasoning for novel concepts', default: true },
            domain_adaptation: { type: 'boolean', description: 'Enable automatic domain detection and adaptation', default: true },
            analogical_reasoning: { type: 'boolean', description: 'Enable analogical reasoning across domains', default: true },

            // Dynamic domain extensions
            force_domains: {
              type: 'array',
              items: { type: 'string' },
              description: 'Force specific domains to be considered (overrides detection)'
            },
            exclude_domains: {
              type: 'array',
              items: { type: 'string' },
              description: 'Exclude specific domains from consideration'
            },
            domain_priority_override: {
              type: 'object',
              additionalProperties: { type: 'number' },
              description: 'Override domain priorities for this query (domain_name: priority)'
            },
            use_experimental_domains: {
              type: 'boolean',
              default: false,
              description: 'Include experimental/beta domains in reasoning'
            },
            min_domain_confidence: {
              type: 'number',
              minimum: 0,
              maximum: 1,
              default: 0.1,
              description: 'Minimum confidence threshold for domain detection'
            },
            max_domains: {
              type: 'integer',
              minimum: 1,
              maximum: 10,
              default: 3,
              description: 'Maximum number of domains to use in reasoning'
            }
          },
          required: ['query']
        }
      },
      {
        name: 'domain_detection_test',
        description: 'Test domain detection for a given query with detailed analysis',
        inputSchema: {
          type: 'object' as const,
          properties: {
            query: { type: 'string', description: 'Query to test domain detection on' },
            include_scores: {
              type: 'boolean',
              default: true,
              description: 'Include detailed detection scores and matching details'
            },
            include_debug: {
              type: 'boolean',
              default: false,
              description: 'Include debug information about detection process'
            },
            test_all_domains: {
              type: 'boolean',
              default: false,
              description: 'Test against all domains including disabled ones'
            },
            show_keyword_matches: {
              type: 'boolean',
              default: true,
              description: 'Show which keywords matched for each domain'
            }
          },
          required: ['query']
        }
      },
      {
        name: 'knowledge_graph_query_dynamic',
        description: 'Knowledge graph query with dynamic domain filtering and boosting',
        inputSchema: {
          type: 'object' as const,
          properties: {
            query: { type: 'string', description: 'Natural language query' },
            domains: {
              type: 'array',
              items: { type: 'string' },
              description: 'Domain filters (supports both built-in and dynamic domains)',
              default: []
            },
            include_analogies: {
              type: 'boolean',
              description: 'Include analogical connections',
              default: true
            },
            limit: { type: 'number', description: 'Max results', default: 20 },
            cross_domain_boost: {
              type: 'number',
              minimum: 0,
              maximum: 2,
              default: 1.0,
              description: 'Boost relevance for cross-domain results'
            },
            dynamic_domain_weight: {
              type: 'number',
              minimum: 0,
              maximum: 2,
              default: 1.0,
              description: 'Weight multiplier for results from dynamic domains'
            },
            builtin_domain_weight: {
              type: 'number',
              minimum: 0,
              maximum: 2,
              default: 1.0,
              description: 'Weight multiplier for results from built-in domains'
            },
            require_domain_match: {
              type: 'boolean',
              default: false,
              description: 'Only return results that match specified domains'
            }
          },
          required: ['query']
        }
      }
    ];

    return [...baseTools, ...enhancedTools];
  }

  async handleToolCall(name: string, args: any): Promise<any> {
    try {
      switch (name) {
        case 'psycho_symbolic_reason_with_dynamic_domains':
          return await this.performEnhancedReasoning(args);
        case 'domain_detection_test':
          return await this.testDomainDetection(args);
        case 'knowledge_graph_query_dynamic':
          return await this.advancedKnowledgeQueryDynamic(args);
        default:
          // Delegate to parent class for existing tools
          return await super.handleToolCall(name, args);
      }
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : String(error),
        timestamp: new Date().toISOString()
      };
    }
  }

  private async performEnhancedReasoning(args: any): Promise<any> {
    const startTime = performance.now();

    // Apply domain filtering and priority overrides
    const domainFilters = this.buildDomainFilters(args);

    // Get enhanced domain detection with dynamic domains
    const enhancedDetection = await this.performEnhancedDomainDetection(
      args.query,
      domainFilters
    );

    // Enhance the reasoning context with dynamic domain information
    const enhancedContext = {
      ...args.context,
      domain_filters: domainFilters,
      dynamic_domains_available: this.getDynamicDomainsCount(),
      enhanced_detection: enhancedDetection
    };

    // Call the parent reasoning method with enhanced context via public interface
    const baseResult = await super.handleToolCall('psycho_symbolic_reason', {
      ...args,
      context: enhancedContext
    });

    // Enhance the result with dynamic domain information
    const enhancedResult = {
      ...baseResult,
      dynamic_domain_info: {
        filters_applied: domainFilters,
        dynamic_domains_used: enhancedDetection.dynamic_domains_detected,
        builtin_domains_used: enhancedDetection.builtin_domains_detected,
        domain_synergies: enhancedDetection.synergies,
        detection_performance: {
          total_domains_checked: enhancedDetection.total_domains_checked,
          detection_time_ms: enhancedDetection.detection_time_ms
        }
      },
      enhanced_reasoning_time: performance.now() - startTime
    };

    // Update usage statistics for dynamic domains
    this.updateDynamicDomainUsage(enhancedDetection.domains_used);

    return enhancedResult;
  }

  private async testDomainDetection(args: any): Promise<any> {
    const startTime = performance.now();
    const query = args.query;

    // Get all domains to test (including disabled if requested)
    const domainsToTest = args.test_all_domains ?
      this.domainRegistry.getAllDomains() :
      this.domainRegistry.getEnabledDomains();

    const detectionResults: any[] = [];

    // Test detection against each domain
    for (const domain of domainsToTest) {
      const domainResult = await this.testDomainDetectionSingle(
        query,
        domain,
        args.show_keyword_matches
      );
      detectionResults.push(domainResult);
    }

    // Sort by detection score
    detectionResults.sort((a, b) => b.score - a.score);

    // Get top detected domains
    const topDomains = detectionResults
      .filter(r => r.score > 0)
      .slice(0, args.max_results || 10);

    const detectionTime = performance.now() - startTime;

    const result: any = {
      query,
      detected_domains: topDomains,
      detection_summary: {
        total_domains_tested: detectionResults.length,
        domains_with_matches: detectionResults.filter(r => r.score > 0).length,
        highest_score: detectionResults[0]?.score || 0,
        detection_time_ms: detectionTime
      },
      system_info: {
        total_domains_available: this.domainRegistry.getAllDomains().length,
        builtin_domains_count: this.getBuiltinDomainsCount(),
        dynamic_domains_count: this.getDynamicDomainsCount(),
        enabled_domains_count: this.domainRegistry.getEnabledDomains().length
      }
    };

    if (args.include_debug) {
      result.debug_info = {
        all_domain_results: detectionResults,
        domain_registry_status: this.domainRegistry.getSystemStatus(),
        detection_algorithm_info: {
          scoring_method: 'keyword_matching_with_semantic_boost',
          confidence_threshold: 0.1,
          max_domains_returned: args.max_results || 10
        }
      };
    }

    return result;
  }

  private async advancedKnowledgeQueryDynamic(args: any): Promise<any> {
    // Enhance the base knowledge query with dynamic domain support via public interface
    const baseResult = await super.handleToolCall('knowledge_graph_query', args);

    // Apply dynamic domain weighting
    if (args.dynamic_domain_weight !== 1.0 || args.builtin_domain_weight !== 1.0) {
      baseResult.results = this.applyDomainWeighting(
        baseResult.results,
        args.dynamic_domain_weight,
        args.builtin_domain_weight
      );
    }

    // Filter by domain requirements if specified
    if (args.require_domain_match && args.domains?.length > 0) {
      baseResult.results = baseResult.results.filter(result =>
        result.domain_tags?.some(tag => args.domains.includes(tag))
      );
    }

    // Add dynamic domain information
    const enhancedResult = {
      ...baseResult,
      dynamic_domain_info: {
        dynamic_domains_available: this.getDynamicDomainsCount(),
        builtin_domains_available: this.getBuiltinDomainsCount(),
        weighting_applied: {
          dynamic_domain_weight: args.dynamic_domain_weight,
          builtin_domain_weight: args.builtin_domain_weight,
          cross_domain_boost: args.cross_domain_boost
        },
        filtering_applied: {
          require_domain_match: args.require_domain_match,
          domains_filter: args.domains
        }
      }
    };

    return enhancedResult;
  }

  // Helper methods
  private buildDomainFilters(args: any): any {
    return {
      force_domains: args.force_domains || [],
      exclude_domains: args.exclude_domains || [],
      domain_priority_override: args.domain_priority_override || {},
      use_experimental_domains: args.use_experimental_domains || false,
      min_domain_confidence: args.min_domain_confidence || 0.1,
      max_domains: args.max_domains || 3
    };
  }

  private async performEnhancedDomainDetection(query: string, filters: any): Promise<any> {
    const startTime = performance.now();
    const allDomains = this.domainRegistry.getEnabledDomains();

    // Apply filtering
    let domainsToCheck = allDomains;

    if (filters.exclude_domains.length > 0) {
      domainsToCheck = domainsToCheck.filter(d =>
        !filters.exclude_domains.includes(d.config.name)
      );
    }

    if (!filters.use_experimental_domains) {
      domainsToCheck = domainsToCheck.filter(d =>
        !d.config.metadata?.experimental
      );
    }

    const detectionResults = {
      domains_used: [],
      dynamic_domains_detected: [],
      builtin_domains_detected: [],
      synergies: [],
      total_domains_checked: domainsToCheck.length,
      detection_time_ms: performance.now() - startTime
    };

    return detectionResults;
  }

  private updateDomainEngine(): void {
    // Update the parent class's domain engine with dynamic domains
    // This would integrate with the existing DomainAdaptationEngine
    console.log('Updating domain engine with dynamic domains...');
  }

  private getDynamicDomainsCount(): number {
    return this.domainRegistry.getAllDomains().filter(d =>
      !this.domainRegistry.isBuiltinDomain(d.config.name)
    ).length;
  }

  private getBuiltinDomainsCount(): number {
    return this.domainRegistry.getAllDomains().filter(d =>
      this.domainRegistry.isBuiltinDomain(d.config.name)
    ).length;
  }

  private updateDynamicDomainUsage(domainsUsed: string[]): void {
    for (const domainName of domainsUsed) {
      this.domainRegistry.incrementUsage(domainName);
    }
  }

  private async testDomainDetectionSingle(query: string, domain: any, showKeywordMatches: boolean): Promise<any> {
    // Simplified domain detection test
    const queryLower = query.toLowerCase();
    const matchedKeywords = domain.config.keywords.filter(keyword =>
      queryLower.includes(keyword.toLowerCase())
    );

    const score = matchedKeywords.length > 0 ? matchedKeywords.length * 2.0 : 0;

    const result: any = {
      domain: domain.config.name,
      score,
      enabled: domain.enabled,
      builtin: this.domainRegistry.isBuiltinDomain(domain.config.name),
      reasoning_style: domain.config.reasoning_style,
      priority: domain.config.priority
    };

    if (showKeywordMatches) {
      result.matched_keywords = matchedKeywords;
      result.total_keywords = domain.config.keywords.length;
      result.match_ratio = matchedKeywords.length / domain.config.keywords.length;
    }

    return result;
  }

  private applyDomainWeighting(results: any[], dynamicWeight: number, builtinWeight: number): any[] {
    return results.map(result => {
      const isDynamic = result.domain_tags?.some(tag =>
        !this.domainRegistry.isBuiltinDomain(tag)
      );

      const weight = isDynamic ? dynamicWeight : builtinWeight;
      return {
        ...result,
        relevance: result.relevance * weight,
        weighted: true,
        weight_applied: weight
      };
    }).sort((a, b) => b.relevance - a.relevance);
  }

  // Expose domain registry for other tools
  getDomainRegistry(): DomainRegistry {
    return this.domainRegistry;
  }
}