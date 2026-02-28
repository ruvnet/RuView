"use strict";
/**
 * Agent Command - Agent and swarm management
 *
 * Commands:
 *   agent spawn     Spawn a new agent
 *   agent list      List running agents
 *   agent stop      Stop an agent
 *   agent status    Show agent status
 *   swarm init      Initialize swarm coordination
 *   swarm status    Show swarm status
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.createAgentCommand = createAgentCommand;
const commander_1 = require("commander");
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const SwarmCoordinator_js_1 = require("../../swarm/SwarmCoordinator.js");
const VALID_WORKER_TYPES = [
    'ultralearn', 'optimize', 'consolidate', 'predict', 'audit',
    'map', 'preload', 'deepdive', 'document', 'refactor', 'benchmark', 'testgaps'
];
function createAgentCommand() {
    const agent = new commander_1.Command('agent');
    agent.description('Agent and swarm management commands');
    // Spawn command
    agent
        .command('spawn')
        .description('Spawn a new agent')
        .option('-t, --type <type>', 'Agent type (worker type)', 'optimize')
        .option('--json', 'Output as JSON')
        .action(async (options) => {
        const spinner = (0, ora_1.default)(`Spawning ${options.type} agent...`).start();
        try {
            const workerType = options.type;
            if (!VALID_WORKER_TYPES.includes(workerType)) {
                spinner.fail(chalk_1.default.red(`Invalid worker type: ${options.type}`));
                console.log(chalk_1.default.gray(`Valid types: ${VALID_WORKER_TYPES.join(', ')}`));
                process.exit(1);
            }
            const coordinator = new SwarmCoordinator_js_1.SwarmCoordinator();
            await coordinator.start();
            const spawnedAgent = await coordinator.spawnAgent(workerType);
            spinner.stop();
            if (options.json) {
                console.log(JSON.stringify(spawnedAgent, null, 2));
                return;
            }
            console.log(chalk_1.default.green(`✓ Agent spawned: ${chalk_1.default.cyan(spawnedAgent.id)}`));
            console.log(chalk_1.default.gray(`  Type: ${spawnedAgent.type}`));
            console.log(chalk_1.default.gray(`  Status: ${spawnedAgent.status}`));
        }
        catch (error) {
            spinner.fail(chalk_1.default.red(`Spawn failed: ${error.message}`));
            process.exit(1);
        }
    });
    // List command
    agent
        .command('list')
        .description('List running agents')
        .option('--json', 'Output as JSON')
        .action(async (options) => {
        try {
            const coordinator = new SwarmCoordinator_js_1.SwarmCoordinator();
            const agents = coordinator.getAgents();
            if (options.json) {
                console.log(JSON.stringify(agents, null, 2));
                return;
            }
            if (agents.length === 0) {
                console.log(chalk_1.default.yellow('No agents running'));
                console.log(chalk_1.default.gray('Spawn one with: ruvbot agent spawn -t optimize'));
                return;
            }
            console.log(chalk_1.default.bold(`\n🤖 Agents (${agents.length})\n`));
            console.log('─'.repeat(70));
            console.log(chalk_1.default.gray('ID'.padEnd(40) + 'TYPE'.padEnd(15) + 'STATUS'.padEnd(12) + 'TASKS'));
            console.log('─'.repeat(70));
            for (const a of agents) {
                const statusColor = a.status === 'busy' ? chalk_1.default.green : a.status === 'idle' ? chalk_1.default.yellow : chalk_1.default.gray;
                console.log(chalk_1.default.cyan(a.id.padEnd(40)) +
                    a.type.padEnd(15) +
                    statusColor(a.status.padEnd(12)) +
                    chalk_1.default.gray(String(a.completedTasks)));
            }
            console.log('─'.repeat(70));
        }
        catch (error) {
            console.error(chalk_1.default.red(`List failed: ${error.message}`));
            process.exit(1);
        }
    });
    // Stop command
    agent
        .command('stop')
        .description('Stop an agent')
        .argument('<id>', 'Agent ID')
        .action(async (id) => {
        const spinner = (0, ora_1.default)(`Stopping agent ${id}...`).start();
        try {
            const coordinator = new SwarmCoordinator_js_1.SwarmCoordinator();
            const removed = await coordinator.removeAgent(id);
            if (removed) {
                spinner.succeed(chalk_1.default.green(`Agent ${id} stopped`));
            }
            else {
                spinner.fail(chalk_1.default.red(`Agent ${id} not found`));
                process.exit(1);
            }
        }
        catch (error) {
            spinner.fail(chalk_1.default.red(`Stop failed: ${error.message}`));
            process.exit(1);
        }
    });
    // Status command
    agent
        .command('status')
        .description('Show agent/swarm status')
        .argument('[id]', 'Agent ID (optional)')
        .option('--json', 'Output as JSON')
        .action(async (id, options) => {
        try {
            const coordinator = new SwarmCoordinator_js_1.SwarmCoordinator();
            if (id) {
                const agentStatus = coordinator.getAgent(id);
                if (!agentStatus) {
                    console.log(chalk_1.default.red(`Agent ${id} not found`));
                    process.exit(1);
                }
                if (options.json) {
                    console.log(JSON.stringify(agentStatus, null, 2));
                    return;
                }
                console.log(chalk_1.default.bold(`\n🤖 Agent: ${id}\n`));
                console.log('─'.repeat(40));
                console.log(`Status:     ${agentStatus.status === 'busy' ? chalk_1.default.green(agentStatus.status) : chalk_1.default.yellow(agentStatus.status)}`);
                console.log(`Type:       ${chalk_1.default.cyan(agentStatus.type)}`);
                console.log(`Completed:  ${agentStatus.completedTasks}`);
                console.log(`Failed:     ${agentStatus.failedTasks}`);
                if (agentStatus.currentTask) {
                    console.log(`Task:       ${agentStatus.currentTask}`);
                }
                console.log('─'.repeat(40));
            }
            else {
                // Show overall swarm status
                const status = coordinator.getStatus();
                if (options.json) {
                    console.log(JSON.stringify(status, null, 2));
                    return;
                }
                console.log(chalk_1.default.bold('\n🐝 Swarm Status\n'));
                console.log('─'.repeat(40));
                console.log(`Topology:       ${chalk_1.default.cyan(status.topology)}`);
                console.log(`Consensus:      ${chalk_1.default.cyan(status.consensus)}`);
                console.log(`Total Agents:   ${chalk_1.default.cyan(status.agentCount)} / ${status.maxAgents}`);
                console.log(`Idle:           ${chalk_1.default.yellow(status.idleAgents)}`);
                console.log(`Busy:           ${chalk_1.default.green(status.busyAgents)}`);
                console.log(`Pending Tasks:  ${chalk_1.default.yellow(status.pendingTasks)}`);
                console.log(`Running Tasks:  ${chalk_1.default.blue(status.runningTasks)}`);
                console.log(`Completed:      ${chalk_1.default.green(status.completedTasks)}`);
                console.log(`Failed:         ${chalk_1.default.red(status.failedTasks)}`);
                console.log('─'.repeat(40));
            }
        }
        catch (error) {
            console.error(chalk_1.default.red(`Status failed: ${error.message}`));
            process.exit(1);
        }
    });
    // Swarm subcommands
    const swarm = agent.command('swarm').description('Swarm coordination commands');
    // Swarm init
    swarm
        .command('init')
        .description('Initialize swarm coordination')
        .option('--topology <topology>', 'Swarm topology: hierarchical, mesh, hierarchical-mesh, adaptive', 'hierarchical')
        .option('--max-agents <max>', 'Maximum agents', '8')
        .option('--strategy <strategy>', 'Coordination strategy: specialized, balanced, adaptive', 'specialized')
        .option('--consensus <consensus>', 'Consensus algorithm: raft, byzantine, gossip, crdt', 'raft')
        .action(async (options) => {
        const spinner = (0, ora_1.default)('Initializing swarm...').start();
        try {
            const coordinator = new SwarmCoordinator_js_1.SwarmCoordinator({
                topology: options.topology,
                maxAgents: parseInt(options.maxAgents, 10),
                strategy: options.strategy,
                consensus: options.consensus,
            });
            await coordinator.start();
            spinner.succeed(chalk_1.default.green('Swarm initialized'));
            console.log(chalk_1.default.gray(`  Topology: ${options.topology}`));
            console.log(chalk_1.default.gray(`  Max Agents: ${options.maxAgents}`));
            console.log(chalk_1.default.gray(`  Strategy: ${options.strategy}`));
            console.log(chalk_1.default.gray(`  Consensus: ${options.consensus}`));
        }
        catch (error) {
            spinner.fail(chalk_1.default.red(`Init failed: ${error.message}`));
            process.exit(1);
        }
    });
    // Swarm status
    swarm
        .command('status')
        .description('Show swarm status')
        .option('--json', 'Output as JSON')
        .action(async (options) => {
        try {
            const coordinator = new SwarmCoordinator_js_1.SwarmCoordinator();
            const status = coordinator.getStatus();
            if (options.json) {
                console.log(JSON.stringify(status, null, 2));
                return;
            }
            console.log(chalk_1.default.bold('\n🐝 Swarm Status\n'));
            console.log('─'.repeat(50));
            console.log(`Topology:      ${chalk_1.default.cyan(status.topology)}`);
            console.log(`Consensus:     ${chalk_1.default.cyan(status.consensus)}`);
            console.log(`Total Agents:  ${chalk_1.default.cyan(status.agentCount)}`);
            console.log(`Active:        ${chalk_1.default.green(status.busyAgents)}`);
            console.log(`Idle:          ${chalk_1.default.yellow(status.idleAgents)}`);
            console.log(`Pending Tasks: ${chalk_1.default.yellow(status.pendingTasks)}`);
            console.log(`Completed:     ${chalk_1.default.green(status.completedTasks)}`);
            console.log('─'.repeat(50));
        }
        catch (error) {
            console.error(chalk_1.default.red(`Status failed: ${error.message}`));
            process.exit(1);
        }
    });
    // Swarm dispatch (bonus command)
    swarm
        .command('dispatch')
        .description('Dispatch a task to the swarm')
        .requiredOption('-w, --worker <type>', 'Worker type')
        .requiredOption('--task <task>', 'Task type')
        .option('--content <content>', 'Task content')
        .option('--priority <priority>', 'Priority: low, normal, high, critical', 'normal')
        .action(async (options) => {
        const spinner = (0, ora_1.default)('Dispatching task...').start();
        try {
            const coordinator = new SwarmCoordinator_js_1.SwarmCoordinator();
            await coordinator.start();
            const task = await coordinator.dispatch({
                worker: options.worker,
                task: {
                    type: options.task,
                    content: options.content || {},
                },
                priority: options.priority,
            });
            spinner.succeed(chalk_1.default.green(`Task dispatched: ${task.id}`));
            console.log(chalk_1.default.gray(`  Worker: ${task.worker}`));
            console.log(chalk_1.default.gray(`  Type: ${task.type}`));
            console.log(chalk_1.default.gray(`  Priority: ${task.priority}`));
            console.log(chalk_1.default.gray(`  Status: ${task.status}`));
        }
        catch (error) {
            spinner.fail(chalk_1.default.red(`Dispatch failed: ${error.message}`));
            process.exit(1);
        }
    });
    return agent;
}
exports.default = createAgentCommand;
//# sourceMappingURL=agent.js.map