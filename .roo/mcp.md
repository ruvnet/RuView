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
 