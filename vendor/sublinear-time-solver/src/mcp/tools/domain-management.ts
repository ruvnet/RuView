/**
 * Domain Management MCP Tools
 * Provides CRUD operations for domain registry through MCP interface
 */

import { Tool } from '@modelcontextprotocol/sdk/types.js';
import { DomainRegistry, DomainConfig, DomainPlugin, ValidationResult } from './domain-registry.js';

export class DomainManagementTools {
  private domainRegistry: DomainRegistry;

  constructor() {
    this.domainRegistry = new DomainRegistry();
  }

  getTools(): Tool[] {
    return [
      {
        name: 'domain_register',
        description: 'Register a new reasoning domain with validation and testing',
        inputSchema: {
          type: 'object',
          properties: {
            name: {
              type: 'string',
              pattern: '^[a-z_]+$',
              description: 'Domain identifier (lowercase with underscores)'
            },
            version: {
              type: 'string',
              pattern: '^\\d+\\.\\d+\\.\\d+$',
              description: 'Semantic version (e.g., 1.0.0)'
            },
            description: {
              type: 'string',
              maxLength: 500,
              description: 'Domain description'
            },
            keywords: {
              type: 'array',
              items: { type: 'string', minLength: 2 },
              minItems: 3,
              uniqueItems: true,
              description: 'Keywords for domain detection (minimum 3 required)'
            },
            reasoning_style: {
              type: 'string',
              enum: [
                'custom', 'mathematical_modeling', 'emergent_systems', 'systematic_analysis',
                'phenomenological', 'temporal_analysis', 'aesthetic_synthesis', 'harmonic_analysis',
                'narrative_analysis', 'conceptual_analysis', 'empathetic_reasoning', 'formal_reasoning',
                'quantitative_analysis', 'creative_synthesis'
              ],
              description: 'Reasoning style for this domain'
            },
            custom_reasoning_description: {
              type: 'string',
              description: 'Custom reasoning description (required if reasoning_style is "custom")'
            },
            analogy_domains: {
              type: 'array',
              items: { type: 'string' },
              default: [],
              description: 'Related domains for analogical reasoning'
            },
            semantic_clusters: {
              type: 'array',
              items: { type: 'string' },
              default: [],
              description: 'Semantic concept clusters'
            },
            cross_domain_mappings: {
              type: 'array',
              items: { type: 'string' },
              default: [],
              description: 'Cross-domain connection concepts'
            },
            inference_rules: {
              type: 'array',
              items: {
                type: 'object',
                properties: {
                  name: { type: 'string' },
                  pattern: { type: 'string' },
                  action: { type: 'string' },
                  confidence: { type: 'number', minimum: 0, maximum: 1 },
                  conditions: { type: 'array', items: { type: 'string' } }
                },
                required: ['name', 'pattern', 'action']
              },
              default: [],
              description: 'Custom inference rules'
            },
            priority: {
              type: 'integer',
              minimum: 0,
              maximum: 100,
              default: 50,
              description: 'Domain priority for detection conflicts (0-100, higher = more priority)'
            },
            dependencies: {
              type: 'array',
              items: { type: 'string' },
              default: [],
              description: 'Required domain dependencies'
            },
            validate_before_register: {
              type: 'boolean',
              default: true,
              description: 'Run validation before registration'
            },
            enable_immediately: {
              type: 'boolean',
              default: true,
              description: 'Enable domain immediately after registration'
            }
          },
          required: ['name', 'version', 'description', 'keywords', 'reasoning_style']
        }
      },
      {
        name: 'domain_list',
        description: 'List all registered domains with status and metadata',
        inputSchema: {
          type: 'object',
          properties: {
            filter: {
              type: 'string',
              enum: ['all', 'enabled', 'disabled', 'builtin', 'custom'],
              default: 'all',
              description: 'Filter domains by status'
            },
            include_metadata: {
              type: 'boolean',
              default: false,
              description: 'Include detailed metadata and performance metrics'
            },
            sort_by: {
              type: 'string',
              enum: ['name', 'priority', 'usage', 'performance'],
              default: 'priority',
              description: 'Sort criteria'
            },
            sort_order: {
              type: 'string',
              enum: ['asc', 'desc'],
              default: 'desc',
              description: 'Sort order'
            }
          }
        }
      },
      {
        name: 'domain_get',
        description: 'Get detailed information about a specific domain',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Domain name' },
            include_performance: {
              type: 'boolean',
              default: true,
              description: 'Include performance metrics'
            },
            include_usage_stats: {
              type: 'boolean',
              default: true,
              description: 'Include usage statistics'
            },
            include_relationships: {
              type: 'boolean',
              default: false,
              description: 'Include domain relationships and dependencies'
            }
          },
          required: ['name']
        }
      },
      {
        name: 'domain_update',
        description: 'Update an existing domain configuration',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Domain name to update' },
            updates: {
              type: 'object',
              description: 'Partial domain configuration updates',
              properties: {
                description: { type: 'string', maxLength: 500 },
                keywords: {
                  type: 'array',
                  items: { type: 'string', minLength: 2 },
                  minItems: 3,
                  uniqueItems: true
                },
                reasoning_style: {
                  type: 'string',
                  enum: [
                    'custom', 'mathematical_modeling', 'emergent_systems', 'systematic_analysis',
                    'phenomenological', 'temporal_analysis', 'aesthetic_synthesis', 'harmonic_analysis',
                    'narrative_analysis', 'conceptual_analysis', 'empathetic_reasoning', 'formal_reasoning',
                    'quantitative_analysis', 'creative_synthesis'
                  ]
                },
                custom_reasoning_description: { type: 'string' },
                analogy_domains: { type: 'array', items: { type: 'string' } },
                semantic_clusters: { type: 'array', items: { type: 'string' } },
                cross_domain_mappings: { type: 'array', items: { type: 'string' } },
                priority: { type: 'integer', minimum: 0, maximum: 100 },
                dependencies: { type: 'array', items: { type: 'string' } }
              }
            },
            validate_before_update: {
              type: 'boolean',
              default: true,
              description: 'Run validation before applying updates'
            },
            create_backup: {
              type: 'boolean',
              default: true,
              description: 'Create backup before updating'
            }
          },
          required: ['name', 'updates']
        }
      },
      {
        name: 'domain_unregister',
        description: 'Unregister a domain from the system',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Domain name to unregister' },
            force: {
              type: 'boolean',
              default: false,
              description: 'Force removal even with dependencies (dangerous)'
            },
            cleanup_knowledge: {
              type: 'boolean',
              default: false,
              description: 'Remove domain-specific knowledge from knowledge base'
            }
          },
          required: ['name']
        }
      },
      {
        name: 'domain_enable',
        description: 'Enable a registered domain',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Domain name to enable' }
          },
          required: ['name']
        }
      },
      {
        name: 'domain_disable',
        description: 'Disable a domain temporarily',
        inputSchema: {
          type: 'object',
          properties: {
            name: { type: 'string', description: 'Domain name to disable' }
          },
          required: ['name']
        }
      },
      {
        name: 'domain_system_status',
        description: 'Get overall domain system status and health',
        inputSchema: {
          type: 'object',
          properties: {
            include_integrity_check: {
              type: 'boolean',
              default: true,
              description: 'Run system integrity validation'
            },
            include_performance_summary: {
              type: 'boolean',
              default: false,
              description: 'Include performance summary across all domains'
            }
          }
        }
      }
    ];
  }

  async handleToolCall(name: string, args: any): Promise<any> {
    try {
      switch (name) {
        case 'domain_register':
          return await this.registerDomain(args);
        case 'domain_list':
          return this.listDomains(args);
        case 'domain_get':
          return this.getDomain(args);
        case 'domain_update':
          return await this.updateDomain(args);
        case 'domain_unregister':
          return await this.unregisterDomain(args);
        case 'domain_enable':
          return this.enableDomain(args);
        case 'domain_disable':
          return this.disableDomain(args);
        case 'domain_system_status':
          return this.getSystemStatus(args);
        default:
          throw new Error(`Unknown domain management tool: ${name}`);
      }
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : String(error),
        timestamp: new Date().toISOString()
      };
    }
  }

  private async registerDomain(args: any): Promise<any> {
    // Validate custom reasoning description if needed
    if (args.reasoning_style === 'custom' && !args.custom_reasoning_description) {
      throw new Error('custom_reasoning_description is required when reasoning_style is "custom"');
    }

    // Build domain configuration
    const config: DomainConfig = {
      name: args.name,
      version: args.version,
      description: args.description,
      keywords: args.keywords,
      reasoning_style: args.reasoning_style,
      custom_reasoning_description: args.custom_reasoning_description,
      analogy_domains: args.analogy_domains || [],
      semantic_clusters: args.semantic_clusters || [],
      cross_domain_mappings: args.cross_domain_mappings || [],
      inference_rules: args.inference_rules || [],
      priority: args.priority || 50,
      dependencies: args.dependencies || []
    };

    // Register domain
    const result = await this.domainRegistry.registerDomain(config);

    // Disable if requested
    if (!args.enable_immediately) {
      this.domainRegistry.disableDomain(args.name);
    }

    return {
      success: true,
      domain_id: result.id,
      registered_at: new Date().toISOString(),
      enabled: args.enable_immediately !== false,
      warnings: result.warnings,
      system_status: this.domainRegistry.getSystemStatus()
    };
  }

  private listDomains(args: any): any {
    let domains = this.domainRegistry.getAllDomains();

    // Apply filters
    switch (args.filter) {
      case 'enabled':
        domains = domains.filter(d => d.enabled);
        break;
      case 'disabled':
        domains = domains.filter(d => !d.enabled);
        break;
      case 'builtin':
        domains = domains.filter(d => this.domainRegistry.isBuiltinDomain(d.config.name));
        break;
      case 'custom':
        domains = domains.filter(d => !this.domainRegistry.isBuiltinDomain(d.config.name));
        break;
    }

    // Sort domains
    const sortKey = args.sort_by || 'priority';
    const sortOrder = args.sort_order || 'desc';
    domains.sort((a, b) => {
      let comparison = 0;
      switch (sortKey) {
        case 'name':
          comparison = a.config.name.localeCompare(b.config.name);
          break;
        case 'priority':
          comparison = a.config.priority - b.config.priority;
          break;
        case 'usage':
          comparison = a.usage_count - b.usage_count;
          break;
        case 'performance':
          comparison = a.performance_metrics.success_rate - b.performance_metrics.success_rate;
          break;
      }
      return sortOrder === 'desc' ? -comparison : comparison;
    });

    // Format response
    const domainList = domains.map(domain => {
      const basic = {
        name: domain.config.name,
        version: domain.config.version,
        description: domain.config.description,
        enabled: domain.enabled,
        priority: domain.config.priority,
        builtin: this.domainRegistry.isBuiltinDomain(domain.config.name),
        reasoning_style: domain.config.reasoning_style,
        keywords_count: domain.config.keywords.length,
        dependencies_count: domain.config.dependencies.length,
        usage_count: domain.usage_count,
        registered_at: new Date(domain.registered_at).toISOString()
      };

      if (args.include_metadata) {
        return {
          ...basic,
          keywords: domain.config.keywords,
          analogy_domains: domain.config.analogy_domains,
          dependencies: domain.config.dependencies,
          performance_metrics: domain.performance_metrics,
          validation_status: domain.validation_status,
          updated_at: new Date(domain.updated_at).toISOString()
        };
      }

      return basic;
    });

    return {
      domains: domainList,
      total: domainList.length,
      filter_applied: args.filter || 'all',
      sort_by: sortKey,
      sort_order: sortOrder,
      system_status: this.domainRegistry.getSystemStatus()
    };
  }

  private getDomain(args: any): any {
    const plugin = this.domainRegistry.getDomain(args.name);
    if (!plugin) {
      throw new Error(`Domain '${args.name}' not found`);
    }

    const result: any = {
      name: plugin.config.name,
      version: plugin.config.version,
      description: plugin.config.description,
      enabled: plugin.enabled,
      builtin: this.domainRegistry.isBuiltinDomain(plugin.config.name),
      config: {
        keywords: plugin.config.keywords,
        reasoning_style: plugin.config.reasoning_style,
        custom_reasoning_description: plugin.config.custom_reasoning_description,
        analogy_domains: plugin.config.analogy_domains,
        semantic_clusters: plugin.config.semantic_clusters,
        cross_domain_mappings: plugin.config.cross_domain_mappings,
        inference_rules: plugin.config.inference_rules,
        priority: plugin.config.priority,
        dependencies: plugin.config.dependencies
      },
      registered_at: new Date(plugin.registered_at).toISOString(),
      updated_at: new Date(plugin.updated_at).toISOString()
    };

    if (args.include_performance) {
      result.performance_metrics = plugin.performance_metrics;
    }

    if (args.include_usage_stats) {
      result.usage_statistics = {
        usage_count: plugin.usage_count,
        last_used: plugin.performance_metrics.last_measured ?
          new Date(plugin.performance_metrics.last_measured).toISOString() : null
      };
    }

    if (args.include_relationships) {
      // Find domains that depend on this one
      const dependents = this.domainRegistry.getAllDomains()
        .filter(d => d.config.dependencies.includes(args.name))
        .map(d => d.config.name);

      // Find domains this one analogizes with
      const analogical_connections = this.domainRegistry.getAllDomains()
        .filter(d => d.config.analogy_domains.includes(args.name) ||
                    plugin.config.analogy_domains.includes(d.config.name))
        .map(d => d.config.name);

      result.relationships = {
        dependencies: plugin.config.dependencies,
        dependents,
        analogical_connections: [...new Set(analogical_connections)]
      };
    }

    return result;
  }

  private async updateDomain(args: any): Promise<any> {
    // Validate custom reasoning description if needed
    if (args.updates.reasoning_style === 'custom' && !args.updates.custom_reasoning_description) {
      throw new Error('custom_reasoning_description is required when reasoning_style is "custom"');
    }

    const result = await this.domainRegistry.updateDomain(args.name, args.updates);

    const updatedPlugin = this.domainRegistry.getDomain(args.name);

    return {
      success: true,
      domain_name: args.name,
      updated_at: new Date().toISOString(),
      warnings: result.warnings,
      current_config: updatedPlugin?.config
    };
  }

  private async unregisterDomain(args: any): Promise<any> {
    const result = await this.domainRegistry.unregisterDomain(args.name, {
      force: args.force
    });

    return {
      success: true,
      domain_name: args.name,
      unregistered_at: new Date().toISOString(),
      cleanup_performed: args.cleanup_knowledge,
      system_status: this.domainRegistry.getSystemStatus()
    };
  }

  private enableDomain(args: any): any {
    const result = this.domainRegistry.enableDomain(args.name);

    return {
      success: true,
      domain_name: args.name,
      enabled: true,
      enabled_at: new Date().toISOString()
    };
  }

  private disableDomain(args: any): any {
    const result = this.domainRegistry.disableDomain(args.name);

    return {
      success: true,
      domain_name: args.name,
      enabled: false,
      disabled_at: new Date().toISOString()
    };
  }

  private getSystemStatus(args: any): any {
    const status = this.domainRegistry.getSystemStatus();
    const result: any = {
      ...status,
      timestamp: new Date().toISOString(),
      healthy: true
    };

    if (args.include_integrity_check) {
      const integrity = this.domainRegistry.validateSystemIntegrity();
      result.integrity_check = integrity;
      result.healthy = integrity.valid;
    }

    if (args.include_performance_summary) {
      const domains = this.domainRegistry.getAllDomains();
      const avgSuccessRate = domains.reduce((sum, d) => sum + d.performance_metrics.success_rate, 0) / domains.length;
      const avgResponseTime = domains.reduce((sum, d) => sum + d.performance_metrics.reasoning_time_avg, 0) / domains.length;

      result.performance_summary = {
        average_success_rate: avgSuccessRate,
        average_response_time_ms: avgResponseTime,
        total_usage: domains.reduce((sum, d) => sum + d.usage_count, 0)
      };
    }

    return result;
  }

  // Expose domain registry for other tools
  getDomainRegistry(): DomainRegistry {
    return this.domainRegistry;
  }
}