# Pair Programming Examples

Real-world examples and scenarios for pair programming sessions.

## Example 1: Feature Implementation

### Scenario
Implementing a user authentication feature with JWT tokens.

### Session Setup
```bash
claude-flow pair --start \
  --mode switch \
  --agent senior-dev \
  --focus implement \
  --verify \
  --test
```

### Session Flow
```
👥 Starting pair programming for authentication feature...

[DRIVER: You - 10 minutes]
/explain JWT authentication flow
> AI explains JWT concepts and best practices

/suggest implementation approach
> AI suggests using middleware pattern with refresh tokens

# You write the basic auth middleware structure

[SWITCH TO NAVIGATOR]

[NAVIGATOR: AI - 10 minutes]
/implement JWT token generation with refresh tokens
> AI generates secure token implementation

/test-gen
> AI creates comprehensive test suite

[SWITCH TO DRIVER]

[DRIVER: You - 10 minutes]
# You refine the implementation
/review --security
> AI performs security review, suggests improvements

/commit --message "feat: JWT authentication with refresh tokens"
✅ Truth Score: 0.98 - Committed successfully
```

## Example 2: Bug Fixing Session

### Scenario
Debugging a memory leak in a Node.js application.

### Session Setup
```bash
claude-flow pair --start \
  --mode navigator \
  --agent debugger-expert \
  --focus debug \
  --trace
```

### Session Flow
```
👥 Starting debugging session...

/status
> Analyzing application for memory issues...

/perf --profile
> Memory usage growing: 150MB → 450MB over 10 minutes

/find "new EventEmitter" --regex
> Found 3 instances of EventEmitter creation

/inspect eventEmitters --deep
> Discovering listeners not being removed

/suggest fix for memory leak
> AI suggests: "Add removeListener in cleanup functions"

/implement cleanup functions for all event emitters
> AI generates proper cleanup code

/test
> Memory stable at 150MB ✅

/commit --message "fix: memory leak in event emitters"
```

## Example 3: Test-Driven Development

### Scenario
Building a shopping cart feature using TDD.

### Session Setup
```bash
claude-flow pair --start \
  --mode tdd \
  --agent tdd-specialist \
  --test-first
```

### Session Flow
```
👥 TDD Session: Shopping Cart Feature

[RED PHASE]
/test-gen "add item to cart"
> AI writes failing test:
  ✗ should add item to cart
  ✗ should update quantity for existing item
  ✗ should calculate total price

[GREEN PHASE]
/implement minimal cart functionality
> You write just enough code to pass tests

/test
> Tests passing: 3/3 ✅

[REFACTOR PHASE]
/refactor --pattern repository
> AI refactors to repository pattern

/test
> Tests still passing: 3/3 ✅

[NEXT CYCLE]
/test-gen "remove item from cart"
> AI writes new failing tests...
```

## Example 4: Code Refactoring

### Scenario
Refactoring legacy code to modern patterns.

### Session Setup
```bash
claude-flow pair --start \
  --mode driver \
  --focus refactor \
  --verify \
  --threshold 0.98
```

### Session Flow
```
👥 Refactoring Session: Modernizing UserService

/analyze UserService.js
> AI identifies:
  - Callback hell (5 levels deep)
  - No error handling
  - Tight coupling
  - No tests

/suggest refactoring plan
> AI suggests:
  1. Convert callbacks to async/await
  2. Add error boundaries
  3. Extract dependencies
  4. Add unit tests

/test-gen --before-refactor
> AI generates tests for current behavior

/refactor callbacks to async/await
# You refactor with AI guidance

/test
> All tests passing ✅

/review --compare
> AI shows before/after comparison
> Code complexity: 35 → 12
> Truth score: 0.99 ✅

/commit --message "refactor: modernize UserService with async/await"
```

## Example 5: Learning Session

### Scenario
Learning React hooks with AI mentorship.

### Session Setup
```bash
claude-flow pair --start \
  --mode mentor \
  --agent react-expert \
  --pace slow \
  --examples
```

### Session Flow
```
👥 Learning Session: React Hooks

/learn useState hook
> AI explains with interactive examples

/example custom hook for API calls
> AI shows best practice implementation:
```javascript
function useApi(url) {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  
  useEffect(() => {
    // Implementation explained step by step
  }, [url]);
  
  return { data, loading, error };
}
```

/implement my own custom hook
# You write with AI guidance

/review --educational
> AI provides detailed feedback with learning points

/quiz react hooks
> AI tests your understanding
> Score: 8/10 - Good progress!
```

## Example 6: Performance Optimization

### Scenario
Optimizing a slow React application.

### Session Setup
```bash
claude-flow pair --start \
  --mode switch \
  --agent performance-expert \
  --focus optimize \
  --profile
```

### Session Flow
```
👥 Performance Optimization Session

/perf --profile
> React DevTools Profiler Results:
  - ProductList: 450ms render
  - CartSummary: 200ms render
  - Unnecessary re-renders: 15

/suggest optimizations for ProductList
> AI suggests:
  1. Add React.memo
  2. Use useMemo for expensive calculations
  3. Implement virtualization for long lists

/implement React.memo and useMemo
# You implement with AI guidance

/perf --profile
> ProductList: 45ms render (90% improvement!) ✅

/implement virtualization with react-window
> AI implements virtual scrolling

/perf --profile
> ProductList: 12ms render (97% improvement!) ✅
> FPS: 60 stable ✅

/commit --message "perf: optimize ProductList with memoization and virtualization"
```

## Example 7: API Development

### Scenario
Building a RESTful API with Express.

### Session Setup
```bash
claude-flow pair --start \
  --mode navigator \
  --agent backend-expert \
  --focus implement \
  --test
```

### Session Flow
```
👥 API Development Session

/design REST API for blog platform
> AI designs endpoints:
  POST   /api/posts
  GET    /api/posts
  GET    /api/posts/:id
  PUT    /api/posts/:id
  DELETE /api/posts/:id

/implement CRUD endpoints with validation
> AI implements with Express + Joi validation

/test-gen --integration
> AI generates integration tests

/security --api
> AI adds:
  - Rate limiting
  - Input sanitization
  - JWT authentication
  - CORS configuration

/document --openapi
> AI generates OpenAPI documentation

/test --integration
> All endpoints tested: 15/15 ✅

/deploy --staging
> API deployed to staging environment
```

## Example 8: Database Migration

### Scenario
Migrating from MongoDB to PostgreSQL.

### Session Setup
```bash
claude-flow pair --start \
  --mode switch \
  --agent database-expert \
  --verify \
  --test
```

### Session Flow
```
👥 Database Migration Session

/analyze MongoDB schema
> AI maps current structure:
  - users collection → users table
  - posts collection → posts table
  - Embedded comments → comments table with FK

/design PostgreSQL schema
> AI creates normalized schema with relations

/implement migration script
# You write migration with AI assistance

/test --migration --sample-data
> Migration successful for 10,000 records ✅

/implement data access layer
> AI creates repository pattern implementation

/test --integration
> All queries working correctly ✅

/verify data integrity
> Truth score: 0.995 ✅
> No data loss detected
```

## Example 9: CI/CD Pipeline

### Scenario
Setting up GitHub Actions CI/CD pipeline.

### Session Setup
```bash
claude-flow pair --start \
  --mode navigator \
  --agent devops-expert \
  --focus implement
```

### Session Flow
```
👥 CI/CD Pipeline Setup

/implement GitHub Actions workflow
> AI creates .github/workflows/ci.yml:
  - Build on push/PR
  - Run tests
  - Check coverage
  - Deploy to staging

/test --ci --dry-run
> Pipeline simulation successful ✅

/implement deployment to production
> AI adds:
  - Manual approval step
  - Rollback capability
  - Health checks
  - Notifications

/security --scan-pipeline
> AI adds security scanning:
  - Dependency scanning
  - Container scanning
  - Secret scanning

/commit --message "ci: complete CI/CD pipeline with security scanning"
```

## Example 10: Mobile App Development

### Scenario
Building a React Native mobile feature.

### Session Setup
```bash
claude-flow pair --start \
  --mode switch \
  --agent mobile-expert \
  --language react-native \
  --test
```

### Session Flow
```
👥 Mobile Development Session

/implement offline-first data sync
> AI implements:
  - Local SQLite storage
  - Queue for pending changes
  - Sync on connection restore
  - Conflict resolution

/test --device ios simulator
> Feature working on iOS ✅

/test --device android emulator
> Feature working on Android ✅

/optimize --mobile
> AI optimizes:
  - Reduces bundle size by 30%
  - Implements lazy loading
  - Adds image caching

/review --accessibility
> AI ensures:
  - Screen reader support
  - Proper contrast ratios
  - Touch target sizes

/commit --message "feat: offline-first sync with optimizations"
```

## Common Patterns

### Starting Patterns
```bash
# Quick start for common scenarios
claude-flow pair --template <template>
```

Available templates:
- `feature` - New feature development
- `bugfix` - Bug fixing session
- `refactor` - Code refactoring
- `optimize` - Performance optimization
- `test` - Test writing
- `review` - Code review
- `learn` - Learning session

### Session Commands Flow

#### Typical Feature Development
```
/start → /explain → /design → /implement → /test → /review → /commit → /end
```

#### Typical Bug Fix
```
/start → /reproduce → /debug → /trace → /fix → /test → /verify → /commit → /end
```

#### Typical Refactoring
```
/start → /analyze → /plan → /test-gen → /refactor → /test → /review → /commit → /end
```

## Best Practices from Examples

1. **Always Start with Context** - Use `/explain` or `/analyze`
2. **Test Early and Often** - Run tests after each change
3. **Verify Before Commit** - Check truth scores
4. **Document Decisions** - Use `/note` for important choices
5. **Review Security** - Always run `/security` for sensitive code
6. **Profile Performance** - Use `/perf` for optimization
7. **Save Sessions** - Use `/save` for complex work

## Related Documentation

- [Getting Started](./README.md)
- [Session Management](./session.md)
- [Commands Reference](./commands.md)
- [Configuration](./config.md)