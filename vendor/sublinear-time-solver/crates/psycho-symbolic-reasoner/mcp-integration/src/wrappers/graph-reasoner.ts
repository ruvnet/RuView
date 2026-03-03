import {
  GraphReasonerWasm,
  GraphReasonerInstance,
  Query,
  QueryResult,
  Fact,
  Rule,
  InferenceResult,
  WasmExecutionError,
  InvalidInputError
} from '../types/index
import { SimpleWasmLoader } from '../wasm/wasm-loader-simple
import { WasmMemoryManager } from '../wasm/memory-manager
import { validateInput, schemas } from '../schemas/index

export class GraphReasonerWrapper {
  private wasmModule: GraphReasonerWasm | null = null;
  private memoryManager: WasmMemoryManager;
  private loader: SimpleWasmLoader;
  private initialized = false;

  constructor() {
    this.memoryManager = WasmMemoryManager.getInstance();
    this.loader = SimpleWasmLoader.getInstance();
  }

  /**
   * Initialize the WASM module
   */
  public async initialize(wasmPath: string): Promise<void> {
    if (this.initialized) {
      return;
    }

    try {
      this.wasmModule = await this.loader.loadGraphReasoner({
        wasmPath,
        initTimeoutMs: 30000,
        memoryInitialPages: 256,
        memoryMaximumPages: 1024
      });
      this.initialized = true;
    } catch (error) {
      throw new WasmExecutionError('Graph reasoner initialization', {
        wasmPath,
        originalError: error
      });
    }
  }

  /**
   * Create a new graph reasoner instance
   */
  public createInstance(instanceId?: string): string {
    this.ensureInitialized();
    
    const result = this.memoryManager.createInstance(
      () => new this.wasmModule!.GraphReasoner(),
      'graph_reasoner',
      instanceId
    );
    
    return result.id;
  }

  /**
   * Get an existing instance
   */
  private getInstance(instanceId: string): GraphReasonerInstance {
    const instance = this.memoryManager.getInstance<GraphReasonerInstance>(instanceId);
    if (!instance) {
      throw new InvalidInputError(
        'instanceId',
        'valid instance ID',
        instanceId
      );
    }
    return instance;
  }

  /**
   * Add a fact to the knowledge graph
   */
  public addFact(
    instanceId: string,
    fact: Fact,
    options: { validate?: boolean; autoInfer?: boolean } = {}
  ): string {
    const { validate = true, autoInfer = false } = options;
    
    if (validate) {
      validateInput(schemas.Fact, fact);
    }

    const instance = this.getInstance(instanceId);
    
    try {
      const result = instance.add_fact(fact.subject, fact.predicate, fact.object);
      
      if (result.startsWith('Error:')) {
        throw new WasmExecutionError('Adding fact', {
          fact,
          error: result
        });
      }
      
      // Optionally trigger inference
      if (autoInfer) {
        this.runInference(instanceId, { maxIterations: 5 });
      }
      
      return result;
    } catch (error) {
      throw new WasmExecutionError('Adding fact', {
        fact,
        originalError: error
      });
    }
  }

  /**
   * Add a rule to the knowledge graph
   */
  public addRule(
    instanceId: string,
    rule: Rule,
    options: { validate?: boolean; replaceExisting?: boolean } = {}
  ): boolean {
    const { validate = true } = options;
    
    if (validate) {
      validateInput(schemas.Rule, rule);
    }

    const instance = this.getInstance(instanceId);
    
    try {
      const ruleJson = JSON.stringify(rule);
      return instance.add_rule(ruleJson);
    } catch (error) {
      throw new WasmExecutionError('Adding rule', {
        rule,
        originalError: error
      });
    }
  }

  /**
   * Query the knowledge graph
   */
  public query(
    instanceId: string,
    query: Query,
    options: {
      format?: 'json' | 'turtle' | 'csv';
      includeMetadata?: boolean;
      validate?: boolean;
    } = {}
  ): QueryResult {
    const { validate = true, format = 'json', includeMetadata = true } = options;
    
    if (validate) {
      validateInput(schemas.Query, query);
    }

    const instance = this.getInstance(instanceId);
    
    try {
      const queryJson = JSON.stringify(query);
      const resultStr = instance.query(queryJson);
      
      let result: QueryResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing query result', {
          query,
          resultStr,
          originalError: parseError
        });
      }
      
      // Add metadata if requested
      if (includeMetadata && result.success) {
        result.metadata = {
          query_id: query.id,
          execution_timestamp: new Date().toISOString(),
          format,
          instance_id: instanceId
        };
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Querying graph', {
        query,
        originalError: error
      });
    }
  }

  /**
   * Run inference on the knowledge graph
   */
  public runInference(
    instanceId: string,
    options: {
      maxIterations?: number;
      confidenceThreshold?: number;
      maxNewFacts?: number;
      timeoutMs?: number;
    } = {}
  ): InferenceResult {
    const {
      maxIterations = 10,
      confidenceThreshold = 0.5,
      maxNewFacts = 1000,
      timeoutMs = 30000
    } = options;

    const instance = this.getInstance(instanceId);
    
    try {
      const resultStr = instance.infer(maxIterations);
      
      let result: InferenceResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing inference result', {
          maxIterations,
          resultStr,
          originalError: parseError
        });
      }
      
      // Filter results by confidence threshold
      if (confidenceThreshold > 0 && result.new_facts) {
        result.new_facts = result.new_facts.filter(
          fact => (fact.confidence || 1.0) >= confidenceThreshold
        );
      }
      
      // Limit number of new facts
      if (maxNewFacts > 0 && result.new_facts && result.new_facts.length > maxNewFacts) {
        result.new_facts = result.new_facts.slice(0, maxNewFacts);
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Running inference', {
        maxIterations,
        originalError: error
      });
    }
  }

  /**
   * Get graph statistics
   */
  public getGraphStats(instanceId: string): any {
    const instance = this.getInstance(instanceId);
    
    try {
      const statsStr = instance.get_graph_stats();
      return JSON.parse(statsStr);
    } catch (error) {
      throw new WasmExecutionError('Getting graph statistics', {
        instanceId,
        originalError: error
      });
    }
  }

  /**
   * Export graph data
   */
  public exportGraph(
    instanceId: string,
    format: 'json' | 'turtle' | 'rdf' = 'json'
  ): string {
    const stats = this.getGraphStats(instanceId);
    
    // For now, return stats as JSON
    // In a full implementation, this would support different export formats
    switch (format) {
      case 'json':
        return JSON.stringify(stats, null, 2);
      case 'turtle':
      case 'rdf':
        // Would implement RDF/Turtle serialization here
        throw new Error(`Export format '${format}' not yet implemented`);
      default:
        throw new InvalidInputError('format', 'json|turtle|rdf', format);
    }
  }

  /**
   * Clear all data from 'an instance
   */
  public clearInstance(instanceId: string): void {';
    // Since WASM doesn't expose a clear method, we recreate the instance
    this.removeInstance(instanceId);
    this.createInstance(instanceId);
  }

  /**
   * Remove an instance
   */
  public removeInstance(instanceId: string): boolean {
    return this.memoryManager.removeInstance(instanceId);
  }

  /**
   * Get all active instance IDs
   */
  public getActiveInstances(): string[] {
    const instances = this.memoryManager.getInstancesByType('graph_reasoner');
    return Array.from(instances.keys());
  }

  /**
   * Bulk operations for better performance
   */
  public addFactsBulk(
    instanceId: string,
    facts: Fact[],
    options: { validate?: boolean; autoInfer?: boolean } = {}
  ): string[] {
    const results: string[] = [];
    
    for (const fact of facts) {
      try {
        const result = this.addFact(instanceId, fact, { ...options, autoInfer: false });
        results.push(result);
      } catch (error) {
        results.push(`Error: ${error.message}`);
      }
    }
    
    // Run inference once at the end if requested
    if (options.autoInfer) {
      this.runInference(instanceId, { maxIterations: 10 });
    }
    
    return results;
  }

  /**
   * Validate that the module is initialized
   */
  private ensureInitialized(): void {
    if (!this.initialized || !this.wasmModule) {
      throw new WasmExecutionError('Module not initialized', {
        hint: 'Call initialize() first'
      });
    }
  }

  /**
   * Get memory usage for this wrapper
   */
  public getMemoryStats(): any {
    const managerStats = this.memoryManager.getMemoryStats();
    const graphReasonerInstances = this.memoryManager.getInstancesByType('graph_reasoner');
    
    return {
      ...managerStats,
      graphReasonerInstances: graphReasonerInstances.size,
      initialized: this.initialized
    };
  }

  /**
   * Cleanup all instances
   */
  public cleanup(): void {
    this.memoryManager.removeInstancesByType('graph_reasoner');
  }
}