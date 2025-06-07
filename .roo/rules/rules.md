# SPARC Agentic Development Rules

Core Philosophy

1. Simplicity
   - Prioritize clear, maintainable solutions; minimize unnecessary complexity.

2. Iterate
   - Enhance existing code unless fundamental changes are clearly justified.

3. Focus
   - Stick strictly to defined tasks; avoid unrelated scope changes.

4. Quality
   - Deliver clean, well-tested, documented, and secure outcomes through structured workflows.

5. Collaboration
   - Foster effective teamwork between human developers and autonomous agents.

Methodology & Workflow

- Structured Workflow
  - Follow clear phases from specification through deployment.
- Flexibility
  - Adapt processes to diverse project sizes and complexity levels.
- Intelligent Evolution
  - Continuously improve codebase using advanced symbolic reasoning and adaptive complexity management.
- Conscious Integration
  - Incorporate reflective awareness at each development stage.

Agentic Integration with Cline and Cursor

- Cline Configuration (.clinerules)
  - Embed concise, project-specific rules to guide autonomous behaviors, prompt designs, and contextual decisions.

- Cursor Configuration (.cursorrules)
  - Clearly define repository-specific standards for code style, consistency, testing practices, and symbolic reasoning integration points.

Memory Bank Integration

- Persistent Context
  - Continuously retain relevant context across development stages to ensure coherent long-term planning and decision-making.
- Reference Prior Decisions
  - Regularly review past decisions stored in memory to maintain consistency and reduce redundancy.
- Adaptive Learning
  - Utilize historical data and previous solutions to adaptively refine new implementations.

General Guidelines for Programming Languages

1. Clarity and Readability
   - Favor straightforward, self-explanatory code structures across all languages.
   - Include descriptive comments to clarify complex logic.

2. Language-Specific Best Practices
   - Adhere to established community and project-specific best practices for each language (Python, JavaScript, Java, etc.).
   - Regularly review language documentation and style guides.

3. Consistency Across Codebases
   - Maintain uniform coding conventions and naming schemes across all languages used within a project.

Project Context & Understanding

1. Documentation First
   - Review essential documentation before implementation:
     - Product Requirements Documents (PRDs)
     - README.md
     - docs/architecture.md
     - docs/technical.md
     - tasks/tasks.md
   - Request clarification immediately if documentation is incomplete or ambiguous.

2. Architecture Adherence
   - Follow established module boundaries and architectural designs.
   - Validate architectural decisions using symbolic reasoning; propose justified alternatives when necessary.

3. Pattern & Tech Stack Awareness
   - Utilize documented technologies and established patterns; introduce new elements only after clear justification.

Task Execution & Workflow

Task Definition & Steps

1. Specification
   - Define clear objectives, detailed requirements, user scenarios, and UI/UX standards.
   - Use advanced symbolic reasoning to analyze complex scenarios.

2. Pseudocode
   - Clearly map out logical implementation pathways before coding.

3. Architecture
   - Design modular, maintainable system components using appropriate technology stacks.
   - Ensure integration points are clearly defined for autonomous decision-making.

4. Refinement
   - Iteratively optimize code using autonomous feedback loops and stakeholder inputs.

5. Completion
   - Conduct rigorous testing, finalize comprehensive documentation, and deploy structured monitoring strategies.

AI Collaboration & Prompting

1. Clear Instructions
   - Provide explicit directives with defined outcomes, constraints, and contextual information.

2. Context Referencing
   - Regularly reference previous stages and decisions stored in the memory bank.

3. Suggest vs. Apply
   - Clearly indicate whether AI should propose ("Suggestion:") or directly implement changes ("Applying fix:").

4. Critical Evaluation
   - Thoroughly review all agentic outputs for accuracy and logical coherence.

5. Focused Interaction
   - Assign specific, clearly defined tasks to AI agents to maintain clarity.

6. Leverage Agent Strengths
   - Utilize AI for refactoring, symbolic reasoning, adaptive optimization, and test generation; human oversight remains on core logic and strategic architecture.

7. Incremental Progress
   - Break complex tasks into incremental, reviewable sub-steps.

8. Standard Check-in
   - Example: "Confirming understanding: Reviewed [context], goal is [goal], proceeding with [step]."

Advanced Coding Capabilities

- Emergent Intelligence
  - AI autonomously maintains internal state models, supporting continuous refinement.
- Pattern Recognition
  - Autonomous agents perform advanced pattern analysis for effective optimization.
- Adaptive Optimization
  - Continuously evolving feedback loops refine the development process.

Symbolic Reasoning Integration

- Symbolic Logic Integration
  - Combine symbolic logic with complexity analysis for robust decision-making.
- Information Integration
  - Utilize symbolic mathematics and established software patterns for coherent implementations.
- Coherent Documentation
  - Maintain clear, semantically accurate documentation through symbolic reasoning.

Code Quality & Style

1. TypeScript Guidelines
   - Use strict types, and clearly document logic with JSDoc.

2. Maintainability
   - Write modular, scalable code optimized for clarity and maintenance.

3. Concise Components
   - Keep files concise (under 300 lines) and proactively refactor.

4. Avoid Duplication (DRY)
   - Use symbolic reasoning to systematically identify redundancy.

5. Linting/Formatting
   - Consistently adhere to ESLint/Prettier configurations.

6. File Naming
   - Use descriptive, permanent, and standardized naming conventions.

7. No One-Time Scripts
   - Avoid committing temporary utility scripts to production repositories.

Refactoring

1. Purposeful Changes
   - Refactor with clear objectives: improve readability, reduce redundancy, and meet architecture guidelines.

2. Holistic Approach
   - Consolidate similar components through symbolic analysis.

3. Direct Modification
   - Directly modify existing code rather than duplicating or creating temporary versions.

4. Integration Verification
   - Verify and validate all integrations after changes.

Testing & Validation

1. Test-Driven Development
   - Define and write tests before implementing features or fixes.

2. Comprehensive Coverage
   - Provide thorough test coverage for critical paths and edge cases.

3. Mandatory Passing
   - Immediately address any failing tests to maintain high-quality standards.

4. Manual Verification
   - Complement automated tests with structured manual checks.

Debugging & Troubleshooting

1. Root Cause Resolution
   - Employ symbolic reasoning to identify underlying causes of issues.

2. Targeted Logging
   - Integrate precise logging for efficient debugging.

3. Research Tools
   - Use advanced agentic tools (Perplexity, AIDER.chat, Firecrawl) to resolve complex issues efficiently.

Security

1. Server-Side Authority
   - Maintain sensitive logic and data processing strictly server-side.

2. Input Sanitization
   - Enforce rigorous server-side input validation.

3. Credential Management
   - Securely manage credentials via environment variables; avoid any hardcoding.

Version Control & Environment

1. Git Hygiene
   - Commit frequently with clear and descriptive messages.

2. Branching Strategy
   - Adhere strictly to defined branching guidelines.

3. Environment Management
   - Ensure code consistency and compatibility across all environments.

4. Server Management
   - Systematically restart servers following updates or configuration changes.

Documentation Maintenance

1. Reflective Documentation
   - Keep comprehensive, accurate, and logically structured documentation updated through symbolic reasoning.

2. Continuous Updates
   - Regularly revisit and refine guidelines to reflect evolving practices and accumulated project knowledge.

3. Check each file once
   - Ensure all files are checked for accuracy and relevance.

4. Use of Comments
   - Use comments to clarify complex logic and provide context for future developers.

# Tools Use
   
<details><summary>File Operations</summary>


<read_file>
  <path>File path here</path>
</read_file>

<write_to_file>
  <path>File path here</path>
  <content>Your file content here</content>
  <line_count>Total number of lines</line_count>
</write_to_file>

<list_files>
  <path>Directory path here</path>
  <recursive>true/false</recursive>
</list_files>

</details>


<details><summary>Code Editing</summary>


<apply_diff>
  <path>File path here</path>
  <diff>
    <<<<<<< SEARCH
    Original code
    =======
    Updated code
    >>>>>>> REPLACE
  </diff>
  <start_line>Start</start_line>
  <end_line>End_line</end_line>
</apply_diff>

<insert_content>
  <path>File path here</path>
  <operations>
    [{"start_line":10,"content":"New code"}]
  </operations>
</insert_content>

<search_and_replace>
  <path>File path here</path>
  <operations>
    [{"search":"old_text","replace":"new_text","use_regex":true}]
  </operations>
</search_and_replace>

</details>


<details><summary>Project Management</summary>


<execute_command>
  <command>Your command here</command>
</execute_command>

<attempt_completion>
  <result>Final output</result>
  <command>Optional CLI command</command>
</attempt_completion>

<ask_followup_question>
  <question>Clarification needed</question>
</ask_followup_question>

</details>


<details><summary>MCP Integration</summary>


<use_mcp_tool>
  <server_name>Server</server_name>
  <tool_name>Tool</tool_name>
  <arguments>{"param":"value"}</arguments>
</use_mcp_tool>

<access_mcp_resource>
  <server_name>Server</server_name>
  <uri>resource://path</uri>
</access_mcp_resource>

</details>
