/**
 * Agentic loop implementation for autonomous decision-making
 */

import {
  Action,
  Observation,
  Plan,
  Goal,
  Context,
} from './types';

export class AgenticLoop {
  private actionHistory: Action[] = [];
  private totalReward: number = 0;
  private actionCount: number = 0;

  /**
   * Plan phase: Generate a plan based on goals and context
   */
  async plan(context: Context, input: string): Promise<Plan> {
    const goal: Goal = {
      id: `goal_${this.actionCount}`,
      description: `Process: ${input}`,
      priority: 1.0,
      achieved: false,
    };

    const actions = await this.generateActionCandidates(input, context);
    const rankedActions = this.rankActions(actions, context);

    const steps = rankedActions.slice(0, 5).map((action, i) => ({
      sequence: i,
      action,
      preconditions: [],
      postconditions: [],
    }));

    return {
      goal,
      steps,
      estimatedReward: rankedActions[0]?.expectedReward || 0,
      confidence: steps.length > 0 ? 0.8 : 0.0,
    };
  }

  /**
   * Generate candidate actions based on input
   */
  private async generateActionCandidates(
    input: string,
    context: Context
  ): Promise<Action[]> {
    const candidates: Action[] = [];
    const inputLower = input.toLowerCase();

    if (inputLower.includes('weather')) {
      candidates.push({
        actionType: 'get_weather',
        description: 'Fetch weather information',
        parameters: { query: input },
        toolCalls: ['weather_api'],
        expectedOutcome: 'Weather data',
        expectedReward: 0.8,
      });
    }

    if (inputLower.includes('learn') || inputLower.includes('remember')) {
      candidates.push({
        actionType: 'update_knowledge',
        description: 'Update knowledge graph',
        parameters: { content: input },
        toolCalls: [],
        expectedOutcome: 'Knowledge updated',
        expectedReward: 0.9,
      });
    }

    // Default action
    candidates.push({
      actionType: 'process_text',
      description: `Process: ${input}`,
      parameters: { text: input },
      toolCalls: [],
      expectedOutcome: 'Processed text',
      expectedReward: 0.5,
    });

    return candidates;
  }

  /**
   * Rank actions by expected reward
   */
  private rankActions(actions: Action[], context: Context): Action[] {
    return actions.sort((a, b) => b.expectedReward - a.expectedReward);
  }

  /**
   * Record action execution
   */
  recordAction(action: Action, reward: number): void {
    this.actionHistory.push(action);
    this.totalReward += reward;
    this.actionCount++;
  }

  /**
   * Get average reward
   */
  getAverageReward(): number {
    return this.actionCount > 0 ? this.totalReward / this.actionCount : 0;
  }

  /**
   * Get action count
   */
  getActionCount(): number {
    return this.actionCount;
  }
}
