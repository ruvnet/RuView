/**
 * Model routing logic for Gemini and OpenRouter
 */

import { ModelProvider, ModelRoute, SynthError } from '../types.js';

export interface RouterConfig {
  defaultProvider: ModelProvider;
  providerKeys: {
    gemini?: string;
    openrouter?: string;
  };
  fallbackChain?: ModelProvider[];
  customRoutes?: ModelRoute[];
}

/**
 * Model router for intelligent provider selection
 */
export class ModelRouter {
  private config: RouterConfig;
  private routes: Map<string, ModelRoute>;

  constructor(config: RouterConfig) {
    this.config = config;
    this.routes = new Map();
    this.initializeRoutes();
  }

  private initializeRoutes(): void {
    // Default Gemini models
    const geminiRoutes: ModelRoute[] = [
      {
        provider: 'gemini',
        model: 'gemini-2.0-flash-exp',
        priority: 10,
        capabilities: ['text', 'json', 'streaming', 'fast']
      },
      {
        provider: 'gemini',
        model: 'gemini-1.5-pro',
        priority: 8,
        capabilities: ['text', 'json', 'complex', 'reasoning']
      },
      {
        provider: 'gemini',
        model: 'gemini-1.5-flash',
        priority: 9,
        capabilities: ['text', 'json', 'fast', 'efficient']
      }
    ];

    // Default OpenRouter models
    const openrouterRoutes: ModelRoute[] = [
      {
        provider: 'openrouter',
        model: 'anthropic/claude-3.5-sonnet',
        priority: 10,
        capabilities: ['text', 'json', 'reasoning', 'complex']
      },
      {
        provider: 'openrouter',
        model: 'openai/gpt-4-turbo',
        priority: 9,
        capabilities: ['text', 'json', 'reasoning']
      },
      {
        provider: 'openrouter',
        model: 'meta-llama/llama-3.1-70b-instruct',
        priority: 7,
        capabilities: ['text', 'json', 'fast']
      }
    ];

    // Add all routes
    [...geminiRoutes, ...openrouterRoutes, ...(this.config.customRoutes || [])].forEach(
      route => {
        const key = `${route.provider}:${route.model}`;
        this.routes.set(key, route);
      }
    );
  }

  /**
   * Select best model for given requirements
   */
  selectModel(requirements: {
    capabilities?: string[];
    provider?: ModelProvider;
    preferredModel?: string;
  }): ModelRoute {
    const { capabilities = [], provider, preferredModel } = requirements;

    // If specific model requested, try to use it
    if (provider && preferredModel) {
      const key = `${provider}:${preferredModel}`;
      const route = this.routes.get(key);
      if (route) {
        return route;
      }
    }

    // Filter by provider if specified
    let candidates = Array.from(this.routes.values());
    if (provider) {
      candidates = candidates.filter(r => r.provider === provider);
    } else {
      // Use default provider
      candidates = candidates.filter(r => r.provider === this.config.defaultProvider);
    }

    // Filter by capabilities
    if (capabilities.length > 0) {
      candidates = candidates.filter(route =>
        capabilities.every(cap => route.capabilities.includes(cap))
      );
    }

    // Sort by priority (highest first)
    candidates.sort((a, b) => b.priority - a.priority);

    if (candidates.length === 0) {
      throw new SynthError(
        'No suitable model found for requirements',
        'NO_MODEL_FOUND',
        { requirements }
      );
    }

    // Safe to access: we've checked length > 0
    const selectedRoute = candidates[0];
    if (!selectedRoute) {
      throw new SynthError(
        'Unexpected error: no route selected despite candidates',
        'ROUTE_SELECTION_ERROR',
        { candidates }
      );
    }

    return selectedRoute;
  }

  /**
   * Get fallback chain for resilience
   */
  getFallbackChain(primary: ModelRoute): ModelRoute[] {
    const chain: ModelRoute[] = [primary];

    if (this.config.fallbackChain) {
      // Only require essential capabilities for fallback models
      // Filter out optimization flags like 'streaming', 'fast', 'efficient'
      const essentialCapabilities = primary.capabilities.filter(
        cap => !['streaming', 'fast', 'efficient', 'complex', 'reasoning'].includes(cap)
      );

      for (const provider of this.config.fallbackChain) {
        try {
          const fallback = this.selectModel({
            provider,
            capabilities: essentialCapabilities.length > 0 ? essentialCapabilities : undefined
          });

          if (fallback.model !== primary.model) {
            chain.push(fallback);
          }
        } catch (error) {
          // Skip this fallback provider if no suitable model found
          console.warn(`No suitable fallback model found for provider ${provider}`);
        }
      }
    }

    return chain;
  }

  /**
   * Get all available routes
   */
  getRoutes(): ModelRoute[] {
    return Array.from(this.routes.values());
  }

  /**
   * Add custom route
   */
  addRoute(route: ModelRoute): void {
    const key = `${route.provider}:${route.model}`;
    this.routes.set(key, route);
  }

  /**
   * Get model configuration
   */
  getModelConfig(route: ModelRoute): {
    provider: ModelProvider;
    model: string;
    apiKey?: string;
  } {
    return {
      provider: route.provider,
      model: route.model,
      apiKey: this.config.providerKeys[route.provider]
    };
  }
}

export { ModelProvider, ModelRoute };
