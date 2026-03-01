---
name: "V3 DDD Architecture"
description: "Domain-Driven Design architecture for claude-flow v3. Implements modular, bounded context architecture with clean separation of concerns and microkernel pattern."
---

# V3 DDD Architecture

## What This Skill Does

Designs and implements Domain-Driven Design (DDD) architecture for claude-flow v3, decomposing god objects into bounded contexts, implementing clean architecture patterns, and enabling modular, testable code structure.

## Quick Start

```bash
# Initialize DDD architecture analysis
Task("Architecture analysis", "Analyze current architecture and design DDD boundaries", "core-architect")

# Domain modeling (parallel)
Task("Domain decomposition", "Break down orchestrator god object into domains", "core-architect")
Task("Context mapping", "Map bounded contexts and relationships", "core-architect")
Task("Interface design", "Design clean domain interfaces", "core-architect")
```

## DDD Implementation Strategy

### Current Architecture Analysis
```
├── PROBLEMATIC: core/orchestrator.ts (1,440 lines - GOD OBJECT)
│   ├── Task management responsibilities
│   ├── Session management responsibilities
│   ├── Health monitoring responsibilities
│   ├── Lifecycle management responsibilities
│   └── Event coordination responsibilities
│
└── TARGET: Modular DDD Architecture
    ├── core/domains/
    │   ├── task-management/
    │   ├── session-management/
    │   ├── health-monitoring/
    │   ├── lifecycle-management/
    │   └── event-coordination/
    └── core/shared/
        ├── interfaces/
        ├── value-objects/
        └── domain-events/
```

### Domain Boundaries

#### 1. Task Management Domain
```typescript
// core/domains/task-management/
interface TaskManagementDomain {
  // Entities
  Task: TaskEntity;
  TaskQueue: TaskQueueEntity;

  // Value Objects
  TaskId: TaskIdVO;
  TaskStatus: TaskStatusVO;
  Priority: PriorityVO;

  // Services
  TaskScheduler: TaskSchedulingService;
  TaskValidator: TaskValidationService;

  // Repository
  TaskRepository: ITaskRepository;
}
```

#### 2. Session Management Domain
```typescript
// core/domains/session-management/
interface SessionManagementDomain {
  // Entities
  Session: SessionEntity;
  SessionState: SessionStateEntity;

  // Value Objects
  SessionId: SessionIdVO;
  SessionStatus: SessionStatusVO;

  // Services
  SessionLifecycle: SessionLifecycleService;
  SessionPersistence: SessionPersistenceService;

  // Repository
  SessionRepository: ISessionRepository;
}
```

#### 3. Health Monitoring Domain
```typescript
// core/domains/health-monitoring/
interface HealthMonitoringDomain {
  // Entities
  HealthCheck: HealthCheckEntity;
  Metric: MetricEntity;

  // Value Objects
  HealthStatus: HealthStatusVO;
  Threshold: ThresholdVO;

  // Services
  HealthCollector: HealthCollectionService;
  AlertManager: AlertManagementService;

  // Repository
  MetricsRepository: IMetricsRepository;
}
```

## Microkernel Architecture Pattern

### Core Kernel
```typescript
// core/kernel/claude-flow-kernel.ts
export class ClaudeFlowKernel {
  private domains: Map<string, Domain> = new Map();
  private eventBus: DomainEventBus;
  private dependencyContainer: Container;

  async initialize(): Promise<void> {
    // Load core domains
    await this.loadDomain('task-management', new TaskManagementDomain());
    await this.loadDomain('session-management', new SessionManagementDomain());
    await this.loadDomain('health-monitoring', new HealthMonitoringDomain());

    // Wire up domain events
    this.setupDomainEventHandlers();
  }

  async loadDomain(name: string, domain: Domain): Promise<void> {
    await domain.initialize(this.dependencyContainer);
    this.domains.set(name, domain);
  }

  getDomain<T extends Domain>(name: string): T {
    const domain = this.domains.get(name);
    if (!domain) {
      throw new DomainNotLoadedError(name);
    }
    return domain as T;
  }
}
```

### Plugin Architecture
```typescript
// core/plugins/
interface DomainPlugin {
  name: string;
  version: string;
  dependencies: string[];

  initialize(kernel: ClaudeFlowKernel): Promise<void>;
  shutdown(): Promise<void>;
}

// Example: Swarm Coordination Plugin
export class SwarmCoordinationPlugin implements DomainPlugin {
  name = 'swarm-coordination';
  version = '3.0.0';
  dependencies = ['task-management', 'session-management'];

  async initialize(kernel: ClaudeFlowKernel): Promise<void> {
    const taskDomain = kernel.getDomain<TaskManagementDomain>('task-management');
    const sessionDomain = kernel.getDomain<SessionManagementDomain>('session-management');

    // Register swarm coordination services
    this.swarmCoordinator = new UnifiedSwarmCoordinator(taskDomain, sessionDomain);
    kernel.registerService('swarm-coordinator', this.swarmCoordinator);
  }
}
```

## Domain Events & Integration

### Event-Driven Communication
```typescript
// core/shared/domain-events/
abstract class DomainEvent {
  public readonly eventId: string;
  public readonly aggregateId: string;
  public readonly occurredOn: Date;
  public readonly eventVersion: number;

  constructor(aggregateId: string) {
    this.eventId = crypto.randomUUID();
    this.aggregateId = aggregateId;
    this.occurredOn = new Date();
    this.eventVersion = 1;
  }
}

// Task domain events
export class TaskAssignedEvent extends DomainEvent {
  constructor(
    taskId: string,
    public readonly agentId: string,
    public readonly priority: Priority
  ) {
    super(taskId);
  }
}

export class TaskCompletedEvent extends DomainEvent {
  constructor(
    taskId: string,
    public readonly result: TaskResult,
    public readonly duration: number
  ) {
    super(taskId);
  }
}

// Event handlers
@EventHandler(TaskCompletedEvent)
export class TaskCompletedHandler {
  constructor(
    private metricsRepository: IMetricsRepository,
    private sessionService: SessionLifecycleService
  ) {}

  async handle(event: TaskCompletedEvent): Promise<void> {
    // Update metrics
    await this.metricsRepository.recordTaskCompletion(
      event.aggregateId,
      event.duration
    );

    // Update session state
    await this.sessionService.markTaskCompleted(
      event.aggregateId,
      event.result
    );
  }
}
```

## Clean Architecture Layers

```typescript
// Architecture layers
┌─────────────────────────────────────────┐
│              Presentation               │  ← CLI, API, UI
├─────────────────────────────────────────┤
│              Application                │  ← Use Cases, Commands
├─────────────────────────────────────────┤
│               Domain                    │  ← Entities, Services, Events
├─────────────────────────────────────────┤
│            Infrastructure               │  ← DB, MCP, External APIs
└─────────────────────────────────────────┘

// Dependency direction: Outside → Inside
// Domain layer has NO external dependencies
```

### Application Layer (Use Cases)
```typescript
// core/application/use-cases/
export class AssignTaskUseCase {
  constructor(
    private taskRepository: ITaskRepository,
    private agentRepository: IAgentRepository,
    private eventBus: DomainEventBus
  ) {}

  async execute(command: AssignTaskCommand): Promise<TaskResult> {
    // 1. Validate command
    await this.validateCommand(command);

    // 2. Load aggregates
    const task = await this.taskRepository.findById(command.taskId);
    const agent = await this.agentRepository.findById(command.agentId);

    // 3. Business logic (in domain)
    task.assignTo(agent);

    // 4. Persist changes
    await this.taskRepository.save(task);

    // 5. Publish domain events
    task.getUncommittedEvents().forEach(event =>
      this.eventBus.publish(event)
    );

    // 6. Return result
    return TaskResult.success(task);
  }
}
```

## Module Configuration

### Bounded Context Modules
```typescript
// core/domains/task-management/module.ts
export const taskManagementModule = {
  name: 'task-management',

  entities: [
    TaskEntity,
    TaskQueueEntity
  ],

  valueObjects: [
    TaskIdVO,
    TaskStatusVO,
    PriorityVO
  ],

  services: [
    TaskSchedulingService,
    TaskValidationService
  ],

  repositories: [
    { provide: ITaskRepository, useClass: SqliteTaskRepository }
  ],

  eventHandlers: [
    TaskAssignedHandler,
    TaskCompletedHandler
  ]
};
```

## Migration Strategy

### Phase 1: Extract Domain Services
```typescript
// Extract services from orchestrator.ts
const extractionPlan = {
  week1: [
    'TaskManager → task-management domain',
    'SessionManager → session-management domain'
  ],
  week2: [
    'HealthMonitor → health-monitoring domain',
    'LifecycleManager → lifecycle-management domain'
  ],
  week3: [
    'EventCoordinator → event-coordination domain',
    'Wire up domain events'
  ]
};
```

### Phase 2: Implement Clean Interfaces
```typescript
// Clean separation with dependency injection
export class TaskController {
  constructor(
    @Inject('AssignTaskUseCase') private assignTask: AssignTaskUseCase,
    @Inject('CompleteTaskUseCase') private completeTask: CompleteTaskUseCase
  ) {}

  async assign(request: AssignTaskRequest): Promise<TaskResponse> {
    const command = AssignTaskCommand.fromRequest(request);
    const result = await this.assignTask.execute(command);
    return TaskResponse.fromResult(result);
  }
}
```

### Phase 3: Plugin System
```typescript
// Enable plugin-based extensions
const pluginSystem = {
  core: ['task-management', 'session-management', 'health-monitoring'],
  optional: ['swarm-coordination', 'learning-integration', 'performance-monitoring']
};
```

## Testing Strategy

### Domain Testing (London School TDD)
```typescript
// Pure domain logic testing
describe('Task Entity', () => {
  let task: TaskEntity;
  let mockAgent: jest.Mocked<AgentEntity>;

  beforeEach(() => {
    task = new TaskEntity(TaskId.create(), 'Test task');
    mockAgent = createMock<AgentEntity>();
  });

  it('should assign to agent when valid', () => {
    mockAgent.canAcceptTask.mockReturnValue(true);

    task.assignTo(mockAgent);

    expect(task.assignedAgent).toBe(mockAgent);
    expect(task.status.value).toBe('assigned');
  });

  it('should emit TaskAssignedEvent when assigned', () => {
    mockAgent.canAcceptTask.mockReturnValue(true);

    task.assignTo(mockAgent);

    const events = task.getUncommittedEvents();
    expect(events).toHaveLength(1);
    expect(events[0]).toBeInstanceOf(TaskAssignedEvent);
  });
});
```

## Success Metrics

- [ ] **God Object Elimination**: orchestrator.ts (1,440 lines) → 5 focused domains (<300 lines each)
- [ ] **Bounded Context Isolation**: 100% domain independence
- [ ] **Plugin Architecture**: Core + optional modules loading
- [ ] **Clean Architecture**: Dependency inversion maintained
- [ ] **Event-Driven Communication**: Loose coupling between domains
- [ ] **Test Coverage**: >90% domain logic coverage

## Related V3 Skills

- `v3-core-implementation` - Implementation of DDD domains
- `v3-memory-unification` - AgentDB integration within bounded contexts
- `v3-swarm-coordination` - Swarm coordination as domain plugin
- `v3-performance-optimization` - Performance optimization across domains

## Usage Examples

### Complete Domain Extraction
```bash
# Full DDD architecture implementation
Task("DDD architecture implementation",
     "Extract orchestrator into DDD domains with clean architecture",
     "core-architect")
```

### Plugin Development
```bash
# Create domain plugin
npm run create:plugin -- --name swarm-coordination --template domain
```