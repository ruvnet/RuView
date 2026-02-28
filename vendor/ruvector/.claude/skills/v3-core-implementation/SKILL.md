---
name: "V3 Core Implementation"
description: "Core module implementation for claude-flow v3. Implements DDD domains, clean architecture patterns, dependency injection, and modular TypeScript codebase with comprehensive testing."
---

# V3 Core Implementation

## What This Skill Does

Implements the core TypeScript modules for claude-flow v3 following Domain-Driven Design principles, clean architecture patterns, and modern TypeScript best practices with comprehensive test coverage.

## Quick Start

```bash
# Initialize core implementation
Task("Core foundation", "Set up DDD domain structure and base classes", "core-implementer")

# Domain implementation (parallel)
Task("Task domain", "Implement task management domain with entities and services", "core-implementer")
Task("Session domain", "Implement session management domain", "core-implementer")
Task("Health domain", "Implement health monitoring domain", "core-implementer")
```

## Core Implementation Architecture

### Domain Structure
```
src/
├── core/
│   ├── kernel/                     # Microkernel pattern
│   │   ├── claude-flow-kernel.ts
│   │   ├── domain-registry.ts
│   │   └── plugin-loader.ts
│   │
│   ├── domains/                    # DDD Bounded Contexts
│   │   ├── task-management/
│   │   │   ├── entities/
│   │   │   ├── value-objects/
│   │   │   ├── services/
│   │   │   ├── repositories/
│   │   │   └── events/
│   │   │
│   │   ├── session-management/
│   │   ├── health-monitoring/
│   │   ├── lifecycle-management/
│   │   └── event-coordination/
│   │
│   ├── shared/                     # Shared kernel
│   │   ├── domain/
│   │   │   ├── entity.ts
│   │   │   ├── value-object.ts
│   │   │   ├── domain-event.ts
│   │   │   └── aggregate-root.ts
│   │   │
│   │   ├── infrastructure/
│   │   │   ├── event-bus.ts
│   │   │   ├── dependency-container.ts
│   │   │   └── logger.ts
│   │   │
│   │   └── types/
│   │       ├── common.ts
│   │       ├── errors.ts
│   │       └── interfaces.ts
│   │
│   └── application/                # Application services
│       ├── use-cases/
│       ├── commands/
│       ├── queries/
│       └── handlers/
```

## Base Domain Classes

### Entity Base Class
```typescript
// src/core/shared/domain/entity.ts
export abstract class Entity<T> {
  protected readonly _id: T;
  private _domainEvents: DomainEvent[] = [];

  constructor(id: T) {
    this._id = id;
  }

  get id(): T {
    return this._id;
  }

  public equals(object?: Entity<T>): boolean {
    if (object == null || object == undefined) {
      return false;
    }

    if (this === object) {
      return true;
    }

    if (!(object instanceof Entity)) {
      return false;
    }

    return this._id === object._id;
  }

  protected addDomainEvent(domainEvent: DomainEvent): void {
    this._domainEvents.push(domainEvent);
  }

  public getUncommittedEvents(): DomainEvent[] {
    return this._domainEvents;
  }

  public markEventsAsCommitted(): void {
    this._domainEvents = [];
  }
}
```

### Value Object Base Class
```typescript
// src/core/shared/domain/value-object.ts
export abstract class ValueObject<T> {
  protected readonly props: T;

  constructor(props: T) {
    this.props = Object.freeze(props);
  }

  public equals(object?: ValueObject<T>): boolean {
    if (object == null || object == undefined) {
      return false;
    }

    if (this === object) {
      return true;
    }

    return JSON.stringify(this.props) === JSON.stringify(object.props);
  }

  get value(): T {
    return this.props;
  }
}
```

### Aggregate Root
```typescript
// src/core/shared/domain/aggregate-root.ts
export abstract class AggregateRoot<T> extends Entity<T> {
  private _version: number = 0;

  get version(): number {
    return this._version;
  }

  protected incrementVersion(): void {
    this._version++;
  }

  public applyEvent(event: DomainEvent): void {
    this.addDomainEvent(event);
    this.incrementVersion();
  }
}
```

## Task Management Domain Implementation

### Task Entity
```typescript
// src/core/domains/task-management/entities/task.entity.ts
import { AggregateRoot } from '../../../shared/domain/aggregate-root';
import { TaskId } from '../value-objects/task-id.vo';
import { TaskStatus } from '../value-objects/task-status.vo';
import { Priority } from '../value-objects/priority.vo';
import { TaskAssignedEvent } from '../events/task-assigned.event';

interface TaskProps {
  id: TaskId;
  description: string;
  priority: Priority;
  status: TaskStatus;
  assignedAgentId?: string;
  createdAt: Date;
  updatedAt: Date;
}

export class Task extends AggregateRoot<TaskId> {
  private props: TaskProps;

  private constructor(props: TaskProps) {
    super(props.id);
    this.props = props;
  }

  static create(description: string, priority: Priority): Task {
    const task = new Task({
      id: TaskId.create(),
      description,
      priority,
      status: TaskStatus.pending(),
      createdAt: new Date(),
      updatedAt: new Date()
    });

    return task;
  }

  static reconstitute(props: TaskProps): Task {
    return new Task(props);
  }

  public assignTo(agentId: string): void {
    if (this.props.status.equals(TaskStatus.completed())) {
      throw new Error('Cannot assign completed task');
    }

    this.props.assignedAgentId = agentId;
    this.props.status = TaskStatus.assigned();
    this.props.updatedAt = new Date();

    this.applyEvent(new TaskAssignedEvent(
      this.id.value,
      agentId,
      this.props.priority
    ));
  }

  public complete(result: TaskResult): void {
    if (!this.props.assignedAgentId) {
      throw new Error('Cannot complete unassigned task');
    }

    this.props.status = TaskStatus.completed();
    this.props.updatedAt = new Date();

    this.applyEvent(new TaskCompletedEvent(
      this.id.value,
      result,
      this.calculateDuration()
    ));
  }

  // Getters
  get description(): string { return this.props.description; }
  get priority(): Priority { return this.props.priority; }
  get status(): TaskStatus { return this.props.status; }
  get assignedAgentId(): string | undefined { return this.props.assignedAgentId; }
  get createdAt(): Date { return this.props.createdAt; }
  get updatedAt(): Date { return this.props.updatedAt; }

  private calculateDuration(): number {
    return this.props.updatedAt.getTime() - this.props.createdAt.getTime();
  }
}
```

### Task Value Objects
```typescript
// src/core/domains/task-management/value-objects/task-id.vo.ts
export class TaskId extends ValueObject<string> {
  private constructor(value: string) {
    super({ value });
  }

  static create(): TaskId {
    return new TaskId(crypto.randomUUID());
  }

  static fromString(id: string): TaskId {
    if (!id || id.length === 0) {
      throw new Error('TaskId cannot be empty');
    }
    return new TaskId(id);
  }

  get value(): string {
    return this.props.value;
  }
}

// src/core/domains/task-management/value-objects/task-status.vo.ts
type TaskStatusType = 'pending' | 'assigned' | 'in_progress' | 'completed' | 'failed';

export class TaskStatus extends ValueObject<TaskStatusType> {
  private constructor(status: TaskStatusType) {
    super({ value: status });
  }

  static pending(): TaskStatus { return new TaskStatus('pending'); }
  static assigned(): TaskStatus { return new TaskStatus('assigned'); }
  static inProgress(): TaskStatus { return new TaskStatus('in_progress'); }
  static completed(): TaskStatus { return new TaskStatus('completed'); }
  static failed(): TaskStatus { return new TaskStatus('failed'); }

  get value(): TaskStatusType {
    return this.props.value;
  }

  public isPending(): boolean { return this.value === 'pending'; }
  public isAssigned(): boolean { return this.value === 'assigned'; }
  public isInProgress(): boolean { return this.value === 'in_progress'; }
  public isCompleted(): boolean { return this.value === 'completed'; }
  public isFailed(): boolean { return this.value === 'failed'; }
}

// src/core/domains/task-management/value-objects/priority.vo.ts
type PriorityLevel = 'low' | 'medium' | 'high' | 'critical';

export class Priority extends ValueObject<PriorityLevel> {
  private constructor(level: PriorityLevel) {
    super({ value: level });
  }

  static low(): Priority { return new Priority('low'); }
  static medium(): Priority { return new Priority('medium'); }
  static high(): Priority { return new Priority('high'); }
  static critical(): Priority { return new Priority('critical'); }

  get value(): PriorityLevel {
    return this.props.value;
  }

  public getNumericValue(): number {
    const priorities = { low: 1, medium: 2, high: 3, critical: 4 };
    return priorities[this.value];
  }
}
```

## Domain Services

### Task Scheduling Service
```typescript
// src/core/domains/task-management/services/task-scheduling.service.ts
import { Injectable } from '../../../shared/infrastructure/dependency-container';
import { Task } from '../entities/task.entity';
import { Priority } from '../value-objects/priority.vo';

@Injectable()
export class TaskSchedulingService {
  public prioritizeTasks(tasks: Task[]): Task[] {
    return tasks.sort((a, b) =>
      b.priority.getNumericValue() - a.priority.getNumericValue()
    );
  }

  public canSchedule(task: Task, agentCapacity: number): boolean {
    if (agentCapacity <= 0) return false;

    // Critical tasks always schedulable
    if (task.priority.equals(Priority.critical())) return true;

    // Other logic based on capacity
    return true;
  }

  public calculateEstimatedDuration(task: Task): number {
    // Simple heuristic - would use ML in real implementation
    const baseTime = 300000; // 5 minutes
    const priorityMultiplier = {
      low: 0.5,
      medium: 1.0,
      high: 1.5,
      critical: 2.0
    };

    return baseTime * priorityMultiplier[task.priority.value];
  }
}
```

## Repository Interfaces & Implementations

### Task Repository Interface
```typescript
// src/core/domains/task-management/repositories/task.repository.ts
export interface ITaskRepository {
  save(task: Task): Promise<void>;
  findById(id: TaskId): Promise<Task | null>;
  findByAgentId(agentId: string): Promise<Task[]>;
  findByStatus(status: TaskStatus): Promise<Task[]>;
  findPendingTasks(): Promise<Task[]>;
  delete(id: TaskId): Promise<void>;
}
```

### SQLite Implementation
```typescript
// src/core/domains/task-management/repositories/sqlite-task.repository.ts
@Injectable()
export class SqliteTaskRepository implements ITaskRepository {
  constructor(
    @Inject('Database') private db: Database,
    @Inject('Logger') private logger: ILogger
  ) {}

  async save(task: Task): Promise<void> {
    const sql = `
      INSERT OR REPLACE INTO tasks (
        id, description, priority, status, assigned_agent_id, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)
    `;

    await this.db.run(sql, [
      task.id.value,
      task.description,
      task.priority.value,
      task.status.value,
      task.assignedAgentId,
      task.createdAt.toISOString(),
      task.updatedAt.toISOString()
    ]);

    this.logger.debug(`Task saved: ${task.id.value}`);
  }

  async findById(id: TaskId): Promise<Task | null> {
    const sql = 'SELECT * FROM tasks WHERE id = ?';
    const row = await this.db.get(sql, [id.value]);

    return row ? this.mapRowToTask(row) : null;
  }

  async findPendingTasks(): Promise<Task[]> {
    const sql = 'SELECT * FROM tasks WHERE status = ? ORDER BY priority DESC, created_at ASC';
    const rows = await this.db.all(sql, ['pending']);

    return rows.map(row => this.mapRowToTask(row));
  }

  private mapRowToTask(row: any): Task {
    return Task.reconstitute({
      id: TaskId.fromString(row.id),
      description: row.description,
      priority: Priority.fromString(row.priority),
      status: TaskStatus.fromString(row.status),
      assignedAgentId: row.assigned_agent_id,
      createdAt: new Date(row.created_at),
      updatedAt: new Date(row.updated_at)
    });
  }
}
```

## Application Layer

### Use Case Implementation
```typescript
// src/core/application/use-cases/assign-task.use-case.ts
@Injectable()
export class AssignTaskUseCase {
  constructor(
    @Inject('TaskRepository') private taskRepository: ITaskRepository,
    @Inject('AgentRepository') private agentRepository: IAgentRepository,
    @Inject('DomainEventBus') private eventBus: DomainEventBus,
    @Inject('Logger') private logger: ILogger
  ) {}

  async execute(command: AssignTaskCommand): Promise<AssignTaskResult> {
    try {
      // 1. Validate command
      await this.validateCommand(command);

      // 2. Load aggregates
      const task = await this.taskRepository.findById(command.taskId);
      if (!task) {
        throw new TaskNotFoundError(command.taskId);
      }

      const agent = await this.agentRepository.findById(command.agentId);
      if (!agent) {
        throw new AgentNotFoundError(command.agentId);
      }

      // 3. Business logic
      if (!agent.canAcceptTask(task)) {
        throw new AgentCannotAcceptTaskError(command.agentId, command.taskId);
      }

      task.assignTo(command.agentId);
      agent.acceptTask(task.id);

      // 4. Persist changes
      await Promise.all([
        this.taskRepository.save(task),
        this.agentRepository.save(agent)
      ]);

      // 5. Publish domain events
      const events = [
        ...task.getUncommittedEvents(),
        ...agent.getUncommittedEvents()
      ];

      for (const event of events) {
        await this.eventBus.publish(event);
      }

      task.markEventsAsCommitted();
      agent.markEventsAsCommitted();

      // 6. Return result
      this.logger.info(`Task ${command.taskId.value} assigned to agent ${command.agentId}`);

      return AssignTaskResult.success({
        taskId: task.id,
        agentId: command.agentId,
        assignedAt: new Date()
      });

    } catch (error) {
      this.logger.error(`Failed to assign task ${command.taskId.value}:`, error);
      return AssignTaskResult.failure(error);
    }
  }

  private async validateCommand(command: AssignTaskCommand): Promise<void> {
    if (!command.taskId) {
      throw new ValidationError('Task ID is required');
    }
    if (!command.agentId) {
      throw new ValidationError('Agent ID is required');
    }
  }
}
```

## Dependency Injection Setup

### Container Configuration
```typescript
// src/core/shared/infrastructure/dependency-container.ts
import { Container } from 'inversify';
import { TYPES } from './types';

export class DependencyContainer {
  private container: Container;

  constructor() {
    this.container = new Container();
    this.setupBindings();
  }

  private setupBindings(): void {
    // Repositories
    this.container.bind<ITaskRepository>(TYPES.TaskRepository)
      .to(SqliteTaskRepository)
      .inSingletonScope();

    this.container.bind<IAgentRepository>(TYPES.AgentRepository)
      .to(SqliteAgentRepository)
      .inSingletonScope();

    // Services
    this.container.bind<TaskSchedulingService>(TYPES.TaskSchedulingService)
      .to(TaskSchedulingService)
      .inSingletonScope();

    // Use Cases
    this.container.bind<AssignTaskUseCase>(TYPES.AssignTaskUseCase)
      .to(AssignTaskUseCase)
      .inSingletonScope();

    // Infrastructure
    this.container.bind<ILogger>(TYPES.Logger)
      .to(ConsoleLogger)
      .inSingletonScope();

    this.container.bind<DomainEventBus>(TYPES.DomainEventBus)
      .to(InMemoryDomainEventBus)
      .inSingletonScope();
  }

  get<T>(serviceIdentifier: symbol): T {
    return this.container.get<T>(serviceIdentifier);
  }

  bind<T>(serviceIdentifier: symbol): BindingToSyntax<T> {
    return this.container.bind<T>(serviceIdentifier);
  }
}
```

## Modern TypeScript Configuration

### Strict TypeScript Setup
```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022"],
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "declaration": true,
    "outDir": "./dist",
    "strict": true,
    "exactOptionalPropertyTypes": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitOverride": true,
    "experimentalDecorators": true,
    "emitDecoratorMetadata": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"],
      "@core/*": ["src/core/*"],
      "@shared/*": ["src/core/shared/*"],
      "@domains/*": ["src/core/domains/*"]
    }
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "**/*.test.ts", "**/*.spec.ts"]
}
```

## Testing Implementation

### Domain Unit Tests
```typescript
// src/core/domains/task-management/__tests__/entities/task.entity.test.ts
describe('Task Entity', () => {
  let task: Task;

  beforeEach(() => {
    task = Task.create('Test task', Priority.medium());
  });

  describe('creation', () => {
    it('should create task with pending status', () => {
      expect(task.status.isPending()).toBe(true);
      expect(task.description).toBe('Test task');
      expect(task.priority.equals(Priority.medium())).toBe(true);
    });

    it('should generate unique ID', () => {
      const task1 = Task.create('Task 1', Priority.low());
      const task2 = Task.create('Task 2', Priority.low());

      expect(task1.id.equals(task2.id)).toBe(false);
    });
  });

  describe('assignment', () => {
    it('should assign to agent and change status', () => {
      const agentId = 'agent-123';

      task.assignTo(agentId);

      expect(task.assignedAgentId).toBe(agentId);
      expect(task.status.isAssigned()).toBe(true);
    });

    it('should emit TaskAssignedEvent when assigned', () => {
      const agentId = 'agent-123';

      task.assignTo(agentId);

      const events = task.getUncommittedEvents();
      expect(events).toHaveLength(1);
      expect(events[0]).toBeInstanceOf(TaskAssignedEvent);
    });

    it('should not allow assignment of completed task', () => {
      task.assignTo('agent-123');
      task.complete(TaskResult.success('done'));

      expect(() => task.assignTo('agent-456'))
        .toThrow('Cannot assign completed task');
    });
  });
});
```

### Integration Tests
```typescript
// src/core/domains/task-management/__tests__/integration/task-repository.integration.test.ts
describe('TaskRepository Integration', () => {
  let repository: SqliteTaskRepository;
  let db: Database;

  beforeEach(async () => {
    db = new Database(':memory:');
    await setupTasksTable(db);
    repository = new SqliteTaskRepository(db, new ConsoleLogger());
  });

  afterEach(async () => {
    await db.close();
  });

  it('should save and retrieve task', async () => {
    const task = Task.create('Test task', Priority.high());

    await repository.save(task);
    const retrieved = await repository.findById(task.id);

    expect(retrieved).toBeDefined();
    expect(retrieved!.id.equals(task.id)).toBe(true);
    expect(retrieved!.description).toBe('Test task');
    expect(retrieved!.priority.equals(Priority.high())).toBe(true);
  });

  it('should find pending tasks ordered by priority', async () => {
    const lowTask = Task.create('Low priority', Priority.low());
    const highTask = Task.create('High priority', Priority.high());

    await repository.save(lowTask);
    await repository.save(highTask);

    const pending = await repository.findPendingTasks();

    expect(pending).toHaveLength(2);
    expect(pending[0].id.equals(highTask.id)).toBe(true); // High priority first
    expect(pending[1].id.equals(lowTask.id)).toBe(true);
  });
});
```

## Performance Optimizations

### Entity Caching
```typescript
// src/core/shared/infrastructure/entity-cache.ts
@Injectable()
export class EntityCache<T extends Entity<any>> {
  private cache = new Map<string, { entity: T; timestamp: number }>();
  private readonly ttl: number = 300000; // 5 minutes

  set(id: string, entity: T): void {
    this.cache.set(id, { entity, timestamp: Date.now() });
  }

  get(id: string): T | null {
    const cached = this.cache.get(id);
    if (!cached) return null;

    // Check TTL
    if (Date.now() - cached.timestamp > this.ttl) {
      this.cache.delete(id);
      return null;
    }

    return cached.entity;
  }

  invalidate(id: string): void {
    this.cache.delete(id);
  }

  clear(): void {
    this.cache.clear();
  }
}
```

## Success Metrics

- [ ] **Domain Isolation**: 100% clean dependency boundaries
- [ ] **Test Coverage**: >90% unit test coverage for domain logic
- [ ] **Type Safety**: Strict TypeScript compilation with zero any types
- [ ] **Performance**: <50ms average use case execution time
- [ ] **Memory Efficiency**: <100MB heap usage for core domains
- [ ] **Plugin Architecture**: Modular domain loading capability

## Related V3 Skills

- `v3-ddd-architecture` - DDD architectural design
- `v3-mcp-optimization` - MCP server integration
- `v3-memory-unification` - AgentDB repository integration
- `v3-swarm-coordination` - Swarm domain implementation

## Usage Examples

### Complete Core Implementation
```bash
# Full core module implementation
Task("Core implementation",
     "Implement all core domains with DDD patterns and comprehensive testing",
     "core-implementer")
```

### Domain-Specific Implementation
```bash
# Single domain implementation
Task("Task domain implementation",
     "Implement task management domain with entities, services, and repositories",
     "core-implementer")
```