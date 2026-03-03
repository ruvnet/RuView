Psycho-Symbolic Reasoning Framework with Rust WASM and FastMCP Integration
Overview: Bridging Symbolic Reasoning with Modern AI
A psycho-symbolic reasoning framework combines classical symbolic AI techniques (like logic-based graph reasoning and rule-based planning) with psychological context (preferences, affect/emotion). The goal is to build more autonomous agents that can plan and reason about tasks while accounting for user preferences or emotional state. In today’s AI landscape dominated by deep learning, such symbolic planning provides essential structure and verifiability for truly autonomous systems
docs.rs
. To implement this, we leverage Rust for performance-critical symbolic algorithms, compile them to WebAssembly (WASM) for portability and sandboxing, and orchestrate everything via a TypeScript layer. The TypeScript side uses FastMCP – a framework based on the Model Context Protocol (MCP) – to connect our reasoning tools with an AI agent (e.g. an LLM) in a secure, standardized way
gofastmcp.com
. In effect, MCP acts like a uniform API “USB-C for AI” that exposes our symbolic tools (as MCP tools) to the agent
gofastmcp.com
. We will outline each component (symbolic graph reasoner, preference/affect extractors, and rule-based planner), how Rust/WASM and TS share responsibilities, and provide patterns for interop, memory management, CLI usage, and security.
Rust Components in WebAssembly: Reasoners, Extractors, and Planners
1. Symbolic Graph Reasoning (Rust/WASM): In Rust we implement a knowledge graph and inference engine. For example, we might represent facts or concepts as nodes and relationships as edges (using a library like petgraph or similar). The Rust module can provide functions to query this graph or derive new facts via logic rules. For instance, given rules like “if X is a subset of Y and Y has property Z, infer X has Z”, the Rust code can traverse the graph and apply these rules. Rust’s strong performance lets us do complex graph searches or logical unification quickly even when compiled to WebAssembly. We expose an entry-point such as fn query_graph(query: String) -> String (or a structured JSON string) that applies symbolic reasoning and returns results (e.g. a chain of inferred relationships or conclusions). 2. Preference & Affect Extractors (Rust/WASM): Another Rust module focuses on extracting user preferences (likes/dislikes, goals) and affect (emotional tone) from data (e.g. text input or user profile). This could use lightweight NLP techniques – for example, a sentiment analysis algorithm (a Rust crate like vader_sentiment or a simple lexicon approach) to gauge affect, and pattern matching or regex to find preference statements (“I prefer X over Y”). These algorithms are implemented in Rust and compiled to WASM, so they run efficiently in a sandboxed environment. The extractors might provide functions like fn extract_affect(text: &str) -> i32 (returning a sentiment score) and fn extract_preferences(text: &str) -> String (returning structured preferences). The psycho aspect comes from using these outputs to modulate the agent’s behavior – e.g. a high negative sentiment could alter the plan or trigger different rules. 3. Rule-Based Planner (Rust/WASM): At the heart is a planner that uses rules to devise or evaluate plans of action. This could be a classical AI planner or a simpler goal-oriented policy engine. For instance, using a library or custom code, we can implement Goal-Oriented Action Planning (GOAP) or state-space search (A*, DFS) to find action sequences that satisfy certain goals under given constraints. The rule base (e.g. “if user is stressed and task is complex, then plan a break”) can be encoded in Rust data structures. The planner function (e.g. fn plan(state: String, goals: String) -> String) takes a description of current state (including facts from the graph and user affect/preferences) and returns a plan or decision. Because this logic is in Rust, we can rigorously enforce that it follows the given rules exactly – making its outputs verifiable and consistent. (It’s even possible to log or prove which rules fired for each plan step, aiding transparency.) We compile all these Rust components to WASM modules. Each module is essentially a portable WebAssembly binary encapsulating the logic. By using Rust’s #[no_mangle] or wasm_bindgen attributes, we ensure the key functions (graph query, extractors, planner) are exported to the WASM interface so they can be called from JavaScript/TypeScript.
Compiling Rust to WebAssembly and Memory Model Coordination
Compiling Rust to WASM is straightforward with modern toolchains. We target either wasm32-unknown-unknown (if our code is pure and doesn’t need OS calls) or wasm32-wasi (if we want POSIX-like system access within the sandbox). Tools like wasm-pack greatly simplify this process, auto-generating JS bindings for Rust functions
medium.com
. For example, by running wasm-pack build --target nodejs in our Rust project, we get an NPM-ready package with our .wasm and a JS wrapper. This wrapper uses wasm-bindgen under the hood to handle data conversion and function invocation, meaning we can call Rust functions as if they were normal JS functions. Memory model: WebAssembly modules have their own linear memory separate from the JS engine. Interop thus involves copying data in and out of this memory (unless using shared memory, which is advanced). Simpler patterns are: pass primitive types or serialize complex data as JSON strings. For instance, our plan(state, goals) might accept a JSON string representing the state; the binding will copy this into WASM memory, call the Rust function, then copy the returned string out. Using JSON/text is straightforward, though for very large data one could also pass pointers (e.g. allocate a buffer in WASM and write into it from JS). In our case, the data sizes (user preferences, graph queries) are manageable as strings. State management: If the reasoning needs to maintain state (like a long-lived knowledge graph), we have two options. One is to load or initialize that state on each invocation (slower but stateless). The other is to keep the WASM module instantiated in memory, with the graph stored in a Rust static or in an in-memory database structure. The JavaScript side can then call multiple functions on the same module instance, which retains its internal state across calls. This approach requires wrapping the module in a long-running process (which is exactly what our CLI or server will do). For example, on startup the TS code can call an initializer like init_graph(data) once, and subsequent calls to query_graph will use the loaded data.
JavaScript/TypeScript Integration and Execution Flow
On the TypeScript side, we integrate these WASM modules using FastMCP tooling. FastMCP (TypeScript) is built atop the official MCP SDK and provides a convenient way to register tools and run an MCP server
github.com
github.com
. We will register our Rust/WASM-powered functions as MCP tools. Each tool has a name, a JSON schema for parameters, and an execute function. In the execute handler, we load or call into the corresponding WASM module:
Loading the WASM: If we used wasm-pack, we can import the generated package. For example: import * as Reasoner from "@myorg/psycho_reasoner";. Loading the module will instantiate the WebAssembly behind the scenes (possibly asynchronously). Alternatively, we can manually use WebAssembly.instantiate if we have the .wasm file and want custom setup (especially if using WASI, where we’d use Node’s WASI class to provide an environment). In either case, we ensure the module is instantiated once at startup (so tools are ready to use).
Registering tools in FastMCP: We create a FastMCP server and add tools like so (TypeScript pseudocode):
import { FastMCP } from "fastmcp";
import * as Reasoner from "@myorg/psycho_reasoner";  // Rust WASM bindings

const server = new FastMCP({ name: "PsychoSymbolicAgent", version: "0.1.0" });

server.addTool({
  name: "queryGraph",
  description: "Symbolic graph reasoning query",
  parameters: { query: "string" },  // simplified schema
  execute: async ({ query }) => {
    return Reasoner.query_graph(query);  // call into WASM
  }
});

server.addTool({
  name: "extractAffect",
  description: "Extract sentiment/affect score from text",
  parameters: { text: "string" },
  execute: async ({ text }) => {
    const score = Reasoner.extract_affect(text);
    return String(score);
  }
});

// ... similarly for preferences and planning tools ...
server.start({ transportType: "stdio" });
Each tool’s execute function calls the Rust WASM function and returns the result (converting to string if needed, since MCP tools expect string outputs). FastMCP takes care of packaging the output into the MCP protocol response.
Execution flow: Once the server is running (e.g. server.start() as above, using stdio transport for CLI use), an AI agent (like an LLM client) can invoke these tools via MCP. For example, the agent might send a JSON request to call queryGraph with certain parameters. The FastMCP TypeScript server receives it, invokes Reasoner.query_graph in WebAssembly, gets the result, and returns it over MCP. If the agent is an LLM, typically it decides when to call tools based on its prompt and the MCP interface; you can also orchestrate a sequence: e.g., first call extractAffect on user input, then feed that result into a planning prompt or directly call planAction tool, etc. Splitting execution between Rust and TS means heavy computation or sensitive logic runs inside the Rust/WASM (maximizing speed and safety), while the TypeScript side handles high-level decision flow, I/O, and integration with the LLM or UI.
Notably, this architecture can run in multiple environments. In a Node.js CLI context, we run the FastMCP server with our tools. But the same WASM modules could run in a browser (using a WASM runtime like Wasmer.js or the browser’s WebAssembly API) with a web-based agent. In fact, one tutorial demonstrates a unified contract where the same WASM AI plugin runs in-browser with Wasmer.js and on server with Wasmer CLI
medium.com
medium.com
 – underscoring that our approach is portable. In our case, using Node for both agent and tools is simplest, but portability is a bonus of WebAssembly.
Patterns for Interop and Memory Management
When calling between JS and WASM, keep these patterns in mind:
Data Conversion: Basic types (numbers, booleans) pass directly. Strings and objects are typically converted. With wasm-bindgen, a Rust function accepting a &str can be called from JS with a normal string; the binding handles allocation and UTF-8 encoding. Likewise, a Rust String return becomes a JS string. Under the hood, memory is allocated in the WASM module for these values – the bindgen runtime usually manages freeing them after the call. If not using bindgen, you’d manually allocate and deallocate via an exported allocator; but using the generated bindings avoids manual memory handling for most cases.
Large Data and Streaming: If your tools needed to handle very large data (imagine a huge knowledge graph or text), consider chunking or streaming. For instance, you might stream data through a shared memory buffer or use WASI to let the WASM read a file directly. Node’s WASI support allows mapping host files or networking with fine-grained permissions. However, streaming adds complexity – in many scenarios, calling functions with moderate JSON payloads is perfectly fine.
Concurrency: WebAssembly (especially WASI and the upcoming component model) can support threading, but JavaScript’s event loop and the single-threaded nature of typical Node scripts mean our default is sequential execution. If the agent needs to do multiple reasoning calls in parallel, you could instantiate multiple WASM instances or use Web Workers (in browser) or Worker Threads (in Node) to run them. Each instance has its own memory, so there’s no data race. This can be useful if, say, you want to extract affect and preferences concurrently. Just be mindful of the overhead of instantiating many WASM modules – reusing a few instances can balance parallelism and resource use.
Error handling: Rust can panic or return Result types – when compiled to WASM, a panic by default aborts the WASM (which would propagate as an error in JS). Using wasm-bindgen, you can catch Rust panics and send them to JS as exceptions. It’s good to design the Rust functions to return error messages or codes so the TS side can handle them gracefully (maybe converting to MCP error responses). FastMCP has error handling patterns built-in
github.com
github.com
, so integrating with that (e.g., returning a JS Error from the tool execute) will inform the calling agent that the tool failed.
CLI Wrapping with NPX and Deployment
To make this framework easily runnable, we package it as a CLI tool. In the project’s package.json, we can specify a bin entry, for example "psycho-agent": "dist/cli.js". The cli.js would simply call the FastMCP server start (as shown above). After publishing to NPM, anyone can run our agent via NPX: npx psycho-agent. This downloads the package and executes cli.js, which in turn instantiates the WASM modules and starts listening (likely on stdio for MCP, which is how e.g. VS Code or other clients could interact, or on an HTTP port if using streamable-http transport). Using NPX for execution means users don’t need to install permanently; it also ensures they get the correct Node and package environment. Our CLI could accept flags, for instance --file scenario.json to load an initial knowledge base, or --transport sse to run an SSE server for browser-based clients. These can be parsed in the Node script and passed into server.start({...}) accordingly. During development, FastMCP’s own CLI (npx fastmcp dev) can be used to test our server as well
github.com
, or we run node cli.js directly. For a trusted web runtime scenario (like running in a browser extension or secure web app), we would skip NPX and instead bundle the WASM and the JS logic into the web app. The fundamental architecture remains the same: Rust/WASM performs the core reasoning, and the JS front-end orchestrates calls, possibly sending results to a UI or a cloud service.
Secure and Verifiable Reasoning with MCP and WASM
Security is a paramount concern for autonomous agents. By using WebAssembly, we gain sandboxing: our Rust code runs in a controlled virtual machine that by default has no access to the host system’s resources. This means even if our reasoning code is complex or untrusted (imagine using third-party plugins), it cannot harm the host or leak data as long as we keep the sandbox restrictions. Projects like HyperMCP and Wassette exemplify this approach – they run AI tools as WASM plugins, limiting their capabilities and ensuring safe execution
github.com
opensource.microsoft.com
. For example, Wassette uses Wasmtime under the hood to enforce a deny-by-default policy on filesystem or network access
opensource.microsoft.com
, and it can prompt for user consent if a tool tries to do something privileged. We can take inspiration from these: for our framework, we might not allow the Rust WASM modules any direct networking or file access at all (unless explicitly needed for, say, reading a knowledge base file). The Node host could provide any needed data instead of the WASM fetching it, keeping the sandbox tight. Using FastMCP also adds security at the protocol level. The MCP server defines exactly what tools (functions) are available to the agent, with typed inputs/outputs. The agent (especially an LLM) is constrained to those tools – it cannot execute arbitrary code outside the provided toolbox. This two-way contract makes the system more predictable and verifiable
gofastmcp.com
. Every tool invocation is an explicit, logged event. If we need verifiable rule application, we can design the Rust planner to output not just a plan but a trace of which rules were applied. Because the planner is implemented in Rust with a fixed rule set, we can trust that it won’t deviate from its rules (unlike an unconstrained LLM which might hallucinate steps). In scenarios requiring high assurance, one could even cryptographically sign the WASM modules or the output plans. In fact, there’s support for signing WASM components (e.g. using Sigstore/Cosign) – Wassette and HyperMCP use OCI registry signatures to ensure a tool wasn’t tampered with
github.com
opensource.microsoft.com
. We could similarly sign our reasoning module so that clients know it’s the authentic logic. This provides a chain of trust: the reasoning results are only as good as the module, and the module’s integrity is verified. Finally, if multi-party computation (MPC) was a concern (for example, combining private user data with a remote rule database without revealing information), that would be an advanced extension. One could theoretically compile a secure MPC protocol into a Rust/WASM module (there are research libraries for MPC) and use FastMCP to orchestrate the exchange of encrypted inputs/outputs between parties. However, this goes beyond a typical single-agent scenario. In most cases, our framework’s security relies on isolation (WASM sandbox) and protocol constraints (MCP tool interface).
Example Scenario and Template
To cement the ideas, consider an autonomous personal assistant agent running in a trusted CLI environment:
Startup: The user runs npx psycho-agent --knowledge base.json. The Node/TS CLI loads the knowledge base JSON from file, starts the FastMCP server, and calls Reasoner.init_graph() passing the JSON data to Rust WASM to build the internal graph model.
Agent Query: The user (or an LLM on behalf of the user) asks, “Should I try to finish the project tonight? I’m feeling pretty tired.” The agent will process this in steps:
It calls the extractAffect tool with the text “I’m feeling pretty tired.” The Rust WASM returns, say, a negative fatigue indicator (e.g. sentiment score -0.5).
It might call a extractPreferences tool on stored user profile (perhaps the user prefers not to work late on weekends – this could be encoded in the knowledge graph too).
Now the TS orchestrator has structured data: fatigue = high, userPrefersWorkLate = false, task = project due date maybe soon (could be retrieved via another resource tool).
TS then invokes the planAction tool, passing in the current state (including those factors). The Rust planner applies its rules: rule example, “if user is tired and not urgent deadline, plan = rest; else if urgent, plan = use a different strategy.” The planner returns a plan or recommendation, e.g. “Plan: get some rest and resume tomorrow morning”.
The agent (if it’s an LLM) takes that plan and forms a final answer to the user, explaining the reasoning (the agent could even cite the rule or preference if designed to do so).
Outcome: The heavy lifting (evaluating rules about fatigue and preferences) was done in Rust/WASM, ensuring consistency. The LLM simply does the natural language part, constrained by the tool outputs. The entire interaction is logged and each tool invocation can be audited.
Throughout this flow, Rust/WASM handled domain-specific logic safely and swiftly, while TypeScript (FastMCP) handled integration, policy (which tool to call when), and communication. This clear split makes the system maintainable and extensible – e.g. new Rust modules can be added as new tools (for instance, an “emotion simulator” or a learning component), and thanks to MCP, an agent can autonomously discover and use them if available.
Conclusion
Implementing a psycho-symbolic reasoning framework with Rust and WebAssembly allows us to combine the reliability of symbolic AI with the flexibility of modern AI systems. Rust ensures performance and memory safety for complex reasoning, WebAssembly provides a portable trusted runtime that can run on the web or in a CLI with strong security guarantees, and FastMCP/TypeScript glue everything together so that an autonomous agent (like an LLM) can leverage these reasoning tools easily. By following the patterns above – defining clear WASM interfaces, using MCP to expose tools, coordinating memory and data across the Rust↔TS boundary – one can create a robust autonomous agent that plans actions, respects user preferences, and accounts for affect, all while running in a secure sandboxed environment. This approach sets the stage for verifiable, modular AI agents: each reasoning module is like a plug-and-play component (even updatable via OCI registries if desired), and the agent’s behavior becomes more interpretable and governable through explicit rules and structured knowledge
docs.rs
opensource.microsoft.com
. By leveraging Rust and WASM in this way, we achieve the best of both worlds: low-level control and high performance for the symbolic reasoning, and high-level ease-of-use and integration through TypeScript and MCP. The framework is ready to be extended to new domains, deployed on various platforms, and trusted to handle critical reasoning tasks in an autonomous fashion. Sources:
FastMCP Documentation – Model Context Protocol overview and FastMCP usage
gofastmcp.com
github.com
HyperMCP (Rust MCP server) – WASM plugin sandboxing features
github.com
github.com
Microsoft Wassette Announcement – Secure Wasm components as AI agent tools (Wasmtime, permissions, signing)
opensource.microsoft.com
opensource.microsoft.com
WASM Radar Tutorial (Enrico Piovesan) – Building portable AI with Rust/WASM, Wasmer, and MCP
medium.com
medium.com
Rusty Planner Docs – Motivation for integrating planning & reasoning in autonomous systems
docs.rs
Citations

rusty_planner - Rust

https://docs.rs/rusty_planner/latest/rusty_planner/

Welcome to FastMCP 2.0! - FastMCP

https://gofastmcp.com/getting-started/welcome

Welcome to FastMCP 2.0! - FastMCP

https://gofastmcp.com/getting-started/welcome

Tutorial: MCP and WebAssembly | Part 2 | by Enrico Piovesan | WebAssembly — WASM Radar | Aug, 2025 | Medium

https://medium.com/wasm-radar/tutorial-mcp-and-webassembly-part-2-a4b038fa5b23

GitHub - punkpeye/fastmcp: A TypeScript framework for building MCP servers.

https://github.com/punkpeye/fastmcp

GitHub - punkpeye/fastmcp: A TypeScript framework for building MCP servers.

https://github.com/punkpeye/fastmcp

Tutorial: MCP and WebAssembly | Part 2 | by Enrico Piovesan | WebAssembly — WASM Radar | Aug, 2025 | Medium

https://medium.com/wasm-radar/tutorial-mcp-and-webassembly-part-2-a4b038fa5b23

Tutorial: MCP and WebAssembly | Part 2 | by Enrico Piovesan | WebAssembly — WASM Radar | Aug, 2025 | Medium

https://medium.com/wasm-radar/tutorial-mcp-and-webassembly-part-2-a4b038fa5b23

GitHub - punkpeye/fastmcp: A TypeScript framework for building MCP servers.

https://github.com/punkpeye/fastmcp

GitHub - punkpeye/fastmcp: A TypeScript framework for building MCP servers.

https://github.com/punkpeye/fastmcp

GitHub - punkpeye/fastmcp: A TypeScript framework for building MCP servers.

https://github.com/punkpeye/fastmcp

GitHub - tuananh/hyper-mcp: ️ A fast, secure MCP server that extends its capabilities through WebAssembly plugins.

https://github.com/tuananh/hyper-mcp

Introducing Wassette: WebAssembly-based tools for AI agents - Microsoft Open Source Blog

https://opensource.microsoft.com/blog/2025/08/06/introducing-wassette-webassembly-based-tools-for-ai-agents/

GitHub - tuananh/hyper-mcp: ️ A fast, secure MCP server that extends its capabilities through WebAssembly plugins.

https://github.com/tuananh/hyper-mcp

Introducing Wassette: WebAssembly-based tools for AI agents - Microsoft Open Source Blog

https://opensource.microsoft.com/blog/2025/08/06/introducing-wassette-webassembly-based-tools-for-ai-agents/
All Sources

docs

gofastmcp

medium

github

opensource.microsoft