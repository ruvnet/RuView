# CI/CD Automation Examples for agentic-synth

Comprehensive examples demonstrating how to integrate agentic-synth into your CI/CD pipelines for automated test data generation.

## Overview

This directory contains production-ready examples for generating synthetic test data in CI/CD environments:

- **test-data-generator.ts** - Generate database fixtures, API mocks, user sessions, load test data, and environment configurations
- **pipeline-testing.ts** - Create dynamic test cases, edge cases, performance tests, security tests, and regression tests

## Quick Start

### Installation

```bash
# Install dependencies
npm install @ruvector/agentic-synth

# Set up environment variables
export GEMINI_API_KEY="your-api-key-here"
# OR
export OPENROUTER_API_KEY="your-api-key-here"
```

### Basic Usage

```typescript
import { CICDTestDataGenerator } from './test-data-generator';

// Generate all test data
const generator = new CICDTestDataGenerator({
  outputDir: './test-fixtures',
  provider: 'gemini',
  seed: 'reproducible-seed'
});

await generator.generateAll();
```

## GitHub Actions Integration

### Example Workflow

Create `.github/workflows/test-data-generation.yml`:

```yaml
name: Generate Test Data

on:
  pull_request:
  push:
    branches: [main]

jobs:
  generate-test-data:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install dependencies
        run: npm ci

      - name: Generate test data
        env:
          GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
          GITHUB_SHA: ${{ github.sha }}
        run: |
          node -e "
          import('./test-data-generator.js').then(async ({ CICDTestDataGenerator }) => {
            const generator = new CICDTestDataGenerator({
              outputDir: './test-fixtures',
              seed: process.env.GITHUB_SHA
            });
            await generator.generateAll();
          });
          "

      - name: Upload test data
        uses: actions/upload-artifact@v4
        with:
          name: test-data
          path: test-fixtures/
          retention-days: 7

      - name: Run tests with generated data
        run: npm test
```

### Parallel Test Generation

```yaml
name: Parallel Test Data Generation

on: [push]

jobs:
  generate:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        data-type: [fixtures, mocks, sessions, performance]

    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Generate ${{ matrix.data-type }} data
        env:
          GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
        run: |
          node generate-${{ matrix.data-type }}.js

      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.data-type }}-data
          path: test-data/
```

## GitLab CI Integration

### Example Pipeline

Create `.gitlab-ci.yml`:

```yaml
stages:
  - generate
  - test
  - deploy

variables:
  TEST_DATA_DIR: test-fixtures

generate-test-data:
  stage: generate
  image: node:20

  before_script:
    - npm ci

  script:
    - |
      node -e "
      import('./test-data-generator.js').then(async ({ CICDTestDataGenerator }) => {
        const generator = new CICDTestDataGenerator({
          outputDir: process.env.TEST_DATA_DIR,
          seed: process.env.CI_COMMIT_SHORT_SHA
        });
        await generator.generateAll({
          users: 100,
          posts: 500,
          apiMocks: 20,
          loadTestRequests: 10000
        });
      });
      "

  artifacts:
    paths:
      - test-fixtures/
    expire_in: 1 week

  cache:
    key: ${CI_COMMIT_REF_SLUG}
    paths:
      - node_modules/

integration-tests:
  stage: test
  dependencies:
    - generate-test-data

  script:
    - npm run test:integration

  coverage: '/Coverage: \d+\.\d+%/'

performance-tests:
  stage: test
  dependencies:
    - generate-test-data

  script:
    - npm run test:performance

  artifacts:
    reports:
      performance: performance-report.json
```

### Multi-Environment Testing

```yaml
.generate-template:
  stage: generate
  image: node:20
  script:
    - |
      node -e "
      import('./test-data-generator.js').then(async ({ CICDTestDataGenerator }) => {
        const generator = new CICDTestDataGenerator({
          outputDir: './test-data',
          seed: process.env.CI_COMMIT_SHA
        });
        await generator.generateEnvironmentConfigs({
          environments: ['${ENVIRONMENT}']
        });
      });
      "
  artifacts:
    paths:
      - test-data/

generate-dev:
  extends: .generate-template
  variables:
    ENVIRONMENT: development

generate-staging:
  extends: .generate-template
  variables:
    ENVIRONMENT: staging

generate-production:
  extends: .generate-template
  variables:
    ENVIRONMENT: production
  only:
    - main
```

## Jenkins Integration

### Example Jenkinsfile

```groovy
pipeline {
    agent any

    environment {
        GEMINI_API_KEY = credentials('gemini-api-key')
        TEST_DATA_DIR = "${WORKSPACE}/test-data"
    }

    stages {
        stage('Setup') {
            steps {
                nodejs(nodeJSInstallationName: 'Node 20') {
                    sh 'npm ci'
                }
            }
        }

        stage('Generate Test Data') {
            steps {
                nodejs(nodeJSInstallationName: 'Node 20') {
                    script {
                        sh """
                        node -e "
                        import('./test-data-generator.js').then(async ({ CICDTestDataGenerator }) => {
                          const generator = new CICDTestDataGenerator({
                            outputDir: process.env.TEST_DATA_DIR,
                            seed: process.env.BUILD_NUMBER
                          });
                          await generator.generateAll();
                        });
                        "
                        """
                    }
                }
            }
        }

        stage('Run Tests') {
            parallel {
                stage('Unit Tests') {
                    steps {
                        sh 'npm run test:unit'
                    }
                }
                stage('Integration Tests') {
                    steps {
                        sh 'npm run test:integration'
                    }
                }
                stage('E2E Tests') {
                    steps {
                        sh 'npm run test:e2e'
                    }
                }
            }
        }
    }

    post {
        always {
            archiveArtifacts artifacts: 'test-data/**', allowEmptyArchive: true
            junit 'test-results/**/*.xml'
        }
        success {
            echo 'Test data generation and tests completed successfully!'
        }
        failure {
            echo 'Test data generation or tests failed!'
        }
    }
}
```

### Multi-Branch Pipeline

```groovy
pipeline {
    agent any

    stages {
        stage('Generate Test Data') {
            steps {
                script {
                    def dataTypes = ['fixtures', 'mocks', 'sessions', 'performance']
                    def jobs = [:]

                    dataTypes.each { dataType ->
                        jobs[dataType] = {
                            node {
                                nodejs(nodeJSInstallationName: 'Node 20') {
                                    sh """
                                    node -e "
                                    import('./test-data-generator.js').then(async ({ CICDTestDataGenerator }) => {
                                      const generator = new CICDTestDataGenerator();
                                      await generator.generate${dataType.capitalize()}();
                                    });
                                    "
                                    """
                                }
                            }
                        }
                    }

                    parallel jobs
                }
            }
        }
    }
}
```

## Advanced Usage

### Custom Test Data Generation

```typescript
import { CICDTestDataGenerator } from './test-data-generator';

const generator = new CICDTestDataGenerator({
  outputDir: './custom-test-data',
  format: 'json',
  provider: 'gemini',
  seed: 'my-seed-123'
});

// Generate specific datasets
await generator.generateDatabaseFixtures({
  users: 50,
  posts: 200,
  comments: 500
});

await generator.generateAPIMockResponses({
  endpoints: ['/api/users', '/api/products'],
  responsesPerEndpoint: 10,
  includeErrors: true
});

await generator.generateLoadTestData({
  requestCount: 100000,
  concurrent: 50,
  duration: 30
});
```

### Pipeline Testing

```typescript
import { PipelineTester } from './pipeline-testing';

const tester = new PipelineTester({
  outputDir: './pipeline-tests',
  seed: process.env.CI_COMMIT_SHA
});

// Generate comprehensive test suite
await tester.generateComprehensiveTestSuite({
  feature: 'authentication',
  testCases: 50,
  edgeCases: 30,
  performanceTests: 20000,
  securityTests: 40
});

// Generate security-specific tests
await tester.generateSecurityTestData({
  attackVectors: ['sql_injection', 'xss', 'csrf'],
  count: 50
});

// Generate performance test data
await tester.generatePerformanceTestData({
  scenario: 'high-load',
  dataPoints: 50000,
  concurrent: true
});
```

### Environment-Specific Configuration

```typescript
import { CICDTestDataGenerator } from './test-data-generator';

const environment = process.env.NODE_ENV || 'development';

const generator = new CICDTestDataGenerator({
  outputDir: `./test-data/${environment}`,
  seed: `${environment}-${Date.now()}`
});

// Generate environment-specific configs
await generator.generateEnvironmentConfigs({
  environments: [environment],
  includeSecrets: environment !== 'production'
});
```

## Best Practices

### 1. Use Reproducible Seeds

Always use deterministic seeds in CI/CD to ensure reproducible test data:

```typescript
const generator = new CICDTestDataGenerator({
  seed: process.env.CI_COMMIT_SHA || process.env.BUILD_NUMBER
});
```

### 2. Cache Generated Data

Cache test data between pipeline runs to speed up execution:

```yaml
# GitHub Actions
- uses: actions/cache@v4
  with:
    path: test-fixtures/
    key: test-data-${{ hashFiles('**/test-schema.json') }}

# GitLab CI
cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - test-fixtures/
```

### 3. Parallelize Generation

Generate different types of test data in parallel for faster pipelines:

```typescript
await Promise.all([
  generator.generateDatabaseFixtures(),
  generator.generateAPIMockResponses(),
  generator.generateUserSessions(),
  generator.generateEnvironmentConfigs()
]);
```

### 4. Validate Generated Data

Always validate generated data before running tests:

```typescript
import { z } from 'zod';

const userSchema = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  username: z.string().min(3)
});

const result = await generator.generateDatabaseFixtures();
result.data.forEach(user => userSchema.parse(user));
```

### 5. Clean Up Test Data

Clean up generated test data after pipeline completion:

```yaml
# GitHub Actions
- name: Cleanup
  if: always()
  run: rm -rf test-fixtures/

# GitLab CI
after_script:
  - rm -rf test-fixtures/
```

## Performance Optimization

### Batch Generation

```typescript
const batchOptions = Array.from({ length: 10 }, (_, i) => ({
  count: 1000,
  schema: mySchema,
  seed: `batch-${i}`
}));

const results = await synth.generateBatch('structured', batchOptions, 5);
```

### Streaming for Large Datasets

```typescript
for await (const dataPoint of synth.generateStream('timeseries', {
  count: 1000000,
  interval: '1s'
})) {
  await processDataPoint(dataPoint);
}
```

### Memory Management

```typescript
const generator = new CICDTestDataGenerator({
  cacheStrategy: 'memory',
  cacheTTL: 3600
});

// Generate in chunks for large datasets
const chunkSize = 10000;
for (let i = 0; i < totalRecords; i += chunkSize) {
  const chunk = await generator.generateDatabaseFixtures({
    users: chunkSize,
    seed: `chunk-${i}`
  });
  await processChunk(chunk);
}
```

## Troubleshooting

### Common Issues

#### 1. API Rate Limiting

```typescript
const generator = new CICDTestDataGenerator({
  maxRetries: 5,
  timeout: 60000
});
```

#### 2. Large Dataset Generation

```typescript
// Use batch generation for large datasets
const results = await synth.generateBatch('structured', batchOptions, 3);
```

#### 3. Memory Issues

```typescript
// Use streaming for very large datasets
for await (const item of synth.generateStream('structured', options)) {
  await processItem(item);
}
```

## Examples

### Complete GitHub Actions Workflow

```yaml
name: CI/CD with Test Data Generation

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  generate-and-test:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Cache test data
        uses: actions/cache@v4
        with:
          path: test-fixtures/
          key: test-data-${{ hashFiles('**/schema.json') }}-${{ github.sha }}
          restore-keys: |
            test-data-${{ hashFiles('**/schema.json') }}-

      - name: Generate test data
        env:
          GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
          GITHUB_SHA: ${{ github.sha }}
        run: npm run generate:test-data

      - name: Run unit tests
        run: npm run test:unit

      - name: Run integration tests
        run: npm run test:integration

      - name: Run E2E tests
        run: npm run test:e2e

      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          files: ./coverage/coverage-final.json

      - name: Upload test data artifact
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: test-data-debug
          path: test-fixtures/
```

## Resources

- [agentic-synth Documentation](../../README.md)
- [GitHub Actions Documentation](https://docs.github.com/actions)
- [GitLab CI Documentation](https://docs.gitlab.com/ee/ci/)
- [Jenkins Documentation](https://www.jenkins.io/doc/)

## Support

For issues or questions:
- Open an issue on [GitHub](https://github.com/ruvnet/ruvector/issues)
- Check the [main documentation](../../README.md)

## License

MIT - See LICENSE file for details
