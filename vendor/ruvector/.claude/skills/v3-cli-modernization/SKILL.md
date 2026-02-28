---
name: "V3 CLI Modernization"
description: "CLI modernization and hooks system enhancement for claude-flow v3. Implements interactive prompts, command decomposition, enhanced hooks integration, and intelligent workflow automation."
---

# V3 CLI Modernization

## What This Skill Does

Modernizes claude-flow v3 CLI with interactive prompts, intelligent command decomposition, enhanced hooks integration, performance optimization, and comprehensive workflow automation capabilities.

## Quick Start

```bash
# Initialize CLI modernization analysis
Task("CLI architecture", "Analyze current CLI structure and identify optimization opportunities", "cli-hooks-developer")

# Modernization implementation (parallel)
Task("Command decomposition", "Break down large CLI files into focused modules", "cli-hooks-developer")
Task("Interactive prompts", "Implement intelligent interactive CLI experience", "cli-hooks-developer")
Task("Hooks enhancement", "Deep integrate hooks with CLI lifecycle", "cli-hooks-developer")
```

## CLI Architecture Modernization

### Current State Analysis
```
Current CLI Issues:
├── index.ts: 108KB monolithic file
├── enterprise.ts: 68KB feature module
├── Limited interactivity: Basic command parsing
├── Hooks integration: Basic pre/post execution
└── No intelligent workflows: Manual command chaining

Target Architecture:
├── Modular Commands: <500 lines per command
├── Interactive Prompts: Smart context-aware UX
├── Enhanced Hooks: Deep lifecycle integration
├── Workflow Automation: Intelligent command orchestration
└── Performance: <200ms command response time
```

### Modular Command Architecture
```typescript
// src/cli/core/command-registry.ts
interface CommandModule {
  name: string;
  description: string;
  category: CommandCategory;
  handler: CommandHandler;
  middleware: MiddlewareStack;
  permissions: Permission[];
  examples: CommandExample[];
}

export class ModularCommandRegistry {
  private commands = new Map<string, CommandModule>();
  private categories = new Map<CommandCategory, CommandModule[]>();
  private aliases = new Map<string, string>();

  registerCommand(command: CommandModule): void {
    this.commands.set(command.name, command);

    // Register in category index
    if (!this.categories.has(command.category)) {
      this.categories.set(command.category, []);
    }
    this.categories.get(command.category)!.push(command);
  }

  async executeCommand(name: string, args: string[]): Promise<CommandResult> {
    const command = this.resolveCommand(name);
    if (!command) {
      throw new CommandNotFoundError(name, this.getSuggestions(name));
    }

    // Execute middleware stack
    const context = await this.buildExecutionContext(command, args);
    const result = await command.middleware.execute(context);

    return result;
  }

  private resolveCommand(name: string): CommandModule | undefined {
    // Try exact match first
    if (this.commands.has(name)) {
      return this.commands.get(name);
    }

    // Try alias
    const aliasTarget = this.aliases.get(name);
    if (aliasTarget) {
      return this.commands.get(aliasTarget);
    }

    // Try fuzzy match
    return this.findFuzzyMatch(name);
  }
}
```

## Command Decomposition Strategy

### Swarm Commands Module
```typescript
// src/cli/commands/swarm/swarm.command.ts
@Command({
  name: 'swarm',
  description: 'Swarm coordination and management',
  category: 'orchestration'
})
export class SwarmCommand {
  constructor(
    private swarmCoordinator: UnifiedSwarmCoordinator,
    private promptService: InteractivePromptService
  ) {}

  @SubCommand('init')
  @Option('--topology', 'Swarm topology (mesh|hierarchical|adaptive)', 'hierarchical')
  @Option('--agents', 'Number of agents to spawn', 5)
  @Option('--interactive', 'Interactive agent configuration', false)
  async init(
    @Arg('projectName') projectName: string,
    options: SwarmInitOptions
  ): Promise<CommandResult> {

    if (options.interactive) {
      return this.interactiveSwarmInit(projectName);
    }

    return this.quickSwarmInit(projectName, options);
  }

  private async interactiveSwarmInit(projectName: string): Promise<CommandResult> {
    console.log(`🚀 Initializing Swarm for ${projectName}`);

    // Interactive topology selection
    const topology = await this.promptService.select({
      message: 'Select swarm topology:',
      choices: [
        { name: 'Hierarchical (Queen-led coordination)', value: 'hierarchical' },
        { name: 'Mesh (Peer-to-peer collaboration)', value: 'mesh' },
        { name: 'Adaptive (Dynamic topology switching)', value: 'adaptive' }
      ]
    });

    // Agent configuration
    const agents = await this.promptAgentConfiguration();

    // Initialize with configuration
    const swarm = await this.swarmCoordinator.initialize({
      name: projectName,
      topology,
      agents,
      hooks: {
        onAgentSpawn: this.handleAgentSpawn.bind(this),
        onTaskComplete: this.handleTaskComplete.bind(this),
        onSwarmComplete: this.handleSwarmComplete.bind(this)
      }
    });

    return CommandResult.success({
      message: `✅ Swarm ${projectName} initialized with ${agents.length} agents`,
      data: { swarmId: swarm.id, topology, agentCount: agents.length }
    });
  }

  @SubCommand('status')
  async status(): Promise<CommandResult> {
    const swarms = await this.swarmCoordinator.listActiveSwarms();

    if (swarms.length === 0) {
      return CommandResult.info('No active swarms found');
    }

    // Interactive swarm selection if multiple
    const selectedSwarm = swarms.length === 1
      ? swarms[0]
      : await this.promptService.select({
          message: 'Select swarm to inspect:',
          choices: swarms.map(s => ({
            name: `${s.name} (${s.agents.length} agents, ${s.topology})`,
            value: s
          }))
        });

    return this.displaySwarmStatus(selectedSwarm);
  }
}
```

### Learning Commands Module
```typescript
// src/cli/commands/learning/learning.command.ts
@Command({
  name: 'learning',
  description: 'Learning system management and optimization',
  category: 'intelligence'
})
export class LearningCommand {
  constructor(
    private learningService: IntegratedLearningService,
    private promptService: InteractivePromptService
  ) {}

  @SubCommand('start')
  @Option('--algorithm', 'RL algorithm to use', 'auto')
  @Option('--tier', 'Learning tier (basic|standard|advanced)', 'standard')
  async start(options: LearningStartOptions): Promise<CommandResult> {
    // Auto-detect optimal algorithm if not specified
    if (options.algorithm === 'auto') {
      const taskContext = await this.analyzeCurrentContext();
      options.algorithm = this.learningService.selectOptimalAlgorithm(taskContext);

      console.log(`🧠 Auto-selected ${options.algorithm} algorithm based on context`);
    }

    const session = await this.learningService.startSession({
      algorithm: options.algorithm,
      tier: options.tier,
      userId: await this.getCurrentUser()
    });

    return CommandResult.success({
      message: `🚀 Learning session started with ${options.algorithm}`,
      data: { sessionId: session.id, algorithm: options.algorithm, tier: options.tier }
    });
  }

  @SubCommand('feedback')
  @Arg('reward', 'Reward value (0-1)', 'number')
  async feedback(
    @Arg('reward') reward: number,
    @Option('--context', 'Additional context for learning')
    context?: string
  ): Promise<CommandResult> {
    const activeSession = await this.learningService.getActiveSession();
    if (!activeSession) {
      return CommandResult.error('No active learning session found. Start one with `learning start`');
    }

    await this.learningService.submitFeedback({
      sessionId: activeSession.id,
      reward,
      context,
      timestamp: new Date()
    });

    return CommandResult.success({
      message: `📊 Feedback recorded (reward: ${reward})`,
      data: { reward, sessionId: activeSession.id }
    });
  }

  @SubCommand('metrics')
  async metrics(): Promise<CommandResult> {
    const metrics = await this.learningService.getMetrics();

    // Interactive metrics display
    await this.displayInteractiveMetrics(metrics);

    return CommandResult.success('Metrics displayed');
  }
}
```

## Interactive Prompt System

### Advanced Prompt Service
```typescript
// src/cli/services/interactive-prompt.service.ts
interface PromptOptions {
  message: string;
  type: 'select' | 'multiselect' | 'input' | 'confirm' | 'progress';
  choices?: PromptChoice[];
  default?: any;
  validate?: (input: any) => boolean | string;
  transform?: (input: any) => any;
}

export class InteractivePromptService {
  private inquirer: any; // Dynamic import for tree-shaking

  async select<T>(options: SelectPromptOptions<T>): Promise<T> {
    const { default: inquirer } = await import('inquirer');

    const result = await inquirer.prompt([{
      type: 'list',
      name: 'selection',
      message: options.message,
      choices: options.choices,
      default: options.default
    }]);

    return result.selection;
  }

  async multiSelect<T>(options: MultiSelectPromptOptions<T>): Promise<T[]> {
    const { default: inquirer } = await import('inquirer');

    const result = await inquirer.prompt([{
      type: 'checkbox',
      name: 'selections',
      message: options.message,
      choices: options.choices,
      validate: (input: T[]) => {
        if (options.minSelections && input.length < options.minSelections) {
          return `Please select at least ${options.minSelections} options`;
        }
        if (options.maxSelections && input.length > options.maxSelections) {
          return `Please select at most ${options.maxSelections} options`;
        }
        return true;
      }
    }]);

    return result.selections;
  }

  async input(options: InputPromptOptions): Promise<string> {
    const { default: inquirer } = await import('inquirer');

    const result = await inquirer.prompt([{
      type: 'input',
      name: 'input',
      message: options.message,
      default: options.default,
      validate: options.validate,
      transformer: options.transform
    }]);

    return result.input;
  }

  async progressTask<T>(
    task: ProgressTask<T>,
    options: ProgressOptions
  ): Promise<T> {
    const { default: cliProgress } = await import('cli-progress');

    const progressBar = new cliProgress.SingleBar({
      format: `${options.title} |{bar}| {percentage}% | {status}`,
      barCompleteChar: '█',
      barIncompleteChar: '░',
      hideCursor: true
    });

    progressBar.start(100, 0, { status: 'Starting...' });

    try {
      const result = await task({
        updateProgress: (percent: number, status?: string) => {
          progressBar.update(percent, { status: status || 'Processing...' });
        }
      });

      progressBar.update(100, { status: 'Complete!' });
      progressBar.stop();

      return result;
    } catch (error) {
      progressBar.stop();
      throw error;
    }
  }

  async confirmWithDetails(
    message: string,
    details: ConfirmationDetails
  ): Promise<boolean> {
    console.log('\n' + chalk.bold(message));
    console.log(chalk.gray('Details:'));

    for (const [key, value] of Object.entries(details)) {
      console.log(chalk.gray(`  ${key}: ${value}`));
    }

    return this.confirm('\nProceed?');
  }
}
```

## Enhanced Hooks Integration

### Deep CLI Hooks Integration
```typescript
// src/cli/hooks/cli-hooks-manager.ts
interface CLIHookEvent {
  type: 'command_start' | 'command_end' | 'command_error' | 'agent_spawn' | 'task_complete';
  command: string;
  args: string[];
  context: ExecutionContext;
  timestamp: Date;
}

export class CLIHooksManager {
  private hooks: Map<string, HookHandler[]> = new Map();
  private learningIntegration: LearningHooksIntegration;

  constructor() {
    this.learningIntegration = new LearningHooksIntegration();
    this.setupDefaultHooks();
  }

  private setupDefaultHooks(): void {
    // Learning integration hooks
    this.registerHook('command_start', async (event: CLIHookEvent) => {
      await this.learningIntegration.recordCommandStart(event);
    });

    this.registerHook('command_end', async (event: CLIHookEvent) => {
      await this.learningIntegration.recordCommandSuccess(event);
    });

    this.registerHook('command_error', async (event: CLIHookEvent) => {
      await this.learningIntegration.recordCommandError(event);
    });

    // Intelligent suggestions
    this.registerHook('command_start', async (event: CLIHookEvent) => {
      const suggestions = await this.generateIntelligentSuggestions(event);
      if (suggestions.length > 0) {
        this.displaySuggestions(suggestions);
      }
    });

    // Performance monitoring
    this.registerHook('command_end', async (event: CLIHookEvent) => {
      await this.recordPerformanceMetrics(event);
    });
  }

  async executeHooks(type: string, event: CLIHookEvent): Promise<void> {
    const handlers = this.hooks.get(type) || [];

    await Promise.all(handlers.map(handler =>
      this.executeHookSafely(handler, event)
    ));
  }

  private async generateIntelligentSuggestions(event: CLIHookEvent): Promise<Suggestion[]> {
    const context = await this.learningIntegration.getExecutionContext(event);
    const patterns = await this.learningIntegration.findSimilarPatterns(context);

    return patterns.map(pattern => ({
      type: 'optimization',
      message: `Based on similar executions, consider: ${pattern.suggestion}`,
      confidence: pattern.confidence
    }));
  }
}
```

### Learning Integration
```typescript
// src/cli/hooks/learning-hooks-integration.ts
export class LearningHooksIntegration {
  constructor(
    private agenticFlowHooks: AgenticFlowHooksClient,
    private agentDBLearning: AgentDBLearningClient
  ) {}

  async recordCommandStart(event: CLIHookEvent): Promise<void> {
    // Start trajectory tracking
    await this.agenticFlowHooks.trajectoryStart({
      sessionId: event.context.sessionId,
      command: event.command,
      args: event.args,
      context: event.context
    });

    // Record experience in AgentDB
    await this.agentDBLearning.recordExperience({
      type: 'command_execution',
      state: this.encodeCommandState(event),
      action: event.command,
      timestamp: event.timestamp
    });
  }

  async recordCommandSuccess(event: CLIHookEvent): Promise<void> {
    const executionTime = Date.now() - event.timestamp.getTime();
    const reward = this.calculateReward(event, executionTime, true);

    // Complete trajectory
    await this.agenticFlowHooks.trajectoryEnd({
      sessionId: event.context.sessionId,
      success: true,
      reward,
      verdict: 'positive'
    });

    // Submit feedback to learning system
    await this.agentDBLearning.submitFeedback({
      sessionId: event.context.learningSessionId,
      reward,
      success: true,
      latencyMs: executionTime
    });

    // Store successful pattern
    if (reward > 0.8) {
      await this.agenticFlowHooks.storePattern({
        pattern: event.command,
        solution: event.context.result,
        confidence: reward
      });
    }
  }

  async recordCommandError(event: CLIHookEvent): Promise<void> {
    const executionTime = Date.now() - event.timestamp.getTime();
    const reward = this.calculateReward(event, executionTime, false);

    // Complete trajectory with error
    await this.agenticFlowHooks.trajectoryEnd({
      sessionId: event.context.sessionId,
      success: false,
      reward,
      verdict: 'negative',
      error: event.context.error
    });

    // Learn from failure
    await this.agentDBLearning.submitFeedback({
      sessionId: event.context.learningSessionId,
      reward,
      success: false,
      latencyMs: executionTime,
      error: event.context.error
    });
  }

  private calculateReward(event: CLIHookEvent, executionTime: number, success: boolean): number {
    if (!success) return 0;

    // Base reward for success
    let reward = 0.5;

    // Performance bonus (faster execution)
    const expectedTime = this.getExpectedExecutionTime(event.command);
    if (executionTime < expectedTime) {
      reward += 0.3 * (1 - executionTime / expectedTime);
    }

    // Complexity bonus
    const complexity = this.calculateCommandComplexity(event);
    reward += complexity * 0.2;

    return Math.min(reward, 1.0);
  }
}
```

## Intelligent Workflow Automation

### Workflow Orchestrator
```typescript
// src/cli/workflows/workflow-orchestrator.ts
interface WorkflowStep {
  id: string;
  command: string;
  args: string[];
  dependsOn: string[];
  condition?: WorkflowCondition;
  retryPolicy?: RetryPolicy;
}

export class WorkflowOrchestrator {
  constructor(
    private commandRegistry: ModularCommandRegistry,
    private promptService: InteractivePromptService
  ) {}

  async executeWorkflow(workflow: Workflow): Promise<WorkflowResult> {
    const context = new WorkflowExecutionContext(workflow);

    // Display workflow overview
    await this.displayWorkflowOverview(workflow);

    const confirmed = await this.promptService.confirm(
      'Execute this workflow?'
    );

    if (!confirmed) {
      return WorkflowResult.cancelled();
    }

    // Execute steps
    return this.promptService.progressTask(
      async ({ updateProgress }) => {
        const steps = this.sortStepsByDependencies(workflow.steps);

        for (let i = 0; i < steps.length; i++) {
          const step = steps[i];
          updateProgress((i / steps.length) * 100, `Executing ${step.command}`);

          await this.executeStep(step, context);
        }

        return WorkflowResult.success(context.getResults());
      },
      { title: `Workflow: ${workflow.name}` }
    );
  }

  async generateWorkflowFromIntent(intent: string): Promise<Workflow> {
    // Use learning system to generate workflow
    const patterns = await this.findWorkflowPatterns(intent);

    if (patterns.length === 0) {
      throw new Error('Could not generate workflow for intent');
    }

    // Select best pattern or let user choose
    const selectedPattern = patterns.length === 1
      ? patterns[0]
      : await this.promptService.select({
          message: 'Select workflow template:',
          choices: patterns.map(p => ({
            name: `${p.name} (${p.confidence}% match)`,
            value: p
          }))
        });

    return this.customizeWorkflow(selectedPattern, intent);
  }

  private async executeStep(step: WorkflowStep, context: WorkflowExecutionContext): Promise<void> {
    // Check conditions
    if (step.condition && !this.evaluateCondition(step.condition, context)) {
      context.skipStep(step.id, 'Condition not met');
      return;
    }

    // Check dependencies
    const missingDeps = step.dependsOn.filter(dep => !context.isStepCompleted(dep));
    if (missingDeps.length > 0) {
      throw new WorkflowError(`Step ${step.id} has unmet dependencies: ${missingDeps.join(', ')}`);
    }

    // Execute with retry policy
    const retryPolicy = step.retryPolicy || { maxAttempts: 1 };
    let lastError: Error | null = null;

    for (let attempt = 1; attempt <= retryPolicy.maxAttempts; attempt++) {
      try {
        const result = await this.commandRegistry.executeCommand(step.command, step.args);
        context.completeStep(step.id, result);
        return;
      } catch (error) {
        lastError = error as Error;

        if (attempt < retryPolicy.maxAttempts) {
          await this.delay(retryPolicy.backoffMs || 1000);
        }
      }
    }

    throw new WorkflowError(`Step ${step.id} failed after ${retryPolicy.maxAttempts} attempts: ${lastError?.message}`);
  }
}
```

## Performance Optimization

### Command Performance Monitoring
```typescript
// src/cli/performance/command-performance.ts
export class CommandPerformanceMonitor {
  private metrics = new Map<string, CommandMetrics>();

  async measureCommand<T>(
    commandName: string,
    executor: () => Promise<T>
  ): Promise<T> {
    const start = performance.now();
    const memBefore = process.memoryUsage();

    try {
      const result = await executor();
      const end = performance.now();
      const memAfter = process.memoryUsage();

      this.recordMetrics(commandName, {
        executionTime: end - start,
        memoryDelta: memAfter.heapUsed - memBefore.heapUsed,
        success: true
      });

      return result;
    } catch (error) {
      const end = performance.now();

      this.recordMetrics(commandName, {
        executionTime: end - start,
        memoryDelta: 0,
        success: false,
        error: error as Error
      });

      throw error;
    }
  }

  private recordMetrics(command: string, measurement: PerformanceMeasurement): void {
    if (!this.metrics.has(command)) {
      this.metrics.set(command, new CommandMetrics(command));
    }

    const metrics = this.metrics.get(command)!;
    metrics.addMeasurement(measurement);

    // Alert if performance degrades
    if (metrics.getP95ExecutionTime() > 5000) { // 5 seconds
      console.warn(`⚠️  Command '${command}' is performing slowly (P95: ${metrics.getP95ExecutionTime()}ms)`);
    }
  }

  getCommandReport(command: string): PerformanceReport {
    const metrics = this.metrics.get(command);
    if (!metrics) {
      throw new Error(`No metrics found for command: ${command}`);
    }

    return {
      command,
      totalExecutions: metrics.getTotalExecutions(),
      successRate: metrics.getSuccessRate(),
      avgExecutionTime: metrics.getAverageExecutionTime(),
      p95ExecutionTime: metrics.getP95ExecutionTime(),
      avgMemoryUsage: metrics.getAverageMemoryUsage(),
      recommendations: this.generateRecommendations(metrics)
    };
  }
}
```

## Smart Auto-completion

### Intelligent Command Completion
```typescript
// src/cli/completion/intelligent-completion.ts
export class IntelligentCompletion {
  constructor(
    private learningService: LearningService,
    private commandRegistry: ModularCommandRegistry
  ) {}

  async generateCompletions(
    partial: string,
    context: CompletionContext
  ): Promise<Completion[]> {
    const completions: Completion[] = [];

    // 1. Exact command matches
    const exactMatches = this.commandRegistry.findCommandsByPrefix(partial);
    completions.push(...exactMatches.map(cmd => ({
      value: cmd.name,
      description: cmd.description,
      type: 'command',
      confidence: 1.0
    })));

    // 2. Learning-based suggestions
    const learnedSuggestions = await this.learningService.suggestCommands(
      partial,
      context
    );
    completions.push(...learnedSuggestions);

    // 3. Context-aware suggestions
    const contextualSuggestions = await this.generateContextualSuggestions(
      partial,
      context
    );
    completions.push(...contextualSuggestions);

    // Sort by confidence and relevance
    return completions
      .sort((a, b) => b.confidence - a.confidence)
      .slice(0, 10); // Top 10 suggestions
  }

  private async generateContextualSuggestions(
    partial: string,
    context: CompletionContext
  ): Promise<Completion[]> {
    const suggestions: Completion[] = [];

    // If in git repository, suggest git-related commands
    if (context.isGitRepository) {
      if (partial.startsWith('git')) {
        suggestions.push({
          value: 'git commit',
          description: 'Create git commit with generated message',
          type: 'workflow',
          confidence: 0.8
        });
      }
    }

    // If package.json exists, suggest npm commands
    if (context.hasPackageJson) {
      if (partial.startsWith('npm') || partial.startsWith('swarm')) {
        suggestions.push({
          value: 'swarm init',
          description: 'Initialize swarm for this project',
          type: 'workflow',
          confidence: 0.9
        });
      }
    }

    return suggestions;
  }
}
```

## Success Metrics

### CLI Performance Targets
- [ ] **Command Response**: <200ms average command execution time
- [ ] **File Decomposition**: index.ts (108KB) → <10KB per command module
- [ ] **Interactive UX**: Smart prompts with context awareness
- [ ] **Hook Integration**: Deep lifecycle integration with learning
- [ ] **Workflow Automation**: Intelligent multi-step command orchestration
- [ ] **Auto-completion**: >90% accuracy for command suggestions

### User Experience Improvements
```typescript
const cliImprovements = {
  before: {
    commandResponse: '~500ms',
    interactivity: 'Basic command parsing',
    workflows: 'Manual command chaining',
    suggestions: 'Static help text'
  },

  after: {
    commandResponse: '<200ms with caching',
    interactivity: 'Smart context-aware prompts',
    workflows: 'Automated multi-step execution',
    suggestions: 'Learning-based intelligent completion'
  }
};
```

## Related V3 Skills

- `v3-core-implementation` - Core domain integration
- `v3-memory-unification` - Memory-backed command caching
- `v3-swarm-coordination` - CLI swarm management integration
- `v3-performance-optimization` - CLI performance monitoring

## Usage Examples

### Complete CLI Modernization
```bash
# Full CLI modernization implementation
Task("CLI modernization implementation",
     "Implement modular commands, interactive prompts, and intelligent workflows",
     "cli-hooks-developer")
```

### Interactive Command Enhancement
```bash
# Enhanced interactive commands
claude-flow swarm init --interactive
claude-flow learning start --guided
claude-flow workflow create --from-intent "setup new project"
```