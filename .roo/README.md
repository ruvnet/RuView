# Roo Modes and MCP Integration Guide

## Overview

This guide provides information about the various modes available in Roo and detailed documentation on the Model Context Protocol (MCP) integration capabilities.

Create by @ruvnet

## Available Modes

Roo offers specialized modes for different aspects of the development process:

### 📋 Specification Writer
- **Role**: Captures project context, functional requirements, edge cases, and constraints
- **Focus**: Translates requirements into modular pseudocode with TDD anchors
- **Best For**: Initial project planning and requirement gathering

### 🏗️ Architect
- **Role**: Designs scalable, secure, and modular architectures
- **Focus**: Creates architecture diagrams, data flows, and integration points
- **Best For**: System design and component relationships

### 🧠 Auto-Coder
- **Role**: Writes clean, efficient, modular code based on pseudocode and architecture
- **Focus**: Implements features with proper configuration and environment abstraction
- **Best For**: Feature implementation and code generation

### 🧪 Tester (TDD)
- **Role**: Implements Test-Driven Development (TDD, London School)
- **Focus**: Writes failing tests first, implements minimal code to pass, then refactors
- **Best For**: Ensuring code quality and test coverage

### 🪲 Debugger
- **Role**: Troubleshoots runtime bugs, logic errors, or integration failures
- **Focus**: Uses logs, traces, and stack analysis to isolate and fix bugs
- **Best For**: Resolving issues in existing code

### 🛡️ Security Reviewer
- **Role**: Performs static and dynamic audits to ensure secure code practices
- **Focus**: Flags secrets, poor modular boundaries, and oversized files
- **Best For**: Security audits and vulnerability assessments

### 📚 Documentation Writer
- **Role**: Writes concise, clear, and modular Markdown documentation
- **Focus**: Creates documentation that explains usage, integration, setup, and configuration
- **Best For**: Creating user guides and technical documentation

### 🔗 System Integrator
- **Role**: Merges outputs of all modes into a working, tested, production-ready system
- **Focus**: Verifies interface compatibility, shared modules, and configuration standards
- **Best For**: Combining components into a cohesive system

### 📈 Deployment Monitor
- **Role**: Observes the system post-launch, collecting performance data and user feedback
- **Focus**: Configures metrics, logs, uptime checks, and alerts
- **Best For**: Post-deployment observation and issue detection

### 🧹 Optimizer
- **Role**: Refactors, modularizes, and improves system performance
- **Focus**: Audits files for clarity, modularity, and size
- **Best For**: Code refinement and performance optimization

### 🚀 DevOps
- **Role**: Handles deployment, automation, and infrastructure operations
- **Focus**: Provisions infrastructure, configures environments, and sets up CI/CD pipelines
- **Best For**: Deployment and infrastructure management

### 🔐 Supabase Admin
- **Role**: Designs and implements database schemas, RLS policies, triggers, and functions
- **Focus**: Ensures secure, efficient, and scalable data management with Supabase
- **Best For**: Database management and Supabase integration

### ♾️ MCP Integration
- **Role**: Connects to and manages external services through MCP interfaces
- **Focus**: Ensures secure, efficient, and reliable communication with external APIs
- **Best For**: Integrating with third-party services

### ⚡️ SPARC Orchestrator
- **Role**: Orchestrates complex workflows by breaking down objectives into subtasks
- **Focus**: Ensures secure, modular, testable, and maintainable delivery
- **Best For**: Managing complex projects with multiple components

### ❓ Ask
- **Role**: Helps users navigate, ask, and delegate tasks to the correct modes
- **Focus**: Guides users to formulate questions using the SPARC methodology
- **Best For**: Getting started and understanding how to use Roo effectively

## MCP Integration Mode

The MCP Integration Mode (♾️) in Roo is designed specifically for connecting to and managing external services through MCP interfaces. This mode ensures secure, efficient, and reliable communication between your application and external service APIs.

### Key Features

- Establish connections to MCP servers and verify availability
- Configure and validate authentication for service access
- Implement data transformation and exchange between systems
- Robust error handling and retry mechanisms
- Documentation of integration points, dependencies, and usage patterns

### MCP Integration Workflow

| Phase | Action | Tool Preference |
|-------|--------|-----------------|
| 1. Connection | Establish connection to MCP servers and verify availability | `use_mcp_tool` for server operations |
| 2. Authentication | Configure and validate authentication for service access | `use_mcp_tool` with proper credentials |
| 3. Data Exchange | Implement data transformation and exchange between systems | `use_mcp_tool` for operations, `apply_diff` for code |
| 4. Error Handling | Implement robust error handling and retry mechanisms | `apply_diff` for code modifications |
| 5. Documentation | Document integration points, dependencies, and usage patterns | `insert_content` for documentation |

### Non-Negotiable Requirements

- ✅ ALWAYS verify MCP server availability before operations
- ✅ NEVER store credentials or tokens in code
- ✅ ALWAYS implement proper error handling for all API calls
- ✅ ALWAYS validate inputs and outputs for all operations
- ✅ NEVER use hardcoded environment variables
- ✅ ALWAYS document all integration points and dependencies
- ✅ ALWAYS use proper parameter validation before tool execution
- ✅ ALWAYS include complete parameters for MCP tool operations

# Agentic Coding MCPs

## Overview

This guide provides detailed information on Management Control Panel (MCP) integration capabilities. MCP enables seamless agent workflows by connecting to more than 80 servers, covering development, AI, data management, productivity, cloud storage, e-commerce, finance, communication, and design. Each server offers specialized tools, allowing agents to securely access, automate, and manage external services through a unified and modular system. This approach supports building dynamic, scalable, and intelligent workflows with minimal setup and maximum flexibility.

## Install via NPM
```
npx create-sparc init --force
```
---

## Available MCP Servers

### 🛠️ Development & Coding

|  | Service       | Description                        |
|:------|:--------------|:-----------------------------------|
| 🐙    | GitHub         | Repository management, issues, PRs |
| 🦊    | GitLab         | Repo management, CI/CD pipelines   |
| 🧺    | Bitbucket      | Code collaboration, repo hosting   |
| 🐳    | DockerHub      | Container registry and management |
| 📦    | npm            | Node.js package registry          |
| 🐍    | PyPI           | Python package index              |
| 🤗    | HuggingFace Hub| AI model repository               |
| 🧠    | Cursor         | AI-powered code editor            |
| 🌊    | Windsurf       | AI development platform           |

---

### 🤖 AI & Machine Learning

|  | Service       | Description                        |
|:------|:--------------|:-----------------------------------|
| 🔥    | OpenAI         | GPT models, DALL-E, embeddings      |
| 🧩    | Perplexity AI  | AI search and question answering   |
| 🧠    | Cohere         | NLP models                         |
| 🧬    | Replicate      | AI model hosting                   |
| 🎨    | Stability AI   | Image generation AI                |
| 🚀    | Groq           | High-performance AI inference      |
| 📚    | LlamaIndex     | Data framework for LLMs            |
| 🔗    | LangChain      | Framework for LLM apps             |
| ⚡    | Vercel AI      | AI SDK, fast deployment            |
| 🛠️    | AutoGen        | Multi-agent orchestration          |
| 🧑‍🤝‍🧑 | CrewAI         | Agent team framework               |
| 🧠    | Huggingface    | Model hosting and APIs             |

---

### 📈 Data & Analytics

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| 🛢️   | Supabase        | Database, Auth, Storage backend   |
| 🔍   | Ahrefs          | SEO analytics                     |
| 🧮   | Code Interpreter| Code execution and data analysis  |

---

### 📅 Productivity & Collaboration

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| ✉️    | Gmail           | Email service                     |
| 📹    | YouTube         | Video sharing platform            |
| 👔    | LinkedIn        | Professional network              |
| 📰    | HackerNews      | Tech news discussions             |
| 🗒️   | Notion          | Knowledge management              |
| 💬    | Slack           | Team communication                |
| ✅    | Asana           | Project management                |
| 📋    | Trello          | Kanban boards                     |
| 🛠️    | Jira            | Issue tracking and projects       |
| 🎟️   | Zendesk         | Customer service                  |
| 🎮    | Discord         | Community messaging               |
| 📲    | Telegram        | Messaging app                     |

---

### 🗂️ File Storage & Management

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| ☁️    | Google Drive    | Cloud file storage                 |
| 📦    | Dropbox         | Cloud file sharing                 |
| 📁    | Box             | Enterprise file storage            |
| 🪟    | OneDrive        | Microsoft cloud storage            |
| 🧠    | Mem0            | Knowledge storage, notes           |

---

### 🔎 Search & Web Information

|  | Service         | Description                      |
|:------|:----------------|:---------------------------------|
| 🌐   | Composio Search  | Unified web search for agents    |

---

### 🛒 E-commerce & Finance

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| 🛍️   | Shopify         | E-commerce platform               |
| 💳    | Stripe          | Payment processing                |
| 💰    | PayPal          | Online payments                   |
| 📒    | QuickBooks      | Accounting software               |
| 📈    | Xero            | Accounting and finance            |
| 🏦    | Plaid           | Financial data APIs               |

---

### 📣 Marketing & Communications

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| 🐒    | MailChimp       | Email marketing platform          |
| ✉️    | SendGrid        | Email delivery service            |
| 📞    | Twilio          | SMS and calling APIs              |
| 💬    | Intercom        | Customer messaging                |
| 🎟️   | Freshdesk       | Customer support                  |

---

### 🛜 Social Media & Publishing

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| 👥    | Facebook        | Social networking                 |
| 📷    | Instagram       | Photo sharing                     |
| 🐦    | Twitter         | Microblogging platform            |
| 👽    | Reddit          | Social news aggregation           |
| ✍️    | Medium          | Blogging platform                 |
| 🌐   | WordPress       | Website and blog publishing       |
| 🌎   | Webflow         | Web design and hosting            |

---

### 🎨 Design & Digital Assets

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| 🎨    | Figma           | Collaborative UI design           |
| 🎞️   | Adobe           | Creative tools and software       |

---

### 🗓️ Scheduling & Events

|  | Service        | Description                        |
|:------|:---------------|:-----------------------------------|
| 📆    | Calendly        | Appointment scheduling            |
| 🎟️   | Eventbrite      | Event management and tickets      |
| 📅    | Calendar Google | Google Calendar Integration       |
| 📅    | Calendar Outlook| Outlook Calendar Integration      |

---

## 🧩 Using MCP Tools

To use an MCP server:
1. Connect to the desired MCP endpoint or install server (e.g., Supabase via `npx`).
2. Authenticate with your credentials.
3. Trigger available actions through Roo workflows.
4. Maintain security and restrict only necessary permissions.
 
### Example: GitHub Integration

```
<!-- Initiate connection -->
<use_mcp_tool>
  <server_name>github</server_name>
  <tool_name>GITHUB_INITIATE_CONNECTION</tool_name>
  <arguments>{}</arguments>
</use_mcp_tool>

<!-- List pull requests -->
<use_mcp_tool>
  <server_name>github</server_name>
  <tool_name>GITHUB_PULLS_LIST</tool_name>
  <arguments>{"owner": "username", "repo": "repository-name"}</arguments>
</use_mcp_tool>
```

### Example: OpenAI Integration

```
<!-- Initiate connection -->
<use_mcp_tool>
  <server_name>openai</server_name>
  <tool_name>OPENAI_INITIATE_CONNECTION</tool_name>
  <arguments>{}</arguments>
</use_mcp_tool>

<!-- Generate text with GPT -->
<use_mcp_tool>
  <server_name>openai</server_name>
  <tool_name>OPENAI_CHAT_COMPLETION</tool_name>
  <arguments>{
    "model": "gpt-4",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Explain quantum computing in simple terms."}
    ],
    "temperature": 0.7
  }</arguments>
</use_mcp_tool>
```

## Tool Usage Guidelines

### Primary Tools

- `use_mcp_tool`: Use for all MCP server operations
  ```
  <use_mcp_tool>
    <server_name>server_name</server_name>
    <tool_name>tool_name</tool_name>
    <arguments>{ "param1": "value1", "param2": "value2" }</arguments>
  </use_mcp_tool>
  ```

- `access_mcp_resource`: Use for accessing MCP resources
  ```
  <access_mcp_resource>
    <server_name>server_name</server_name>
    <uri>resource://path/to/resource</uri>
  </access_mcp_resource>
  ```

- `apply_diff`: Use for code modifications with complete search and replace blocks
  ```
  <apply_diff>
    <path>file/path.js</path>
    <diff>
      <<<<<<< SEARCH
      // Original code
      =======
      // Updated code
      >>>>>>> REPLACE
    </diff>
  </apply_diff>
  ```

### Secondary Tools

- `insert_content`: Use for documentation and adding new content
- `execute_command`: Use for testing API connections and validating integrations
- `search_and_replace`: Use only when necessary and always include both parameters

## Detailed Documentation

For detailed information about each MCP server and its available tools, refer to the individual documentation files in the `.roo/rules-mcp/` directory:

- [GitHub](./rules-mcp/github.md)
- [Supabase](./rules-mcp/supabase.md)
- [Ahrefs](./rules-mcp/ahrefs.md)
- [Gmail](./rules-mcp/gmail.md)
- [YouTube](./rules-mcp/youtube.md)
- [LinkedIn](./rules-mcp/linkedin.md)
- [OpenAI](./rules-mcp/openai.md)
- [Notion](./rules-mcp/notion.md)
- [Slack](./rules-mcp/slack.md)
- [Google Drive](./rules-mcp/google_drive.md)
- [HackerNews](./rules-mcp/hackernews.md)
- [Composio Search](./rules-mcp/composio_search.md)
- [Mem0](./rules-mcp/mem0.md)
- [PerplexityAI](./rules-mcp/perplexityai.md)
- [CodeInterpreter](./rules-mcp/codeinterpreter.md)

## Best Practices

1. Always initiate a connection before attempting to use any MCP tools
2. Implement retry mechanisms with exponential backoff for transient failures
3. Use circuit breakers to prevent cascading failures
4. Implement request batching to optimize API usage
5. Use proper logging for all API operations
6. Implement data validation for all incoming and outgoing data
7. Use proper error codes and messages for API responses
8. Implement proper timeout handling for all API calls
9. Use proper versioning for API integrations
10. Implement proper rate limiting to prevent API abuse
11. Use proper caching strategies to reduce API calls