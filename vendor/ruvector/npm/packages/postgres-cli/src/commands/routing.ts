/**
 * Routing/Agent Commands
 * CLI commands for Tiny Dancer agent routing and management
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import type { RuVectorClient } from '../client.js';

export interface RegisterAgentOptions {
  name: string;
  type: string;
  capabilities: string;
  cost: string;
  latency: string;
  quality: string;
}

export interface RegisterAgentFullOptions {
  config: string;
}

export interface UpdateMetricsOptions {
  name: string;
  latency: string;
  success: boolean;
  quality?: string;
}

export interface RouteOptions {
  embedding: string;
  optimizeFor?: string;
  constraints?: string;
}

export interface FindAgentsOptions {
  capability: string;
  limit?: string;
}

export class RoutingCommands {
  static async registerAgent(
    client: RuVectorClient,
    options: RegisterAgentOptions
  ): Promise<void> {
    const spinner = ora(`Registering agent '${options.name}'...`).start();

    try {
      await client.connect();

      const capabilities = options.capabilities.split(',').map(c => c.trim());

      await client.registerAgent(
        options.name,
        options.type,
        capabilities,
        parseFloat(options.cost),
        parseFloat(options.latency),
        parseFloat(options.quality)
      );

      spinner.succeed(chalk.green(`Agent '${options.name}' registered successfully`));

      console.log(chalk.bold.blue('\nAgent Details:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Name:')} ${options.name}`);
      console.log(`  ${chalk.green('Type:')} ${options.type}`);
      console.log(`  ${chalk.green('Capabilities:')} ${capabilities.join(', ')}`);
      console.log(`  ${chalk.green('Cost/Request:')} $${options.cost}`);
      console.log(`  ${chalk.green('Avg Latency:')} ${options.latency}ms`);
      console.log(`  ${chalk.green('Quality Score:')} ${options.quality}`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to register agent'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async registerAgentFull(
    client: RuVectorClient,
    options: RegisterAgentFullOptions
  ): Promise<void> {
    const spinner = ora('Registering agent with full config...').start();

    try {
      await client.connect();

      const config = JSON.parse(options.config);
      await client.registerAgentFull(config);

      spinner.succeed(chalk.green(`Agent '${config.name}' registered successfully`));
    } catch (err) {
      spinner.fail(chalk.red('Failed to register agent'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async updateMetrics(
    client: RuVectorClient,
    options: UpdateMetricsOptions
  ): Promise<void> {
    const spinner = ora(`Updating metrics for '${options.name}'...`).start();

    try {
      await client.connect();

      await client.updateAgentMetrics(
        options.name,
        parseFloat(options.latency),
        options.success,
        options.quality ? parseFloat(options.quality) : undefined
      );

      spinner.succeed(chalk.green('Metrics updated'));

      console.log(`  ${chalk.green('Latency:')} ${options.latency}ms`);
      console.log(`  ${chalk.green('Success:')} ${options.success}`);
      if (options.quality) {
        console.log(`  ${chalk.green('Quality:')} ${options.quality}`);
      }
    } catch (err) {
      spinner.fail(chalk.red('Failed to update metrics'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async removeAgent(
    client: RuVectorClient,
    name: string
  ): Promise<void> {
    const spinner = ora(`Removing agent '${name}'...`).start();

    try {
      await client.connect();
      await client.removeAgent(name);
      spinner.succeed(chalk.green(`Agent '${name}' removed`));
    } catch (err) {
      spinner.fail(chalk.red('Failed to remove agent'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async setActive(
    client: RuVectorClient,
    name: string,
    active: boolean
  ): Promise<void> {
    const spinner = ora(`Setting agent '${name}' ${active ? 'active' : 'inactive'}...`).start();

    try {
      await client.connect();
      await client.setAgentActive(name, active);
      spinner.succeed(chalk.green(`Agent '${name}' is now ${active ? 'active' : 'inactive'}`));
    } catch (err) {
      spinner.fail(chalk.red('Failed to update agent status'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async route(
    client: RuVectorClient,
    options: RouteOptions
  ): Promise<void> {
    const spinner = ora('Routing request to best agent...').start();

    try {
      await client.connect();

      const embedding = JSON.parse(options.embedding);
      const optimizeFor = options.optimizeFor || 'balanced';
      const constraints = options.constraints ? JSON.parse(options.constraints) : undefined;

      const decision = await client.route(embedding, optimizeFor, constraints);

      spinner.succeed(chalk.green('Routing decision made'));

      console.log(chalk.bold.blue('\nRouting Decision:'));
      console.log(chalk.gray('-'.repeat(50)));
      console.log(`  ${chalk.green('Selected Agent:')} ${chalk.bold(decision.agent_name)}`);
      console.log(`  ${chalk.green('Confidence:')} ${(decision.confidence * 100).toFixed(1)}%`);
      console.log(`  ${chalk.green('Estimated Cost:')} $${decision.estimated_cost.toFixed(4)}`);
      console.log(`  ${chalk.green('Estimated Latency:')} ${decision.estimated_latency_ms.toFixed(0)}ms`);
      console.log(`  ${chalk.green('Expected Quality:')} ${(decision.expected_quality * 100).toFixed(1)}%`);
      console.log(`  ${chalk.green('Similarity Score:')} ${decision.similarity_score.toFixed(4)}`);

      if (decision.reasoning) {
        console.log(`  ${chalk.green('Reasoning:')} ${decision.reasoning}`);
      }

      if (decision.alternatives && decision.alternatives.length > 0) {
        console.log(chalk.bold.blue('\nAlternatives:'));
        for (const alt of decision.alternatives.slice(0, 3)) {
          console.log(`  ${chalk.yellow('-')} ${alt.name} (score: ${alt.score?.toFixed(3) || 'N/A'})`);
        }
      }
    } catch (err) {
      spinner.fail(chalk.red('Routing failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async listAgents(client: RuVectorClient): Promise<void> {
    const spinner = ora('Fetching agents...').start();

    try {
      await client.connect();

      const agents = await client.listAgents();

      spinner.stop();

      if (agents.length === 0) {
        console.log(chalk.yellow('No agents registered'));
        return;
      }

      console.log(chalk.bold.blue(`\nRegistered Agents (${agents.length}):`));

      const table = new Table({
        head: [
          chalk.cyan('Name'),
          chalk.cyan('Type'),
          chalk.cyan('Cost'),
          chalk.cyan('Latency'),
          chalk.cyan('Quality'),
          chalk.cyan('Requests'),
          chalk.cyan('Active'),
        ],
        colWidths: [15, 12, 10, 10, 10, 10, 8],
      });

      for (const agent of agents) {
        table.push([
          agent.name,
          agent.agent_type,
          `$${agent.cost_per_request.toFixed(3)}`,
          `${agent.avg_latency_ms.toFixed(0)}ms`,
          `${(agent.quality_score * 100).toFixed(0)}%`,
          agent.total_requests.toString(),
          agent.is_active ? chalk.green('Yes') : chalk.red('No'),
        ]);
      }

      console.log(table.toString());
    } catch (err) {
      spinner.fail(chalk.red('Failed to list agents'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async getAgent(client: RuVectorClient, name: string): Promise<void> {
    const spinner = ora(`Fetching agent '${name}'...`).start();

    try {
      await client.connect();

      const agent = await client.getAgent(name);

      spinner.stop();

      console.log(chalk.bold.blue(`\nAgent: ${agent.name}`));
      console.log(chalk.gray('-'.repeat(50)));
      console.log(`  ${chalk.green('Type:')} ${agent.agent_type}`);
      console.log(`  ${chalk.green('Capabilities:')} ${agent.capabilities.join(', ')}`);
      console.log(`  ${chalk.green('Active:')} ${agent.is_active ? chalk.green('Yes') : chalk.red('No')}`);

      console.log(chalk.bold.blue('\nCost Model:'));
      console.log(`  ${chalk.green('Per Request:')} $${agent.cost_model.per_request}`);
      if (agent.cost_model.per_token) {
        console.log(`  ${chalk.green('Per Token:')} $${agent.cost_model.per_token}`);
      }

      console.log(chalk.bold.blue('\nPerformance:'));
      console.log(`  ${chalk.green('Avg Latency:')} ${agent.performance.avg_latency_ms}ms`);
      console.log(`  ${chalk.green('Quality Score:')} ${(agent.performance.quality_score * 100).toFixed(1)}%`);
      console.log(`  ${chalk.green('Success Rate:')} ${(agent.performance.success_rate * 100).toFixed(1)}%`);
      console.log(`  ${chalk.green('Total Requests:')} ${agent.performance.total_requests}`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to get agent'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async findByCapability(
    client: RuVectorClient,
    options: FindAgentsOptions
  ): Promise<void> {
    const spinner = ora(`Finding agents with '${options.capability}'...`).start();

    try {
      await client.connect();

      const limit = options.limit ? parseInt(options.limit) : 10;
      const agents = await client.findAgentsByCapability(options.capability, limit);

      spinner.stop();

      if (agents.length === 0) {
        console.log(chalk.yellow(`No agents found with capability '${options.capability}'`));
        return;
      }

      console.log(chalk.bold.blue(`\nAgents with '${options.capability}' (${agents.length}):`));

      const table = new Table({
        head: [
          chalk.cyan('Name'),
          chalk.cyan('Quality'),
          chalk.cyan('Latency'),
          chalk.cyan('Cost'),
        ],
        colWidths: [20, 12, 12, 12],
      });

      for (const agent of agents) {
        table.push([
          agent.name,
          `${(agent.quality_score * 100).toFixed(0)}%`,
          `${agent.avg_latency_ms.toFixed(0)}ms`,
          `$${agent.cost_per_request.toFixed(3)}`,
        ]);
      }

      console.log(table.toString());
    } catch (err) {
      spinner.fail(chalk.red('Failed to find agents'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async stats(client: RuVectorClient): Promise<void> {
    const spinner = ora('Fetching routing statistics...').start();

    try {
      await client.connect();

      const stats = await client.routingStats();

      spinner.stop();

      console.log(chalk.bold.blue('\nRouting Statistics:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Total Agents:')} ${stats.total_agents}`);
      console.log(`  ${chalk.green('Active Agents:')} ${stats.active_agents}`);
      console.log(`  ${chalk.green('Total Requests:')} ${stats.total_requests}`);
      console.log(`  ${chalk.green('Avg Quality:')} ${(stats.average_quality * 100).toFixed(1)}%`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to get stats'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async clearAgents(client: RuVectorClient): Promise<void> {
    const spinner = ora('Clearing all agents...').start();

    try {
      await client.connect();
      await client.clearAgents();
      spinner.succeed(chalk.green('All agents cleared'));
    } catch (err) {
      spinner.fail(chalk.red('Failed to clear agents'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static showHelp(): void {
    console.log(chalk.bold.blue('\nTiny Dancer Routing System:'));
    console.log(chalk.gray('-'.repeat(60)));

    console.log(`
${chalk.yellow('Overview:')}
  Intelligent routing of AI requests to the most suitable agent
  based on cost, latency, quality, and capabilities.

${chalk.yellow('Agent Types:')}
  ${chalk.green('llm')}         - Large Language Models (GPT-4, Claude, etc.)
  ${chalk.green('embedding')}   - Embedding models
  ${chalk.green('specialized')} - Domain-specific models
  ${chalk.green('multimodal')}  - Vision/audio models

${chalk.yellow('Optimization Targets:')}
  ${chalk.green('cost')}     - Minimize cost
  ${chalk.green('latency')}  - Minimize response time
  ${chalk.green('quality')}  - Maximize output quality
  ${chalk.green('balanced')} - Balance all factors (default)

${chalk.yellow('Commands:')}
  ${chalk.green('routing register')}      - Register a new agent
  ${chalk.green('routing register-full')} - Register with full JSON config
  ${chalk.green('routing update')}        - Update agent metrics
  ${chalk.green('routing remove')}        - Remove an agent
  ${chalk.green('routing set-active')}    - Enable/disable agent
  ${chalk.green('routing route')}         - Route a request
  ${chalk.green('routing list')}          - List all agents
  ${chalk.green('routing get')}           - Get agent details
  ${chalk.green('routing find')}          - Find agents by capability
  ${chalk.green('routing stats')}         - Get routing statistics
  ${chalk.green('routing clear')}         - Clear all agents

${chalk.yellow('Example:')}
  ${chalk.gray('# Register an agent')}
  ruvector-pg routing register \\
    --name gpt-4 \\
    --type llm \\
    --capabilities "code,translation,analysis" \\
    --cost 0.03 \\
    --latency 500 \\
    --quality 0.95

  ${chalk.gray('# Route a request')}
  ruvector-pg routing route \\
    --embedding "[0.1, 0.2, ...]" \\
    --optimize-for balanced \\
    --constraints '{"max_cost": 0.1}'
`);
  }
}

export default RoutingCommands;
