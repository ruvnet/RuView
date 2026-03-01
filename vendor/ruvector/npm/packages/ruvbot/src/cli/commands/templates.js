"use strict";
/**
 * RuvBot CLI - Templates Command
 *
 * Deploy pre-built agent templates with a single command.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.createTemplatesCommand = createTemplatesCommand;
exports.createDeployCommand = createDeployCommand;
const commander_1 = require("commander");
const index_js_1 = require("../../templates/index.js");
function createTemplatesCommand() {
    const templates = new commander_1.Command('templates')
        .alias('t')
        .description('Manage and deploy agent templates');
    // List templates
    templates
        .command('list')
        .alias('ls')
        .option('-c, --category <category>', 'Filter by category (practical, intermediate, advanced, exotic)')
        .option('--json', 'Output as JSON')
        .description('List available templates')
        .action(async (options) => {
        const byCategory = (0, index_js_1.getTemplatesByCategory)();
        if (options.json) {
            console.log(JSON.stringify(byCategory, null, 2));
            return;
        }
        console.log('\n🤖 RuvBot Template Library\n');
        console.log('Deploy with: npx ruvbot deploy <template-id>\n');
        const categories = options.category
            ? { [options.category]: byCategory[options.category] || [] }
            : byCategory;
        for (const [category, templates] of Object.entries(categories)) {
            const emoji = getCategoryEmoji(category);
            console.log(`${emoji} ${category.toUpperCase()}`);
            console.log('─'.repeat(50));
            for (const t of templates) {
                console.log(`  ${t.id.padEnd(25)} ${t.name}`);
                console.log(`  ${''.padEnd(25)} ${dim(t.description)}`);
                console.log();
            }
        }
    });
    // Show template details
    templates
        .command('info <template-id>')
        .description('Show detailed information about a template')
        .action(async (templateId) => {
        const template = (0, index_js_1.getTemplate)(templateId);
        if (!template) {
            console.error(`Template "${templateId}" not found.`);
            console.log('\nAvailable templates:');
            (0, index_js_1.listTemplates)().forEach(t => console.log(`  - ${t.id}`));
            process.exit(1);
        }
        console.log(`\n${getCategoryEmoji(template.category)} ${template.name}`);
        console.log('═'.repeat(50));
        console.log(`\n${template.description}\n`);
        console.log('📋 Configuration:');
        console.log(`   Topology:   ${template.config.topology}`);
        console.log(`   Max Agents: ${template.config.maxAgents}`);
        if (template.config.consensus) {
            console.log(`   Consensus:  ${template.config.consensus}`);
        }
        if (template.config.memory) {
            console.log(`   Memory:     ${template.config.memory}`);
        }
        if (template.config.workers?.length) {
            console.log(`   Workers:    ${template.config.workers.join(', ')}`);
        }
        console.log('\n🤖 Agents:');
        for (const agent of template.agents) {
            console.log(`   • ${agent.name} (${agent.type})`);
            console.log(`     ${dim(agent.role)}`);
        }
        console.log('\n📝 Example:');
        console.log(`   ${template.example}`);
        console.log();
    });
    return templates;
}
function createDeployCommand() {
    const deploy = new commander_1.Command('deploy')
        .argument('<template-id>', 'Template to deploy')
        .option('--name <name>', 'Custom name for the deployment')
        .option('--model <model>', 'Override default LLM model')
        .option('--dry-run', 'Show what would be deployed without executing')
        .option('--background', 'Run in background')
        .description('Deploy a template')
        .action(async (templateId, options) => {
        const template = (0, index_js_1.getTemplate)(templateId);
        if (!template) {
            console.error(`Template "${templateId}" not found.`);
            console.log('\nRun "npx ruvbot templates list" to see available templates.');
            process.exit(1);
        }
        console.log(`\n🚀 Deploying: ${template.name}`);
        console.log('─'.repeat(50));
        if (options.dryRun) {
            console.log('\n[DRY RUN] Would deploy:\n');
            showDeploymentPlan(template, options);
            return;
        }
        // Generate deployment commands
        const commands = generateDeploymentCommands(template, options);
        console.log('\n📦 Initializing swarm...');
        console.log(dim(`   ${commands.swarmInit}`));
        console.log('\n🤖 Spawning agents:');
        for (const cmd of commands.agentSpawns) {
            console.log(dim(`   ${cmd}`));
        }
        if (commands.workerStarts.length > 0) {
            console.log('\n⚙️  Starting background workers:');
            for (const cmd of commands.workerStarts) {
                console.log(dim(`   ${cmd}`));
            }
        }
        console.log('\n✅ Deployment complete!');
        console.log(`\n📊 Monitor with: npx ruvbot status`);
        console.log(`🛑 Stop with:    npx ruvbot stop ${options.name || templateId}`);
    });
    return deploy;
}
function showDeploymentPlan(template, options) {
    console.log(`Template:    ${template.id}`);
    console.log(`Category:    ${template.category}`);
    console.log(`Topology:    ${template.config.topology}`);
    console.log(`Max Agents:  ${template.config.maxAgents}`);
    console.log();
    console.log('Agents to spawn:');
    for (const agent of template.agents) {
        console.log(`  • ${agent.name} (${agent.type})`);
    }
    if (template.config.workers?.length) {
        console.log();
        console.log('Workers to start:');
        for (const worker of template.config.workers) {
            console.log(`  • ${worker}`);
        }
    }
}
function generateDeploymentCommands(template, options) {
    const name = options.name || template.id;
    // Swarm initialization
    const swarmInit = `npx @claude-flow/cli@latest swarm init --topology ${template.config.topology} --max-agents ${template.config.maxAgents}${template.config.consensus ? ` --consensus ${template.config.consensus}` : ''}`;
    // Agent spawn commands
    const agentSpawns = template.agents.map(agent => {
        const model = options.model || agent.model || 'google/gemini-2.0-flash-001';
        return `npx @claude-flow/cli@latest agent spawn -t ${agent.type} --name ${agent.name}`;
    });
    // Worker start commands
    const workerStarts = (template.config.workers || []).map(worker => `npx @claude-flow/cli@latest hooks worker dispatch --trigger ${worker}`);
    return { swarmInit, agentSpawns, workerStarts };
}
function getCategoryEmoji(category) {
    const emojis = {
        practical: '🔧',
        intermediate: '⚡',
        advanced: '🧠',
        exotic: '🌌',
    };
    return emojis[category] || '📦';
}
function dim(text) {
    return `\x1b[2m${text}\x1b[0m`;
}
//# sourceMappingURL=templates.js.map