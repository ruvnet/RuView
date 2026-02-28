"use strict";
/**
 * Routing/Agent Commands
 * CLI commands for Tiny Dancer agent routing and management
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.RoutingCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
class RoutingCommands {
    static async registerAgent(client, options) {
        const spinner = (0, ora_1.default)(`Registering agent '${options.name}'...`).start();
        try {
            await client.connect();
            const capabilities = options.capabilities.split(',').map(c => c.trim());
            await client.registerAgent(options.name, options.type, capabilities, parseFloat(options.cost), parseFloat(options.latency), parseFloat(options.quality));
            spinner.succeed(chalk_1.default.green(`Agent '${options.name}' registered successfully`));
            console.log(chalk_1.default.bold.blue('\nAgent Details:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Name:')} ${options.name}`);
            console.log(`  ${chalk_1.default.green('Type:')} ${options.type}`);
            console.log(`  ${chalk_1.default.green('Capabilities:')} ${capabilities.join(', ')}`);
            console.log(`  ${chalk_1.default.green('Cost/Request:')} $${options.cost}`);
            console.log(`  ${chalk_1.default.green('Avg Latency:')} ${options.latency}ms`);
            console.log(`  ${chalk_1.default.green('Quality Score:')} ${options.quality}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to register agent'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async registerAgentFull(client, options) {
        const spinner = (0, ora_1.default)('Registering agent with full config...').start();
        try {
            await client.connect();
            const config = JSON.parse(options.config);
            await client.registerAgentFull(config);
            spinner.succeed(chalk_1.default.green(`Agent '${config.name}' registered successfully`));
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to register agent'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async updateMetrics(client, options) {
        const spinner = (0, ora_1.default)(`Updating metrics for '${options.name}'...`).start();
        try {
            await client.connect();
            await client.updateAgentMetrics(options.name, parseFloat(options.latency), options.success, options.quality ? parseFloat(options.quality) : undefined);
            spinner.succeed(chalk_1.default.green('Metrics updated'));
            console.log(`  ${chalk_1.default.green('Latency:')} ${options.latency}ms`);
            console.log(`  ${chalk_1.default.green('Success:')} ${options.success}`);
            if (options.quality) {
                console.log(`  ${chalk_1.default.green('Quality:')} ${options.quality}`);
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to update metrics'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async removeAgent(client, name) {
        const spinner = (0, ora_1.default)(`Removing agent '${name}'...`).start();
        try {
            await client.connect();
            await client.removeAgent(name);
            spinner.succeed(chalk_1.default.green(`Agent '${name}' removed`));
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to remove agent'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async setActive(client, name, active) {
        const spinner = (0, ora_1.default)(`Setting agent '${name}' ${active ? 'active' : 'inactive'}...`).start();
        try {
            await client.connect();
            await client.setAgentActive(name, active);
            spinner.succeed(chalk_1.default.green(`Agent '${name}' is now ${active ? 'active' : 'inactive'}`));
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to update agent status'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async route(client, options) {
        const spinner = (0, ora_1.default)('Routing request to best agent...').start();
        try {
            await client.connect();
            const embedding = JSON.parse(options.embedding);
            const optimizeFor = options.optimizeFor || 'balanced';
            const constraints = options.constraints ? JSON.parse(options.constraints) : undefined;
            const decision = await client.route(embedding, optimizeFor, constraints);
            spinner.succeed(chalk_1.default.green('Routing decision made'));
            console.log(chalk_1.default.bold.blue('\nRouting Decision:'));
            console.log(chalk_1.default.gray('-'.repeat(50)));
            console.log(`  ${chalk_1.default.green('Selected Agent:')} ${chalk_1.default.bold(decision.agent_name)}`);
            console.log(`  ${chalk_1.default.green('Confidence:')} ${(decision.confidence * 100).toFixed(1)}%`);
            console.log(`  ${chalk_1.default.green('Estimated Cost:')} $${decision.estimated_cost.toFixed(4)}`);
            console.log(`  ${chalk_1.default.green('Estimated Latency:')} ${decision.estimated_latency_ms.toFixed(0)}ms`);
            console.log(`  ${chalk_1.default.green('Expected Quality:')} ${(decision.expected_quality * 100).toFixed(1)}%`);
            console.log(`  ${chalk_1.default.green('Similarity Score:')} ${decision.similarity_score.toFixed(4)}`);
            if (decision.reasoning) {
                console.log(`  ${chalk_1.default.green('Reasoning:')} ${decision.reasoning}`);
            }
            if (decision.alternatives && decision.alternatives.length > 0) {
                console.log(chalk_1.default.bold.blue('\nAlternatives:'));
                for (const alt of decision.alternatives.slice(0, 3)) {
                    console.log(`  ${chalk_1.default.yellow('-')} ${alt.name} (score: ${alt.score?.toFixed(3) || 'N/A'})`);
                }
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Routing failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async listAgents(client) {
        const spinner = (0, ora_1.default)('Fetching agents...').start();
        try {
            await client.connect();
            const agents = await client.listAgents();
            spinner.stop();
            if (agents.length === 0) {
                console.log(chalk_1.default.yellow('No agents registered'));
                return;
            }
            console.log(chalk_1.default.bold.blue(`\nRegistered Agents (${agents.length}):`));
            const table = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('Name'),
                    chalk_1.default.cyan('Type'),
                    chalk_1.default.cyan('Cost'),
                    chalk_1.default.cyan('Latency'),
                    chalk_1.default.cyan('Quality'),
                    chalk_1.default.cyan('Requests'),
                    chalk_1.default.cyan('Active'),
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
                    agent.is_active ? chalk_1.default.green('Yes') : chalk_1.default.red('No'),
                ]);
            }
            console.log(table.toString());
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to list agents'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async getAgent(client, name) {
        const spinner = (0, ora_1.default)(`Fetching agent '${name}'...`).start();
        try {
            await client.connect();
            const agent = await client.getAgent(name);
            spinner.stop();
            console.log(chalk_1.default.bold.blue(`\nAgent: ${agent.name}`));
            console.log(chalk_1.default.gray('-'.repeat(50)));
            console.log(`  ${chalk_1.default.green('Type:')} ${agent.agent_type}`);
            console.log(`  ${chalk_1.default.green('Capabilities:')} ${agent.capabilities.join(', ')}`);
            console.log(`  ${chalk_1.default.green('Active:')} ${agent.is_active ? chalk_1.default.green('Yes') : chalk_1.default.red('No')}`);
            console.log(chalk_1.default.bold.blue('\nCost Model:'));
            console.log(`  ${chalk_1.default.green('Per Request:')} $${agent.cost_model.per_request}`);
            if (agent.cost_model.per_token) {
                console.log(`  ${chalk_1.default.green('Per Token:')} $${agent.cost_model.per_token}`);
            }
            console.log(chalk_1.default.bold.blue('\nPerformance:'));
            console.log(`  ${chalk_1.default.green('Avg Latency:')} ${agent.performance.avg_latency_ms}ms`);
            console.log(`  ${chalk_1.default.green('Quality Score:')} ${(agent.performance.quality_score * 100).toFixed(1)}%`);
            console.log(`  ${chalk_1.default.green('Success Rate:')} ${(agent.performance.success_rate * 100).toFixed(1)}%`);
            console.log(`  ${chalk_1.default.green('Total Requests:')} ${agent.performance.total_requests}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to get agent'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async findByCapability(client, options) {
        const spinner = (0, ora_1.default)(`Finding agents with '${options.capability}'...`).start();
        try {
            await client.connect();
            const limit = options.limit ? parseInt(options.limit) : 10;
            const agents = await client.findAgentsByCapability(options.capability, limit);
            spinner.stop();
            if (agents.length === 0) {
                console.log(chalk_1.default.yellow(`No agents found with capability '${options.capability}'`));
                return;
            }
            console.log(chalk_1.default.bold.blue(`\nAgents with '${options.capability}' (${agents.length}):`));
            const table = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('Name'),
                    chalk_1.default.cyan('Quality'),
                    chalk_1.default.cyan('Latency'),
                    chalk_1.default.cyan('Cost'),
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
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to find agents'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async stats(client) {
        const spinner = (0, ora_1.default)('Fetching routing statistics...').start();
        try {
            await client.connect();
            const stats = await client.routingStats();
            spinner.stop();
            console.log(chalk_1.default.bold.blue('\nRouting Statistics:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Total Agents:')} ${stats.total_agents}`);
            console.log(`  ${chalk_1.default.green('Active Agents:')} ${stats.active_agents}`);
            console.log(`  ${chalk_1.default.green('Total Requests:')} ${stats.total_requests}`);
            console.log(`  ${chalk_1.default.green('Avg Quality:')} ${(stats.average_quality * 100).toFixed(1)}%`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to get stats'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async clearAgents(client) {
        const spinner = (0, ora_1.default)('Clearing all agents...').start();
        try {
            await client.connect();
            await client.clearAgents();
            spinner.succeed(chalk_1.default.green('All agents cleared'));
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to clear agents'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static showHelp() {
        console.log(chalk_1.default.bold.blue('\nTiny Dancer Routing System:'));
        console.log(chalk_1.default.gray('-'.repeat(60)));
        console.log(`
${chalk_1.default.yellow('Overview:')}
  Intelligent routing of AI requests to the most suitable agent
  based on cost, latency, quality, and capabilities.

${chalk_1.default.yellow('Agent Types:')}
  ${chalk_1.default.green('llm')}         - Large Language Models (GPT-4, Claude, etc.)
  ${chalk_1.default.green('embedding')}   - Embedding models
  ${chalk_1.default.green('specialized')} - Domain-specific models
  ${chalk_1.default.green('multimodal')}  - Vision/audio models

${chalk_1.default.yellow('Optimization Targets:')}
  ${chalk_1.default.green('cost')}     - Minimize cost
  ${chalk_1.default.green('latency')}  - Minimize response time
  ${chalk_1.default.green('quality')}  - Maximize output quality
  ${chalk_1.default.green('balanced')} - Balance all factors (default)

${chalk_1.default.yellow('Commands:')}
  ${chalk_1.default.green('routing register')}      - Register a new agent
  ${chalk_1.default.green('routing register-full')} - Register with full JSON config
  ${chalk_1.default.green('routing update')}        - Update agent metrics
  ${chalk_1.default.green('routing remove')}        - Remove an agent
  ${chalk_1.default.green('routing set-active')}    - Enable/disable agent
  ${chalk_1.default.green('routing route')}         - Route a request
  ${chalk_1.default.green('routing list')}          - List all agents
  ${chalk_1.default.green('routing get')}           - Get agent details
  ${chalk_1.default.green('routing find')}          - Find agents by capability
  ${chalk_1.default.green('routing stats')}         - Get routing statistics
  ${chalk_1.default.green('routing clear')}         - Clear all agents

${chalk_1.default.yellow('Example:')}
  ${chalk_1.default.gray('# Register an agent')}
  ruvector-pg routing register \\
    --name gpt-4 \\
    --type llm \\
    --capabilities "code,translation,analysis" \\
    --cost 0.03 \\
    --latency 500 \\
    --quality 0.95

  ${chalk_1.default.gray('# Route a request')}
  ruvector-pg routing route \\
    --embedding "[0.1, 0.2, ...]" \\
    --optimize-for balanced \\
    --constraints '{"max_cost": 0.1}'
`);
    }
}
exports.RoutingCommands = RoutingCommands;
exports.default = RoutingCommands;
//# sourceMappingURL=routing.js.map