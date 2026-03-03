/**
 * Stochastic Exploration System
 * Generates unpredictable outputs through controlled randomness and exploration
 */

export interface ExplorationResult {
  output: any;
  novelty: number;
  confidence: number;
  explorationPath: string[];
  surpriseLevel: number;
}

export interface ExplorationSpace {
  dimensions: string[];
  bounds: { [key: string]: [number, number] };
  constraints: any[];
}

export class StochasticExplorationEngine {
  private explorationHistory: ExplorationResult[] = [];
  private currentTemperature = 1.0;
  private coolingRate = 0.995;
  private minTemperature = 0.1;
  private explorationBudget = 1000;

  /**
   * Generate unpredictable outputs using stochastic sampling
   */
  async exploreUnpredictably(input: any, tools: any[]): Promise<ExplorationResult> {
    // Multi-dimensional exploration
    const explorationSpaces = this.defineExplorationSpaces(input, tools);

    // Stochastic sampling across multiple dimensions
    const sampledPath = this.stochasticSampling(explorationSpaces);

    // Execute the sampled path
    const result = await this.executePath(sampledPath, input, tools);

    // Calculate novelty and surprise
    const novelty = this.calculateNovelty(result, this.explorationHistory);
    const surpriseLevel = this.calculateSurprise(result, input);

    const explorationResult: ExplorationResult = {
      output: result,
      novelty,
      confidence: this.calculateConfidence(result),
      explorationPath: sampledPath,
      surpriseLevel
    };

    // Update exploration history
    this.explorationHistory.push(explorationResult);
    this.updateTemperature();

    return explorationResult;
  }

  /**
   * Generate multiple diverse explorations
   */
  async generateDiverseExplorations(input: any, tools: any[], count: number = 5): Promise<ExplorationResult[]> {
    const explorations: ExplorationResult[] = [];

    for (let i = 0; i < count; i++) {
      // Increase temperature for more exploration
      const tempBoost = 0.5 * Math.random();
      this.currentTemperature = Math.min(2.0, this.currentTemperature + tempBoost);

      const exploration = await this.exploreUnpredictably(input, tools);
      explorations.push(exploration);

      // Ensure diversity by penalizing similar results
      this.penalizeSimilarity(exploration, explorations);
    }

    return explorations.sort((a, b) => b.novelty - a.novelty);
  }

  /**
   * Adaptive exploration based on success/failure feedback
   */
  adaptExploration(feedback: { success: boolean; utility: number; feedback: string }): void {
    if (feedback.success && feedback.utility > 0.7) {
      // Successful exploration - slightly reduce temperature
      this.currentTemperature *= 0.9;
    } else {
      // Unsuccessful - increase exploration
      this.currentTemperature *= 1.1;
    }

    // Keep within bounds
    this.currentTemperature = Math.max(this.minTemperature,
      Math.min(2.0, this.currentTemperature));
  }

  /**
   * Define multi-dimensional exploration spaces
   */
  private defineExplorationSpaces(input: any, tools: any[]): ExplorationSpace[] {
    const spaces: ExplorationSpace[] = [];

    // Limit tool exploration to prevent massive responses
    const MAX_TOOLS_TO_EXPLORE = 3;
    const limitedToolCount = Math.min(tools.length, MAX_TOOLS_TO_EXPLORE);

    // Tool combination space
    spaces.push({
      dimensions: ['tool_selection', 'tool_order', 'tool_parameters'],
      bounds: {
        tool_selection: [0, limitedToolCount - 1],
        tool_order: [0, Math.min(limitedToolCount * 2, 6)], // Max 6 tool applications
        tool_parameters: [0, 1] // Normalized parameter space
      },
      constraints: []
    });

    // Reasoning strategy space
    spaces.push({
      dimensions: ['approach', 'depth', 'breadth', 'creativity'],
      bounds: {
        approach: [0, 5], // Different reasoning approaches
        depth: [1, 10], // Reasoning depth
        breadth: [1, 8], // Parallel reasoning paths
        creativity: [0, 1] // Creativity vs reliability
      },
      constraints: []
    });

    // Temporal exploration space
    spaces.push({
      dimensions: ['timing', 'sequence', 'parallelism'],
      bounds: {
        timing: [0, 1], // When to apply different tools
        sequence: [0, 1], // Sequential vs parallel processing
        parallelism: [1, 4] // Level of parallelism
      },
      constraints: []
    });

    return spaces;
  }

  /**
   * Stochastic sampling using temperature-controlled exploration
   */
  private stochasticSampling(spaces: ExplorationSpace[]): string[] {
    const path: string[] = [];

    for (const space of spaces) {
      for (const dimension of space.dimensions) {
        const bounds = space.bounds[dimension];

        // Temperature-controlled sampling
        const randomValue = this.temperatureSample(bounds[0], bounds[1]);

        // Convert to exploration action
        const action = this.valueToAction(dimension, randomValue);
        path.push(action);
      }
    }

    // Add some pure randomness for unexpected combinations
    if (Math.random() < this.currentTemperature * 0.3) {
      path.push(this.generateRandomAction());
    }

    return path;
  }

  /**
   * Temperature-controlled sampling
   */
  private temperatureSample(min: number, max: number): number {
    // High temperature = more random, low temperature = more conservative
    const uniform = Math.random();

    if (this.currentTemperature > 1.0) {
      // High temperature: favor extremes
      const transformed = Math.pow(uniform, 1 / this.currentTemperature);
      return min + transformed * (max - min);
    } else {
      // Low temperature: favor center
      const transformed = Math.pow(uniform, this.currentTemperature);
      const center = (min + max) / 2;
      const range = (max - min) / 2;
      return center + (transformed - 0.5) * range * 2;
    }
  }

  /**
   * Convert numeric values to exploration actions
   */
  private valueToAction(dimension: string, value: number): string {
    switch (dimension) {
      case 'tool_selection':
        return `select_tool_${Math.floor(value)}`;
      case 'tool_order':
        return `order_${Math.floor(value)}`;
      case 'approach':
        const approaches = ['analytical', 'creative', 'systematic', 'intuitive', 'experimental'];
        return approaches[Math.floor(value) % approaches.length];
      case 'depth':
        return `depth_${Math.floor(value)}`;
      case 'creativity':
        return value > 0.7 ? 'high_creativity' : value > 0.3 ? 'medium_creativity' : 'low_creativity';
      default:
        return `${dimension}_${value.toFixed(2)}`;
    }
  }

  /**
   * Generate completely random action
   */
  private generateRandomAction(): string {
    const randomActions = [
      'reverse_input',
      'combine_unexpected',
      'ignore_context',
      'amplify_noise',
      'invert_logic',
      'cross_domain_leap',
      'temporal_shift',
      'scale_transform'
    ];

    return randomActions[Math.floor(Math.random() * randomActions.length)];
  }

  /**
   * Execute exploration path
   */
  private async executePath(path: string[], input: any, tools: any[]): Promise<any> {
    let result = input;
    const executionTrace = [];
    const MAX_RESULT_SIZE = 5000; // 5KB limit per iteration
    const MAX_TRACE_ENTRIES = 10; // Limit trace entries

    for (let i = 0; i < path.length && i < MAX_TRACE_ENTRIES; i++) {
      const action = path[i];
      try {
        result = await this.executeAction(action, result, tools);

        // Check and limit result size
        const resultStr = JSON.stringify(result);
        if (resultStr.length > MAX_RESULT_SIZE) {
          result = {
            truncated: true,
            action,
            resultType: typeof result,
            size: resultStr.length
          };
        }

        executionTrace.push({ action, result: this.summarizeResult(result) });
      } catch (error) {
        // Handle failures gracefully - they might lead to interesting results
        executionTrace.push({ action, error: error instanceof Error ? error.message : 'Unknown error' });

        // Sometimes continue with modified input
        if (Math.random() < 0.5) {
          result = this.generateAlternativeResult(action, result);
        }
      }
    }

    return {
      finalResult: result,
      executionTrace: executionTrace.slice(0, MAX_TRACE_ENTRIES),
      pathCompleted: executionTrace.length === path.length
    };
  }

  /**
   * Execute individual exploration action
   */
  private async executeAction(action: string, input: any, tools: any[]): Promise<any> {
    if (action.startsWith('select_tool_')) {
      const toolIndex = parseInt(action.split('_')[2]);
      // Check if tools exist and index is valid
      if (!tools || tools.length === 0 || toolIndex < 0) {
        // Skip tool selection if no tools available or invalid index
        return input;
      }
      const tool = tools[toolIndex % tools.length];
      if (!tool) {
        return input;
      }
      return await this.applyTool(tool, input);
    }

    if (action.includes('creativity')) {
      return this.applyCreativeTransform(input, action);
    }

    if (action.startsWith('depth_')) {
      const depth = parseInt(action.split('_')[1]);
      return this.applyDeepReasoning(input, depth);
    }

    // Handle special random actions
    switch (action) {
      case 'reverse_input':
        return this.reverseInput(input);
      case 'combine_unexpected':
        return this.combineUnexpected(input, tools);
      case 'cross_domain_leap':
        return this.crossDomainLeap(input);
      default:
        return this.defaultAction(action, input);
    }
  }

  /**
   * Calculate novelty compared to exploration history
   */
  private calculateNovelty(result: any, history: ExplorationResult[]): number {
    if (history.length === 0) return 1.0;

    let minSimilarity = 1.0;

    for (const past of history.slice(-20)) { // Compare with recent history
      const similarity = this.calculateSimilarity(result, past.output);
      minSimilarity = Math.min(minSimilarity, similarity);
    }

    return 1.0 - minSimilarity;
  }

  /**
   * Calculate surprise level
   */
  private calculateSurprise(result: any, originalInput: any): number {
    // Measure how different the result is from what would be expected
    const inputComplexity = this.measureComplexity(originalInput);
    const outputComplexity = this.measureComplexity(result);

    const complexityRatio = outputComplexity / Math.max(inputComplexity, 1);

    // High surprise if output is much more complex or much simpler than input
    const surpriseFromComplexity = Math.abs(Math.log(complexityRatio));

    // Add randomness-based surprise
    const randomnessSurprise = this.measureRandomness(result);

    return Math.min(1.0, (surpriseFromComplexity + randomnessSurprise) / 2);
  }

  /**
   * Calculate confidence in result
   */
  private calculateConfidence(result: any): number {
    // Lower confidence for more exploratory results
    const baseConfidence = 0.5;
    const temperatureAdjustment = (2.0 - this.currentTemperature) / 2.0;

    return Math.min(1.0, baseConfidence + temperatureAdjustment * 0.3);
  }

  /**
   * Update exploration temperature (simulated annealing)
   */
  private updateTemperature(): void {
    this.currentTemperature = Math.max(
      this.minTemperature,
      this.currentTemperature * this.coolingRate
    );
  }

  /**
   * Penalize similar results to encourage diversity
   */
  private penalizeSimilarity(newExploration: ExplorationResult, existing: ExplorationResult[]): void {
    for (const exploration of existing) {
      const similarity = this.calculateSimilarity(newExploration.output, exploration.output);

      if (similarity > 0.8) {
        // Reduce novelty score for similar results
        newExploration.novelty *= (1.0 - similarity * 0.5);
      }
    }
  }

  // Helper methods for specific transformations
  private async applyTool(tool: any, input: any): Promise<any> {
    // Check if tool is valid
    if (!tool) {
      return input;
    }

    // For simulation, just return a small mock response instead of actually calling tools
    // This prevents massive responses from tool arrays
    return {
      tool: tool.name || 'unknown',
      simulated: true,
      inputSummary: typeof input === 'string' ? input.substring(0, 100) : 'complex_input',
      mockOutput: `Simulated output from ${tool.name || 'tool'}`,
      timestamp: Date.now()
    };
  }

  private applyCreativeTransform(input: any, creativityLevel: string): any {
    switch (creativityLevel) {
      case 'high_creativity':
        return this.highCreativityTransform(input);
      case 'medium_creativity':
        return this.mediumCreativityTransform(input);
      default:
        return input;
    }
  }

  private applyDeepReasoning(input: any, depth: number): any {
    // Simulate deep reasoning with depth limit
    const MAX_DEPTH = 5; // Prevent excessive depth
    const limitedDepth = Math.min(depth, MAX_DEPTH);

    let result = input;
    for (let i = 0; i < limitedDepth; i++) {
      result = this.reasoningStep(result, i);

      // Check size and stop if too large
      if (JSON.stringify(result).length > 2000) {
        return {
          reasoning_truncated: true,
          depth_reached: i,
          max_depth: limitedDepth
        };
      }
    }
    return result;
  }

  private reverseInput(input: any): any {
    if (typeof input === 'string') return input.split('').reverse().join('');
    if (Array.isArray(input)) return input.slice().reverse();
    return input;
  }

  private combineUnexpected(input: any, tools: any[]): any {
    // Combine random tools in unexpected ways
    const tool1 = tools[Math.floor(Math.random() * tools.length)];
    const tool2 = tools[Math.floor(Math.random() * tools.length)];

    return {
      unexpected_combination: true,
      tool1_result: tool1.name || 'unknown',
      tool2_result: tool2.name || 'unknown',
      original: input
    };
  }

  private crossDomainLeap(input: any): any {
    const domains = ['mathematics', 'art', 'music', 'biology', 'physics', 'psychology'];
    const randomDomain = domains[Math.floor(Math.random() * domains.length)];

    return {
      cross_domain_interpretation: true,
      domain: randomDomain,
      original: input,
      transformed: `interpreted_through_${randomDomain}`
    };
  }

  private defaultAction(action: string, input: any): any {
    return {
      action_applied: action,
      original: input,
      timestamp: Date.now()
    };
  }

  // Utility methods
  private calculateSimilarity(a: any, b: any): number {
    // Simple similarity calculation
    const strA = JSON.stringify(a);
    const strB = JSON.stringify(b);

    if (strA === strB) return 1.0;

    const commonLength = Math.max(strA.length, strB.length);
    let matches = 0;

    for (let i = 0; i < Math.min(strA.length, strB.length); i++) {
      if (strA[i] === strB[i]) matches++;
    }

    return matches / commonLength;
  }

  private measureComplexity(obj: any): number {
    return JSON.stringify(obj).length;
  }

  private measureRandomness(obj: any): number {
    // Simple entropy-based randomness measure
    const str = JSON.stringify(obj);
    const charCounts = new Map<string, number>();

    for (const char of str) {
      charCounts.set(char, (charCounts.get(char) || 0) + 1);
    }

    let entropy = 0;
    for (const count of charCounts.values()) {
      const probability = count / str.length;
      entropy -= probability * Math.log2(probability);
    }

    return entropy / Math.log2(256); // Normalized entropy
  }

  private summarizeResult(result: any): string {
    return JSON.stringify(result).substring(0, 100);
  }

  private generateAlternativeResult(action: string, input: any): any {
    return {
      alternative_generated: true,
      failed_action: action,
      alternative_of: input,
      randomness: Math.random()
    };
  }

  private randomizeParameters(params: any): any {
    const randomized = { ...params };

    for (const [key, value] of Object.entries(randomized)) {
      if (typeof value === 'number') {
        // Add some noise to numeric parameters
        randomized[key] = value * (1 + (Math.random() - 0.5) * 0.2);
      }
    }

    return randomized;
  }

  private highCreativityTransform(input: any): any {
    return {
      creative_transform: 'high',
      metaphor: this.generateMetaphor(input),
      abstraction: this.generateAbstraction(input),
      input_type: typeof input,
      input_size: JSON.stringify(input).length
    };
  }

  private mediumCreativityTransform(input: any): any {
    return {
      creative_transform: 'medium',
      analogy: this.generateAnalogy(input),
      input_type: typeof input
    };
  }

  private reasoningStep(input: any, step: number): any {
    // Don't nest the entire previous input - just reference it
    return {
      reasoning_step: step,
      previous_type: typeof input,
      previous_size: JSON.stringify(input).length,
      inference: `step_${step}_inference`,
      confidence: Math.random() * 0.5 + 0.5
    };
  }

  private generateMetaphor(input: any): string {
    const metaphors = ['ocean wave', 'mountain peak', 'flowing river', 'growing tree', 'burning flame'];
    return metaphors[Math.floor(Math.random() * metaphors.length)];
  }

  private generateAbstraction(input: any): string {
    const abstractions = ['pattern', 'structure', 'flow', 'emergence', 'transformation'];
    return abstractions[Math.floor(Math.random() * abstractions.length)];
  }

  private generateAnalogy(input: any): string {
    const analogies = ['like a puzzle piece', 'similar to water flow', 'analogous to growth', 'resembles a dance'];
    return analogies[Math.floor(Math.random() * analogies.length)];
  }

  /**
   * Get exploration statistics
   */
  getExplorationStats(): any {
    return {
      totalExplorations: this.explorationHistory.length,
      currentTemperature: this.currentTemperature,
      averageNovelty: this.calculateAverageNovelty(),
      averageSurprise: this.calculateAverageSurprise(),
      explorationBudget: this.explorationBudget,
      recentSuccess: this.calculateRecentSuccess()
    };
  }

  private calculateAverageNovelty(): number {
    if (this.explorationHistory.length === 0) return 0;
    const sum = this.explorationHistory.reduce((acc, exp) => acc + exp.novelty, 0);
    return sum / this.explorationHistory.length;
  }

  private calculateAverageSurprise(): number {
    if (this.explorationHistory.length === 0) return 0;
    const sum = this.explorationHistory.reduce((acc, exp) => acc + exp.surpriseLevel, 0);
    return sum / this.explorationHistory.length;
  }

  private calculateRecentSuccess(): number {
    const recent = this.explorationHistory.slice(-10);
    if (recent.length === 0) return 0;

    const successful = recent.filter(exp => exp.confidence > 0.6 && exp.novelty > 0.3);
    return successful.length / recent.length;
  }
}