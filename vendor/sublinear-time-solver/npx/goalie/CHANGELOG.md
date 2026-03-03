# Changelog

## [1.0.4] - 2025-09-29

### Fixed
- CLI now correctly reads version from package.json instead of hardcoded value
- `npx goalie --version` now shows the correct version

## [1.0.3] - 2025-09-29

### Fixed
- Removed duplicate "Search completed successfully" message that confused users
- Search now shows progress correctly without premature completion message

## [1.0.2] - 2025-09-29

### Fixed
- CLI commands now properly exit after completion (fixed hanging issue)
- Added timeout wrappers to prevent infinite loops in all CLI commands
- Fixed undefined `paginationInfo.totalResults` display issue
- Fixed anti-hallucination plugin name mismatch in MCP tools
- Improved error handling for missing PERPLEXITY_API_KEY (now throws proper error)
- Fixed TypeScript type issues with Promise.race

### Added
- All advanced reasoning plugins now have functional execute methods:
  - Chain-of-Thought reasoning with Tree-of-Thoughts
  - Self-Consistency with majority voting
  - Anti-Hallucination with citation verification
  - Agentic Research with multi-agent orchestration
- Ed25519 cryptographic verification fully tested and working
- Comprehensive CLI command documentation in README

### Updated
- README.md with correct CLI command syntax and comprehensive documentation
- All CLI commands use proper names: `search`, `query`, `reasoning`, `explain`, `raw`, `plugin`
- Reasoning subcommands: `chain-of-thought`, `consistency`, `verify`, `agents`

### Verified
- All CLI commands work with real Perplexity API
- Files save correctly to `.research/` directory in both JSON and Markdown formats
- MCP tools function properly
- Ed25519 verification successfully verifies citations (tested: 69/69 verified)
- All plugins initialize and execute correctly

## [1.0.1] - 2025-09-28

### Initial Release
- Goal-Oriented Action Planning (GOAP) with A* pathfinding
- Perplexity API integration
- MCP (Model Context Protocol) server
- Advanced reasoning plugins
- Ed25519 cryptographic verification
- Anti-hallucination features