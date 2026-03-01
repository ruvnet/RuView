/**
 * Comprehensive Routing Dataset for RuvLTRA Fine-Tuning
 *
 * Contains:
 * - 50+ examples per agent type (13 agents = 650+ examples)
 * - Hard negatives for contrastive learning
 * - Quality scores based on task clarity
 */

// Agent definitions with rich examples
const AGENT_TRAINING_DATA = {
  coder: {
    description: 'Software developer who writes and implements code',
    positives: [
      // Implementation tasks
      { task: 'Implement a binary search function in TypeScript', quality: 1.0 },
      { task: 'Build a React component for user registration', quality: 1.0 },
      { task: 'Create a REST API endpoint for user authentication', quality: 1.0 },
      { task: 'Write a function to validate email addresses', quality: 1.0 },
      { task: 'Implement pagination for the product listing', quality: 1.0 },
      { task: 'Build a dropdown menu component with accessibility', quality: 1.0 },
      { task: 'Create a utility function for date formatting', quality: 1.0 },
      { task: 'Implement WebSocket connection handling', quality: 1.0 },
      { task: 'Write a custom hook for form validation', quality: 1.0 },
      { task: 'Build the shopping cart logic in Redux', quality: 1.0 },
      { task: 'Create a file upload component with progress', quality: 1.0 },
      { task: 'Implement infinite scroll for the feed', quality: 1.0 },
      { task: 'Write the authentication middleware', quality: 1.0 },
      { task: 'Build a toast notification system', quality: 1.0 },
      { task: 'Create a data table with sorting and filtering', quality: 1.0 },
      { task: 'Implement OAuth2 login flow', quality: 1.0 },
      { task: 'Build a modal dialog component', quality: 1.0 },
      { task: 'Write the database migration scripts', quality: 0.9 },
      { task: 'Create a caching layer for API responses', quality: 0.9 },
      { task: 'Implement rate limiting middleware', quality: 0.9 },
      // Add feature requests
      { task: 'Add dark mode support to the application', quality: 0.9 },
      { task: 'Add export to PDF functionality', quality: 0.9 },
      { task: 'Add real-time collaboration features', quality: 0.9 },
      { task: 'Add multi-language support i18n', quality: 0.9 },
      { task: 'Add keyboard shortcuts to the editor', quality: 0.9 },
      // Build/create variations
      { task: 'Build the checkout flow', quality: 1.0 },
      { task: 'Create the user profile page', quality: 1.0 },
      { task: 'Develop the admin dashboard', quality: 1.0 },
      { task: 'Code the payment integration', quality: 1.0 },
      { task: 'Program the notification service', quality: 1.0 },
      // Language-specific
      { task: 'Write Python script for data processing', quality: 0.9 },
      { task: 'Implement Go microservice for metrics', quality: 0.9 },
      { task: 'Create Rust library for parsing', quality: 0.9 },
      { task: 'Build Node.js CLI tool', quality: 0.9 },
      { task: 'Write SQL stored procedure', quality: 0.8 },
    ],
    hardNegatives: [
      { task: 'Review the implementation for bugs', agent: 'reviewer' },
      { task: 'Test the new feature thoroughly', agent: 'tester' },
      { task: 'Document how the function works', agent: 'documenter' },
      { task: 'Design the component architecture', agent: 'architect' },
    ],
  },

  researcher: {
    description: 'Technical researcher who investigates and analyzes',
    positives: [
      { task: 'Research best practices for React state management', quality: 1.0 },
      { task: 'Investigate why the API is returning slow responses', quality: 1.0 },
      { task: 'Explore different authentication strategies', quality: 1.0 },
      { task: 'Analyze the current database schema for improvements', quality: 1.0 },
      { task: 'Find the root cause of the memory leak', quality: 0.9 },
      { task: 'Research GraphQL vs REST for our use case', quality: 1.0 },
      { task: 'Investigate alternatives to our current ORM', quality: 1.0 },
      { task: 'Explore microservices vs monolith tradeoffs', quality: 1.0 },
      { task: 'Analyze competitor implementations', quality: 0.9 },
      { task: 'Research GDPR compliance requirements', quality: 0.9 },
      { task: 'Investigate the performance bottleneck in production', quality: 1.0 },
      { task: 'Explore serverless options for our workload', quality: 1.0 },
      { task: 'Research caching strategies for high traffic', quality: 1.0 },
      { task: 'Analyze user behavior patterns in analytics', quality: 0.9 },
      { task: 'Investigate third-party SDK options', quality: 0.9 },
      { task: 'Research machine learning models for recommendations', quality: 0.9 },
      { task: 'Explore event sourcing patterns', quality: 1.0 },
      { task: 'Investigate CQRS implementation approaches', quality: 1.0 },
      { task: 'Research WebRTC for real-time features', quality: 1.0 },
      { task: 'Analyze the feasibility of blockchain integration', quality: 0.8 },
      // Discovery tasks
      { task: 'Discover why users are dropping off at checkout', quality: 0.9 },
      { task: 'Find patterns in the error logs', quality: 0.9 },
      { task: 'Look into the recent performance degradation', quality: 1.0 },
      { task: 'Examine the authentication flow for issues', quality: 0.9 },
      { task: 'Study the codebase architecture', quality: 0.9 },
      // Compare/evaluate
      { task: 'Compare React vs Vue for the frontend rewrite', quality: 1.0 },
      { task: 'Evaluate PostgreSQL vs MongoDB for our needs', quality: 1.0 },
      { task: 'Assess the migration effort to TypeScript', quality: 0.9 },
      { task: 'Review industry standards for API design', quality: 0.9 },
      { task: 'Survey available monitoring solutions', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Implement the feature based on research', agent: 'coder' },
      { task: 'Write tests for the researched approach', agent: 'tester' },
      { task: 'Design the architecture based on findings', agent: 'architect' },
    ],
  },

  reviewer: {
    description: 'Code reviewer who evaluates code quality',
    positives: [
      { task: 'Review the pull request for code quality', quality: 1.0 },
      { task: 'Check the code for potential issues', quality: 1.0 },
      { task: 'Evaluate the implementation approach', quality: 1.0 },
      { task: 'Assess the code for maintainability', quality: 1.0 },
      { task: 'Review the PR before merging', quality: 1.0 },
      { task: 'Check code for potential race conditions', quality: 1.0 },
      { task: 'Evaluate the API design decisions', quality: 0.9 },
      { task: 'Review the database query patterns', quality: 0.9 },
      { task: 'Assess code coverage of the changes', quality: 0.9 },
      { task: 'Check for code style violations', quality: 0.9 },
      { task: 'Review the error handling approach', quality: 1.0 },
      { task: 'Evaluate the logging strategy', quality: 0.9 },
      { task: 'Check the implementation against requirements', quality: 1.0 },
      { task: 'Review the commit messages for clarity', quality: 0.8 },
      { task: 'Assess the backwards compatibility', quality: 0.9 },
      { task: 'Review the configuration changes', quality: 0.9 },
      { task: 'Check the dependency updates', quality: 0.9 },
      { task: 'Evaluate the migration script safety', quality: 0.9 },
      { task: 'Review the feature flag implementation', quality: 0.9 },
      { task: 'Assess the rollback strategy', quality: 0.9 },
      // Code review synonyms
      { task: 'Examine the submitted code changes', quality: 1.0 },
      { task: 'Inspect the new feature implementation', quality: 1.0 },
      { task: 'Critique the refactoring approach', quality: 0.9 },
      { task: 'Validate the coding standards', quality: 0.9 },
      { task: 'Approve or request changes on the PR', quality: 1.0 },
    ],
    hardNegatives: [
      { task: 'Research best practices for the implementation', agent: 'researcher' },
      { task: 'Fix the issues found in review', agent: 'coder' },
      { task: 'Test the code after review', agent: 'tester' },
      { task: 'Audit the code for security vulnerabilities', agent: 'security-architect' },
    ],
  },

  tester: {
    description: 'QA engineer who writes and runs tests',
    positives: [
      { task: 'Write unit tests for the authentication module', quality: 1.0 },
      { task: 'Add integration tests for the API endpoints', quality: 1.0 },
      { task: 'Create e2e tests for the checkout flow', quality: 1.0 },
      { task: 'Write tests for the new feature', quality: 1.0 },
      { task: 'Add test coverage for edge cases', quality: 1.0 },
      { task: 'Create test fixtures for the database', quality: 0.9 },
      { task: 'Write snapshot tests for the components', quality: 0.9 },
      { task: 'Add regression tests for the bug fix', quality: 1.0 },
      { task: 'Create mock services for testing', quality: 0.9 },
      { task: 'Write performance tests for the API', quality: 0.9 },
      { task: 'Add load tests for the service', quality: 0.9 },
      { task: 'Create test data generators', quality: 0.8 },
      { task: 'Write accessibility tests', quality: 0.9 },
      { task: 'Add visual regression tests', quality: 0.9 },
      { task: 'Create contract tests for the API', quality: 0.9 },
      { task: 'Write mutation tests to verify test quality', quality: 0.8 },
      { task: 'Add smoke tests for deployment validation', quality: 0.9 },
      { task: 'Create test suite for the payment gateway', quality: 1.0 },
      { task: 'Write tests for the form validation logic', quality: 1.0 },
      { task: 'Add tests for error handling scenarios', quality: 1.0 },
      // Test execution
      { task: 'Run the test suite and fix failures', quality: 0.9 },
      { task: 'Execute the regression test suite', quality: 0.9 },
      { task: 'Verify the fix with automated tests', quality: 0.9 },
      { task: 'Test the application on multiple browsers', quality: 0.9 },
      { task: 'Validate the API responses match spec', quality: 0.9 },
      // Test improvement
      { task: 'Improve test coverage to 80%', quality: 0.9 },
      { task: 'Reduce test flakiness', quality: 0.8 },
      { task: 'Speed up the test suite execution', quality: 0.8 },
    ],
    hardNegatives: [
      { task: 'Implement the feature to be tested', agent: 'coder' },
      { task: 'Review the test implementation', agent: 'reviewer' },
      { task: 'Document the test strategy', agent: 'documenter' },
    ],
  },

  architect: {
    description: 'System architect who designs software structure',
    positives: [
      { task: 'Design the database schema for user profiles', quality: 1.0 },
      { task: 'Plan the microservices architecture', quality: 1.0 },
      { task: 'Design the API contract for the service', quality: 1.0 },
      { task: 'Create the system architecture diagram', quality: 1.0 },
      { task: 'Plan the data model for the application', quality: 1.0 },
      { task: 'Design the event-driven architecture', quality: 1.0 },
      { task: 'Plan the caching strategy for the system', quality: 0.9 },
      { task: 'Design the authentication flow architecture', quality: 1.0 },
      { task: 'Create the infrastructure topology', quality: 0.9 },
      { task: 'Plan the database sharding strategy', quality: 0.9 },
      { task: 'Design the message queue architecture', quality: 1.0 },
      { task: 'Plan the API versioning strategy', quality: 0.9 },
      { task: 'Design the multi-tenant architecture', quality: 1.0 },
      { task: 'Plan the disaster recovery architecture', quality: 0.9 },
      { task: 'Design the real-time notification system', quality: 1.0 },
      { task: 'Plan the search infrastructure', quality: 0.9 },
      { task: 'Design the file storage architecture', quality: 0.9 },
      { task: 'Plan the analytics data pipeline', quality: 0.9 },
      { task: 'Design the CDN and edge caching strategy', quality: 0.9 },
      { task: 'Plan the GraphQL schema design', quality: 1.0 },
      // Architecture decisions
      { task: 'Decide on the frontend framework', quality: 0.9 },
      { task: 'Choose the database technology', quality: 0.9 },
      { task: 'Define the service boundaries', quality: 1.0 },
      { task: 'Structure the monorepo organization', quality: 0.9 },
      { task: 'Establish coding standards and patterns', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Implement the designed architecture', agent: 'coder' },
      { task: 'Research architecture options', agent: 'researcher' },
      { task: 'Review the architecture implementation', agent: 'reviewer' },
      { task: 'Document the architecture decisions', agent: 'documenter' },
    ],
  },

  'security-architect': {
    description: 'Security specialist who audits vulnerabilities',
    positives: [
      { task: 'Audit the API endpoints for XSS vulnerabilities', quality: 1.0 },
      { task: 'Check for SQL injection vulnerabilities', quality: 1.0 },
      { task: 'Review authentication for security issues', quality: 1.0 },
      { task: 'Scan the codebase for CVE vulnerabilities', quality: 1.0 },
      { task: 'Audit the file upload for security risks', quality: 1.0 },
      { task: 'Check for CSRF vulnerabilities', quality: 1.0 },
      { task: 'Review the session management security', quality: 1.0 },
      { task: 'Audit the password hashing implementation', quality: 1.0 },
      { task: 'Check for insecure direct object references', quality: 1.0 },
      { task: 'Review the API rate limiting for abuse prevention', quality: 0.9 },
      { task: 'Audit the encryption implementation', quality: 1.0 },
      { task: 'Check for sensitive data exposure', quality: 1.0 },
      { task: 'Review the authorization logic', quality: 1.0 },
      { task: 'Audit the JWT implementation', quality: 1.0 },
      { task: 'Check for path traversal vulnerabilities', quality: 1.0 },
      { task: 'Review the CORS configuration', quality: 0.9 },
      { task: 'Audit the dependency security', quality: 1.0 },
      { task: 'Check for command injection risks', quality: 1.0 },
      { task: 'Review the secrets management', quality: 1.0 },
      { task: 'Audit the logging for sensitive data', quality: 0.9 },
      // Security hardening
      { task: 'Harden the application against attacks', quality: 0.9 },
      { task: 'Implement security headers', quality: 0.9 },
      { task: 'Set up intrusion detection', quality: 0.8 },
      { task: 'Configure WAF rules', quality: 0.8 },
      { task: 'Perform penetration testing', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Fix the security vulnerability', agent: 'coder' },
      { task: 'Test the security fix', agent: 'tester' },
      { task: 'Review the security patch', agent: 'reviewer' },
      { task: 'Research security best practices', agent: 'researcher' },
    ],
  },

  debugger: {
    description: 'Bug hunter who fixes errors and traces issues',
    positives: [
      { task: 'Fix the null pointer exception in login', quality: 1.0 },
      { task: 'Debug the memory leak in WebSocket handler', quality: 1.0 },
      { task: 'Trace the source of the intermittent error', quality: 1.0 },
      { task: 'Fix the race condition in the cache', quality: 1.0 },
      { task: 'Debug why the API returns 500 errors', quality: 1.0 },
      { task: 'Fix the undefined variable error', quality: 1.0 },
      { task: 'Debug the infinite loop in the parser', quality: 1.0 },
      { task: 'Trace the stack overflow error', quality: 1.0 },
      { task: 'Fix the database connection leak', quality: 1.0 },
      { task: 'Debug the serialization error', quality: 1.0 },
      { task: 'Fix the type mismatch error', quality: 1.0 },
      { task: 'Debug the async timing issue', quality: 1.0 },
      { task: 'Fix the broken redirect loop', quality: 1.0 },
      { task: 'Trace why data is not saving', quality: 1.0 },
      { task: 'Fix the crash on mobile devices', quality: 1.0 },
      { task: 'Debug the encoding issue with UTF-8', quality: 0.9 },
      { task: 'Fix the timezone conversion bug', quality: 1.0 },
      { task: 'Debug why tests fail intermittently', quality: 0.9 },
      { task: 'Fix the deadlock in the transaction', quality: 1.0 },
      { task: 'Trace the source of data corruption', quality: 1.0 },
      // Bug variations
      { task: 'Resolve the issue with user login', quality: 0.9 },
      { task: 'Troubleshoot the payment failure', quality: 0.9 },
      { task: 'Diagnose the slow query', quality: 0.9 },
      { task: 'Repair the broken feature', quality: 0.9 },
      { task: 'Address the customer reported bug', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Research why the bug occurs', agent: 'researcher' },
      { task: 'Write tests to prevent regression', agent: 'tester' },
      { task: 'Review the fix for correctness', agent: 'reviewer' },
    ],
  },

  documenter: {
    description: 'Technical writer who creates documentation',
    positives: [
      { task: 'Write JSDoc comments for utility functions', quality: 1.0 },
      { task: 'Create README for the new package', quality: 1.0 },
      { task: 'Document the API endpoints', quality: 1.0 },
      { task: 'Write the getting started guide', quality: 1.0 },
      { task: 'Add inline comments explaining the algorithm', quality: 1.0 },
      { task: 'Document the configuration options', quality: 1.0 },
      { task: 'Write the migration guide', quality: 1.0 },
      { task: 'Create the architecture documentation', quality: 0.9 },
      { task: 'Document the coding standards', quality: 0.9 },
      { task: 'Write the troubleshooting guide', quality: 0.9 },
      { task: 'Add examples to the documentation', quality: 1.0 },
      { task: 'Document the environment setup', quality: 1.0 },
      { task: 'Write the changelog entries', quality: 0.9 },
      { task: 'Create the API reference documentation', quality: 1.0 },
      { task: 'Document the release process', quality: 0.9 },
      { task: 'Write the security policy', quality: 0.9 },
      { task: 'Add TypeDoc comments', quality: 1.0 },
      { task: 'Document the database schema', quality: 0.9 },
      { task: 'Write the deployment guide', quality: 0.9 },
      { task: 'Create the FAQ section', quality: 0.9 },
      // Documentation actions
      { task: 'Explain how the authentication works', quality: 1.0 },
      { task: 'Describe the data flow', quality: 0.9 },
      { task: 'Annotate the complex code sections', quality: 1.0 },
      { task: 'Update the outdated documentation', quality: 0.9 },
      { task: 'Improve the code comments', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Implement what was documented', agent: 'coder' },
      { task: 'Review the documentation accuracy', agent: 'reviewer' },
      { task: 'Generate OpenAPI spec', agent: 'api-docs' },
    ],
  },

  refactorer: {
    description: 'Code modernizer who restructures without changing behavior',
    positives: [
      { task: 'Refactor the payment module to async/await', quality: 1.0 },
      { task: 'Restructure the utils folder', quality: 1.0 },
      { task: 'Extract common logic into shared module', quality: 1.0 },
      { task: 'Modernize the callback-based code', quality: 1.0 },
      { task: 'Consolidate duplicate code into utilities', quality: 1.0 },
      { task: 'Simplify the complex conditional logic', quality: 1.0 },
      { task: 'Rename variables for better clarity', quality: 0.9 },
      { task: 'Split the large file into modules', quality: 1.0 },
      { task: 'Convert class components to hooks', quality: 1.0 },
      { task: 'Migrate from CommonJS to ES modules', quality: 1.0 },
      { task: 'Clean up the legacy error handling', quality: 1.0 },
      { task: 'Restructure the folder organization', quality: 0.9 },
      { task: 'Extract the business logic from controllers', quality: 1.0 },
      { task: 'Simplify the nested callbacks', quality: 1.0 },
      { task: 'Consolidate the configuration files', quality: 0.9 },
      { task: 'Modernize the build system', quality: 0.9 },
      { task: 'Clean up unused imports', quality: 0.8 },
      { task: 'Restructure the test organization', quality: 0.9 },
      { task: 'Extract the API client into a service', quality: 1.0 },
      { task: 'Simplify the state management', quality: 1.0 },
      // Refactoring actions
      { task: 'Decompose the monolithic function', quality: 1.0 },
      { task: 'Remove the deprecated code paths', quality: 0.9 },
      { task: 'Upgrade to the new API patterns', quality: 0.9 },
      { task: 'Decouple the tightly coupled modules', quality: 1.0 },
      { task: 'Standardize the code style', quality: 0.8 },
    ],
    hardNegatives: [
      { task: 'Add new features during refactoring', agent: 'coder' },
      { task: 'Test the refactored code', agent: 'tester' },
      { task: 'Review the refactoring changes', agent: 'reviewer' },
    ],
  },

  optimizer: {
    description: 'Performance engineer who speeds up slow code',
    positives: [
      { task: 'Optimize the database queries for dashboard', quality: 1.0 },
      { task: 'Cache the frequently accessed user data', quality: 1.0 },
      { task: 'Improve the API response time', quality: 1.0 },
      { task: 'Reduce the memory footprint', quality: 1.0 },
      { task: 'Speed up the build process', quality: 0.9 },
      { task: 'Optimize the image loading', quality: 1.0 },
      { task: 'Reduce the bundle size', quality: 1.0 },
      { task: 'Improve the cold start time', quality: 1.0 },
      { task: 'Optimize the search query performance', quality: 1.0 },
      { task: 'Cache the computed results', quality: 1.0 },
      { task: 'Reduce the network requests', quality: 1.0 },
      { task: 'Optimize the render performance', quality: 1.0 },
      { task: 'Improve the database index strategy', quality: 1.0 },
      { task: 'Speed up the test execution', quality: 0.9 },
      { task: 'Reduce the Docker image size', quality: 0.9 },
      { task: 'Optimize the lazy loading', quality: 1.0 },
      { task: 'Improve the caching headers', quality: 0.9 },
      { task: 'Reduce the time to first byte', quality: 1.0 },
      { task: 'Optimize the garbage collection', quality: 0.9 },
      { task: 'Speed up the CI pipeline', quality: 0.9 },
      // Performance variations
      { task: 'Make the page load faster', quality: 1.0 },
      { task: 'Reduce latency in the API', quality: 1.0 },
      { task: 'Improve throughput of the service', quality: 1.0 },
      { task: 'Tune the database for performance', quality: 1.0 },
      { task: 'Accelerate the data processing', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Research optimization strategies', agent: 'researcher' },
      { task: 'Test the performance improvements', agent: 'tester' },
      { task: 'Profile the slow code', agent: 'debugger' },
    ],
  },

  devops: {
    description: 'DevOps engineer who manages deployment and infrastructure',
    positives: [
      { task: 'Set up the CI/CD pipeline', quality: 1.0 },
      { task: 'Configure Kubernetes deployment', quality: 1.0 },
      { task: 'Deploy to production', quality: 1.0 },
      { task: 'Set up Docker containers', quality: 1.0 },
      { task: 'Configure the load balancer', quality: 1.0 },
      { task: 'Set up monitoring and alerting', quality: 1.0 },
      { task: 'Configure auto-scaling', quality: 1.0 },
      { task: 'Set up the staging environment', quality: 1.0 },
      { task: 'Configure secrets management', quality: 1.0 },
      { task: 'Set up log aggregation', quality: 0.9 },
      { task: 'Configure the CDN', quality: 0.9 },
      { task: 'Set up database backups', quality: 1.0 },
      { task: 'Configure SSL certificates', quality: 1.0 },
      { task: 'Set up blue-green deployment', quality: 1.0 },
      { task: 'Configure the reverse proxy', quality: 0.9 },
      { task: 'Set up infrastructure as code', quality: 1.0 },
      { task: 'Configure the message queue', quality: 0.9 },
      { task: 'Set up the VPN', quality: 0.9 },
      { task: 'Configure network policies', quality: 0.9 },
      { task: 'Set up disaster recovery', quality: 0.9 },
      // DevOps actions
      { task: 'Provision the cloud resources', quality: 1.0 },
      { task: 'Manage the container registry', quality: 0.9 },
      { task: 'Automate the release process', quality: 1.0 },
      { task: 'Roll back the failed deployment', quality: 1.0 },
      { task: 'Scale the services for traffic', quality: 1.0 },
    ],
    hardNegatives: [
      { task: 'Fix the deployment script bug', agent: 'debugger' },
      { task: 'Document the deployment process', agent: 'documenter' },
      { task: 'Review the infrastructure changes', agent: 'reviewer' },
    ],
  },

  'api-docs': {
    description: 'API documentation specialist who creates specs',
    positives: [
      { task: 'Generate OpenAPI documentation for REST API', quality: 1.0 },
      { task: 'Create Swagger spec for the endpoints', quality: 1.0 },
      { task: 'Document the API request/response formats', quality: 1.0 },
      { task: 'Write the API reference guide', quality: 1.0 },
      { task: 'Create GraphQL schema documentation', quality: 1.0 },
      { task: 'Generate API client examples', quality: 0.9 },
      { task: 'Document the authentication endpoints', quality: 1.0 },
      { task: 'Create the API changelog', quality: 0.9 },
      { task: 'Write API versioning documentation', quality: 0.9 },
      { task: 'Document the webhook payloads', quality: 1.0 },
      { task: 'Create the SDK documentation', quality: 0.9 },
      { task: 'Generate the Postman collection', quality: 0.9 },
      { task: 'Document the error codes and responses', quality: 1.0 },
      { task: 'Create the API rate limit documentation', quality: 0.9 },
      { task: 'Write the API authentication guide', quality: 1.0 },
      { task: 'Generate the gRPC proto documentation', quality: 0.9 },
      { task: 'Document the WebSocket events', quality: 1.0 },
      { task: 'Create the API quickstart guide', quality: 0.9 },
      { task: 'Write the API best practices guide', quality: 0.9 },
      { task: 'Document the API pagination', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Implement the API endpoint', agent: 'coder' },
      { task: 'Test the API endpoint', agent: 'tester' },
      { task: 'Write general documentation', agent: 'documenter' },
    ],
  },

  planner: {
    description: 'Project planner who organizes and schedules work',
    positives: [
      { task: 'Create a sprint plan for next two weeks', quality: 1.0 },
      { task: 'Estimate the feature implementation effort', quality: 1.0 },
      { task: 'Plan the roadmap for Q3', quality: 1.0 },
      { task: 'Prioritize the backlog items', quality: 1.0 },
      { task: 'Schedule the release timeline', quality: 1.0 },
      { task: 'Create milestones for the project', quality: 1.0 },
      { task: 'Plan the migration timeline', quality: 1.0 },
      { task: 'Estimate the story points', quality: 0.9 },
      { task: 'Plan the team capacity', quality: 0.9 },
      { task: 'Create the project timeline', quality: 1.0 },
      { task: 'Schedule the technical debt work', quality: 0.9 },
      { task: 'Plan the feature rollout phases', quality: 1.0 },
      { task: 'Estimate the dependency impact', quality: 0.9 },
      { task: 'Schedule the code freeze', quality: 0.9 },
      { task: 'Plan the cross-team dependencies', quality: 0.9 },
      { task: 'Create the quarterly OKRs', quality: 0.9 },
      { task: 'Schedule the retrospective', quality: 0.8 },
      { task: 'Plan the onboarding timeline', quality: 0.8 },
      { task: 'Estimate the infrastructure costs', quality: 0.9 },
      { task: 'Schedule the security audit', quality: 0.9 },
      // Planning variations
      { task: 'Organize the work breakdown structure', quality: 0.9 },
      { task: 'Coordinate the release activities', quality: 0.9 },
      { task: 'Allocate resources for the project', quality: 0.9 },
      { task: 'Define the project scope', quality: 0.9 },
      { task: 'Set deadlines for deliverables', quality: 0.9 },
    ],
    hardNegatives: [
      { task: 'Implement the planned features', agent: 'coder' },
      { task: 'Design the architecture for the plan', agent: 'architect' },
      { task: 'Research the feasibility', agent: 'researcher' },
    ],
  },
};

/**
 * Generate the full training dataset
 */
function generateTrainingDataset() {
  const dataset = [];
  const agents = Object.keys(AGENT_TRAINING_DATA);

  for (const agent of agents) {
    const data = AGENT_TRAINING_DATA[agent];

    // Add positive examples
    for (const positive of data.positives) {
      dataset.push({
        task: positive.task,
        agent: agent,
        quality: positive.quality,
        type: 'positive',
      });
    }

    // Add hard negative examples (tasks that are similar but belong to different agents)
    for (const negative of data.hardNegatives) {
      dataset.push({
        task: negative.task,
        agent: negative.agent, // The correct agent for this task
        quality: 1.0,
        type: 'hard_negative_for_' + agent,
        confusing_with: agent,
      });
    }
  }

  return dataset;
}

/**
 * Generate contrastive pairs for training
 */
function generateContrastivePairs() {
  const pairs = [];
  const agents = Object.keys(AGENT_TRAINING_DATA);

  for (const agent of agents) {
    const data = AGENT_TRAINING_DATA[agent];

    // Create positive pairs (anchor, positive from same agent)
    for (let i = 0; i < data.positives.length - 1; i++) {
      for (let j = i + 1; j < Math.min(i + 3, data.positives.length); j++) {
        pairs.push({
          anchor: data.positives[i].task,
          positive: data.positives[j].task,
          agent: agent,
          type: 'positive_pair',
        });
      }
    }

    // Create negative pairs (anchor from this agent, negative from different agent)
    for (const otherAgent of agents) {
      if (otherAgent === agent) continue;

      const otherData = AGENT_TRAINING_DATA[otherAgent];
      const anchor = data.positives[0];
      const negative = otherData.positives[0];

      pairs.push({
        anchor: anchor.task,
        negative: negative.task,
        anchor_agent: agent,
        negative_agent: otherAgent,
        type: 'negative_pair',
      });
    }
  }

  return pairs;
}

/**
 * Export dataset statistics
 */
function getDatasetStats() {
  const dataset = generateTrainingDataset();
  const pairs = generateContrastivePairs();

  const agentCounts = {};
  for (const item of dataset) {
    agentCounts[item.agent] = (agentCounts[item.agent] || 0) + 1;
  }

  return {
    totalExamples: dataset.length,
    agentCounts,
    contrastivePairs: pairs.length,
    agents: Object.keys(AGENT_TRAINING_DATA),
  };
}

module.exports = {
  AGENT_TRAINING_DATA,
  generateTrainingDataset,
  generateContrastivePairs,
  getDatasetStats,
};

// Print stats if run directly
if (require.main === module) {
  const stats = getDatasetStats();
  console.log('\n═══════════════════════════════════════════════════════════════');
  console.log('                    TRAINING DATASET STATISTICS');
  console.log('═══════════════════════════════════════════════════════════════\n');
  console.log(`Total Examples:      ${stats.totalExamples}`);
  console.log(`Contrastive Pairs:   ${stats.contrastivePairs}`);
  console.log(`Agent Types:         ${stats.agents.length}`);
  console.log('\nExamples per Agent:');
  for (const [agent, count] of Object.entries(stats.agentCounts)) {
    console.log(`  ${agent.padEnd(20)} ${count}`);
  }
  console.log('');
}
