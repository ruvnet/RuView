/**
 * Cross-Tool Information Sharing System
 * Enables tools to share insights, intermediate results, and learned patterns
 */

export interface SharedInformation {
  id: string;
  sourceTools: string[];
  targetTools: string[];
  content: any;
  type: 'insight' | 'pattern' | 'result' | 'optimization' | 'failure';
  timestamp: number;
  relevance: number;
  persistence: 'session' | 'permanent' | 'temporary';
  metadata: any;
}

export interface ToolConnection {
  source: string;
  target: string;
  strength: number;
  informationTypes: string[];
  successRate: number;
  lastUsed: number;
}

export interface InformationFlow {
  pathway: string[];
  information: SharedInformation;
  transformations: any[];
  emergentProperties: any[];
}

export class CrossToolSharingSystem {
  private sharedInformation: Map<string, SharedInformation> = new Map();
  private toolConnections: Map<string, ToolConnection[]> = new Map();
  private informationFlows: InformationFlow[] = [];
  private subscriptions: Map<string, string[]> = new Map(); // tool -> information types
  private transformationRules: Map<string, (info: any) => any> = new Map();
  private sharingDepth = 0;
  private maxSharingDepth = 3;

  /**
   * Share information from one tool to potentially interested tools
   */
  async shareInformation(info: SharedInformation): Promise<string[]> {
    // Prevent deep recursion
    if (this.sharingDepth >= this.maxSharingDepth) {
      return [];
    }

    this.sharingDepth++;

    try {
      // Store the information
      this.sharedInformation.set(info.id, info);

      // Find interested tools
      const interestedTools = this.findInterestedTools(info);

      // Propagate information to interested tools
      const propagationResults = [];
      for (const tool of interestedTools) {
        const result = await this.propagateToTool(tool, info);
        propagationResults.push(result);
      }

      // Update connection strengths based on success
      this.updateConnectionStrengths(info.sourceTools, interestedTools, propagationResults);

      // Check for emergent patterns from information combinations
      await this.detectEmergentPatterns(info);

      return interestedTools;
    } finally {
      this.sharingDepth--;
    }
  }

  /**
   * Subscribe a tool to specific types of information
   */
  subscribeToInformation(toolName: string, informationTypes: string[]): void {
    const existing = this.subscriptions.get(toolName) || [];
    const combined = [...new Set([...existing, ...informationTypes])];
    this.subscriptions.set(toolName, combined);
  }

  /**
   * Get relevant information for a tool
   */
  getRelevantInformation(toolName: string, query?: any): SharedInformation[] {
    const subscribedTypes = this.subscriptions.get(toolName) || [];
    const relevantInfo: SharedInformation[] = [];

    for (const [id, info] of this.sharedInformation) {
      // Check if tool is subscribed to this type
      if (subscribedTypes.includes(info.type)) {
        relevantInfo.push(info);
        continue;
      }

      // Check if tool is explicitly targeted
      if (info.targetTools.includes(toolName)) {
        relevantInfo.push(info);
        continue;
      }

      // Check relevance based on query
      if (query && this.calculateQueryRelevance(info, query) > 0.5) {
        relevantInfo.push(info);
      }
    }

    // Sort by relevance and recency
    return relevantInfo.sort((a, b) => {
      const relevanceScore = b.relevance - a.relevance;
      const timeScore = (b.timestamp - a.timestamp) / 1000000; // Normalize time
      return relevanceScore + timeScore * 0.1;
    });
  }

  /**
   * Create dynamic connections between tools based on information flow
   */
  async createDynamicConnection(sourceTool: string, targetTool: string,
                               informationType: string): Promise<boolean> {
    const connectionKey = `${sourceTool}->${targetTool}`;

    const existing = this.toolConnections.get(connectionKey) || [];
    const connection = existing.find(c => c.source === sourceTool && c.target === targetTool);

    if (connection) {
      // Strengthen existing connection
      connection.strength = Math.min(1.0, connection.strength + 0.1);
      if (!connection.informationTypes.includes(informationType)) {
        connection.informationTypes.push(informationType);
      }
      connection.lastUsed = Date.now();
    } else {
      // Create new connection
      const newConnection: ToolConnection = {
        source: sourceTool,
        target: targetTool,
        strength: 0.3,
        informationTypes: [informationType],
        successRate: 0.5,
        lastUsed: Date.now()
      };

      existing.push(newConnection);
      this.toolConnections.set(connectionKey, existing);
    }

    return true;
  }

  /**
   * Register a transformation rule for adapting information between tools
   */
  registerTransformationRule(fromTool: string, toTool: string,
                            transform: (info: any) => any): void {
    const key = `${fromTool}->${toTool}`;
    this.transformationRules.set(key, transform);
  }

  /**
   * Create information cascade across multiple tools
   */
  async createInformationCascade(initialInfo: SharedInformation,
                                targetTools: string[]): Promise<InformationFlow> {
    const flow: InformationFlow = {
      pathway: [],
      information: initialInfo,
      transformations: [],
      emergentProperties: []
    };

    let currentInfo = initialInfo;

    for (const tool of targetTools) {
      flow.pathway.push(tool);

      // Transform information for this tool
      const transformed = await this.transformInformationForTool(currentInfo, tool);
      flow.transformations.push({
        tool,
        input: currentInfo,
        output: transformed,
        timestamp: Date.now()
      });

      // Check for emergent properties
      const emergent = this.detectEmergentProperties(currentInfo, transformed);
      if (emergent.length > 0) {
        flow.emergentProperties.push(...emergent);
      }

      currentInfo = transformed;
    }

    this.informationFlows.push(flow);
    return flow;
  }

  /**
   * Analyze cross-tool collaboration patterns
   */
  analyzeCollaborationPatterns(): any {
    const patterns = {
      mostConnectedTools: this.getMostConnectedTools(),
      strongestConnections: this.getStrongestConnections(),
      informationHubs: this.getInformationHubs(),
      emergentCombinations: this.getEmergentCombinations(),
      collaborationSuccess: this.calculateCollaborationSuccess()
    };

    return patterns;
  }

  /**
   * Optimize information sharing based on historical performance
   */
  optimizeSharing(): void {
    // Remove weak connections
    this.pruneWeakConnections();

    // Strengthen successful pathways
    this.reinforceSuccessfulPathways();

    // Clean old information
    this.cleanupOldInformation();

    // Update subscription recommendations
    this.updateSubscriptionRecommendations();
  }

  /**
   * Find tools that might be interested in given information
   */
  private findInterestedTools(info: SharedInformation): string[] {
    const interested: string[] = [];

    // Check explicit targets
    interested.push(...info.targetTools);

    // Check subscriptions
    for (const [tool, types] of this.subscriptions) {
      if (types.includes(info.type)) {
        interested.push(tool);
      }
    }

    // Check based on connection patterns
    for (const sourceTool of info.sourceTools) {
      const connections = this.toolConnections.get(sourceTool) || [];
      for (const connection of connections) {
        if (connection.strength > 0.5 &&
            connection.informationTypes.includes(info.type)) {
          interested.push(connection.target);
        }
      }
    }

    // Remove duplicates and source tools
    return [...new Set(interested)].filter(tool => !info.sourceTools.includes(tool));
  }

  /**
   * Propagate information to a specific tool
   */
  private async propagateToTool(toolName: string, info: SharedInformation): Promise<boolean> {
    try {
      // Transform information for the target tool
      const transformed = await this.transformInformationForTool(info, toolName);

      // Create new shared information entry
      const propagatedInfo: SharedInformation = {
        id: `${info.id}_propagated_${toolName}_${Date.now()}`,
        sourceTools: [...info.sourceTools, 'sharing_system'],
        targetTools: [toolName],
        content: transformed,
        type: info.type,
        timestamp: Date.now(),
        relevance: info.relevance * 0.8, // Slight relevance decay
        persistence: info.persistence,
        metadata: {
          ...info.metadata,
          propagatedFrom: info.id,
          transformedFor: toolName
        }
      };

      this.sharedInformation.set(propagatedInfo.id, propagatedInfo);
      return true;
    } catch (error) {
      console.error(`Failed to propagate to ${toolName}:`, error);
      return false;
    }
  }

  /**
   * Transform information to be suitable for a specific tool
   */
  private async transformInformationForTool(info: SharedInformation, toolName: string): Promise<any> {
    // Check for registered transformation rule
    for (const sourceTool of info.sourceTools) {
      const transformKey = `${sourceTool}->${toolName}`;
      const transform = this.transformationRules.get(transformKey);

      if (transform) {
        return transform(info.content);
      }
    }

    // Default transformation based on tool type
    return this.defaultTransformation(info.content, toolName);
  }

  /**
   * Default transformation logic
   */
  private defaultTransformation(content: any, toolName: string): any {
    switch (toolName) {
      case 'matrix-solver':
        return this.transformToMatrixFormat(content);
      case 'consciousness':
        return this.transformToConsciousnessFormat(content);
      case 'psycho-symbolic':
        return this.transformToSymbolicFormat(content);
      case 'temporal':
        return this.transformToTemporalFormat(content);
      default:
        return content; // No transformation
    }
  }

  /**
   * Calculate relevance between information and query
   */
  private calculateQueryRelevance(info: SharedInformation, query: any): number {
    // Simple relevance calculation based on content similarity
    const infoStr = JSON.stringify(info.content).toLowerCase();
    const queryStr = JSON.stringify(query).toLowerCase();

    // Check for common keywords
    const infoWords = infoStr.split(/\W+/);
    const queryWords = queryStr.split(/\W+/);

    const commonWords = infoWords.filter(word => queryWords.includes(word));
    const relevance = commonWords.length / Math.max(queryWords.length, 1);

    return Math.min(1.0, relevance);
  }

  /**
   * Update connection strengths based on propagation success
   */
  private updateConnectionStrengths(sourceTools: string[], targetTools: string[],
                                   results: boolean[]): void {
    for (const source of sourceTools) {
      targetTools.forEach((target, index) => {
        const connectionKey = `${source}->${target}`;
        const connections = this.toolConnections.get(connectionKey) || [];
        const connection = connections.find(c => c.source === source && c.target === target);

        if (connection) {
          const success = results[index];
          const updateStrength = success ? 0.1 : -0.05;
          connection.strength = Math.max(0, Math.min(1.0, connection.strength + updateStrength));

          // Update success rate
          const totalAttempts = connection.successRate * 10; // Approximate
          const newSuccessRate = (connection.successRate * totalAttempts + (success ? 1 : 0)) / (totalAttempts + 1);
          connection.successRate = newSuccessRate;
        }
      });
    }
  }

  /**
   * Detect emergent patterns from information combinations
   */
  private async detectEmergentPatterns(newInfo: SharedInformation): Promise<void> {
    // Look for patterns when information from different tools combines
    const recentInfo = Array.from(this.sharedInformation.values())
      .filter(info => Date.now() - info.timestamp < 60000) // Last minute
      .filter(info => info.id !== newInfo.id);

    for (const existing of recentInfo) {
      const emergent = this.detectEmergentProperties(existing, newInfo);

      if (emergent.length > 0) {
        // Create new emergent information
        const emergentInfo: SharedInformation = {
          id: `emergent_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
          sourceTools: [...existing.sourceTools, ...newInfo.sourceTools],
          targetTools: [],
          content: { emergentProperties: emergent, sources: [existing.id, newInfo.id] },
          type: 'pattern',
          timestamp: Date.now(),
          relevance: 0.8,
          persistence: 'session',
          metadata: { emergent: true, sourceCount: 2 }
        };

        await this.shareInformation(emergentInfo);
      }
    }
  }

  /**
   * Detect emergent properties from two pieces of information
   */
  private detectEmergentProperties(info1: SharedInformation, info2: SharedInformation): any[] {
    const emergent = [];

    // Check for complementary patterns
    if (this.areComplementary(info1.content, info2.content)) {
      emergent.push({
        type: 'complementary_pattern',
        description: 'Information pieces complement each other',
        synergy: this.calculateSynergy(info1.content, info2.content)
      });
    }

    // Check for amplification effects
    if (this.checkAmplification(info1.content, info2.content)) {
      emergent.push({
        type: 'amplification',
        description: 'Information pieces amplify each other',
        amplification_factor: this.calculateAmplificationFactor(info1.content, info2.content)
      });
    }

    // Check for novel combinations
    const novelCombination = this.generateNovelCombination(info1.content, info2.content);
    if (novelCombination) {
      emergent.push({
        type: 'novel_combination',
        description: 'Unexpected combination creates new insight',
        combination: novelCombination
      });
    }

    return emergent;
  }

  // Transformation methods for different tool types
  private transformToMatrixFormat(content: any): any {
    if (Array.isArray(content)) {
      return { matrix: content, format: 'dense' };
    }
    return { scalar: content };
  }

  private transformToConsciousnessFormat(content: any): any {
    return {
      emergenceLevel: this.extractEmergenceLevel(content),
      integrationData: content,
      timestamp: Date.now()
    };
  }

  private transformToSymbolicFormat(content: any): any {
    return {
      symbols: this.extractSymbols(content),
      relations: this.extractRelations(content),
      domain: 'cross_tool_sharing'
    };
  }

  private transformToTemporalFormat(content: any): any {
    return {
      temporalData: content,
      timestamp: Date.now(),
      sequence: this.extractSequence(content)
    };
  }

  // Analysis methods
  private getMostConnectedTools(): any[] {
    const toolCounts = new Map<string, number>();

    for (const connections of this.toolConnections.values()) {
      for (const connection of connections) {
        toolCounts.set(connection.source, (toolCounts.get(connection.source) || 0) + 1);
        toolCounts.set(connection.target, (toolCounts.get(connection.target) || 0) + 1);
      }
    }

    return Array.from(toolCounts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5);
  }

  private getStrongestConnections(): ToolConnection[] {
    const allConnections: ToolConnection[] = [];

    for (const connections of this.toolConnections.values()) {
      allConnections.push(...connections);
    }

    return allConnections
      .sort((a, b) => b.strength - a.strength)
      .slice(0, 10);
  }

  private getInformationHubs(): string[] {
    const hubScores = new Map<string, number>();

    for (const info of this.sharedInformation.values()) {
      for (const source of info.sourceTools) {
        hubScores.set(source, (hubScores.get(source) || 0) + 1);
      }
      for (const target of info.targetTools) {
        hubScores.set(target, (hubScores.get(target) || 0) + 0.5);
      }
    }

    return Array.from(hubScores.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(entry => entry[0]);
  }

  private getEmergentCombinations(): any[] {
    return this.informationFlows
      .filter(flow => flow.emergentProperties.length > 0)
      .map(flow => ({
        pathway: flow.pathway,
        emergentCount: flow.emergentProperties.length,
        properties: flow.emergentProperties
      }));
  }

  private calculateCollaborationSuccess(): number {
    const allConnections: ToolConnection[] = [];

    for (const connections of this.toolConnections.values()) {
      allConnections.push(...connections);
    }

    if (allConnections.length === 0) return 0;

    const avgSuccessRate = allConnections.reduce((sum, conn) => sum + conn.successRate, 0) / allConnections.length;
    return avgSuccessRate;
  }

  // Optimization methods
  private pruneWeakConnections(): void {
    for (const [key, connections] of this.toolConnections) {
      const strongConnections = connections.filter(conn => conn.strength > 0.2);
      if (strongConnections.length !== connections.length) {
        this.toolConnections.set(key, strongConnections);
      }
    }
  }

  private reinforceSuccessfulPathways(): void {
    for (const flow of this.informationFlows) {
      if (flow.emergentProperties.length > 0) {
        // Strengthen connections in successful pathways
        for (let i = 0; i < flow.pathway.length - 1; i++) {
          const source = flow.pathway[i];
          const target = flow.pathway[i + 1];
          this.createDynamicConnection(source, target, 'pattern');
        }
      }
    }
  }

  private cleanupOldInformation(): void {
    const oneHour = 60 * 60 * 1000;
    const now = Date.now();

    for (const [id, info] of this.sharedInformation) {
      if (info.persistence === 'temporary' && now - info.timestamp > oneHour) {
        this.sharedInformation.delete(id);
      }
    }
  }

  private updateSubscriptionRecommendations(): void {
    // Analyze successful information sharing and recommend new subscriptions
    // This would be implemented based on analysis of collaboration patterns
  }

  // Utility methods for pattern detection
  private areComplementary(content1: any, content2: any): boolean {
    // Check if two pieces of content complement each other
    // This is a simplified implementation
    return JSON.stringify(content1) !== JSON.stringify(content2);
  }

  private checkAmplification(content1: any, content2: any): boolean {
    // Check if combination amplifies the effect
    return true; // Simplified
  }

  private calculateSynergy(content1: any, content2: any): number {
    return Math.random() * 0.5 + 0.5; // Simplified
  }

  private calculateAmplificationFactor(content1: any, content2: any): number {
    return Math.random() * 2 + 1; // Simplified
  }

  private generateNovelCombination(content1: any, content2: any): any {
    return {
      combined: true,
      elements: [content1, content2],
      novelty: Math.random()
    };
  }

  private extractEmergenceLevel(content: any): number {
    return Math.random() * 0.5 + 0.5; // Simplified
  }

  private extractSymbols(content: any): string[] {
    return ['symbol1', 'symbol2']; // Simplified
  }

  private extractRelations(content: any): any[] {
    return []; // Simplified
  }

  private extractSequence(content: any): any[] {
    return []; // Simplified
  }

  /**
   * Get sharing system statistics
   */
  getStats(): any {
    return {
      totalSharedInformation: this.sharedInformation.size,
      totalConnections: Array.from(this.toolConnections.values()).reduce((sum, arr) => sum + arr.length, 0),
      totalFlows: this.informationFlows.length,
      averageConnectionStrength: this.calculateAverageConnectionStrength(),
      emergentPatternsDetected: this.countEmergentPatterns(),
      mostActiveTools: this.getMostConnectedTools().slice(0, 3)
    };
  }

  private calculateAverageConnectionStrength(): number {
    const allConnections: ToolConnection[] = [];
    for (const connections of this.toolConnections.values()) {
      allConnections.push(...connections);
    }

    if (allConnections.length === 0) return 0;
    return allConnections.reduce((sum, conn) => sum + conn.strength, 0) / allConnections.length;
  }

  private countEmergentPatterns(): number {
    return this.informationFlows.reduce((sum, flow) => sum + flow.emergentProperties.length, 0);
  }
}