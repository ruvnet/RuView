---
name: browser
description: Web browser automation with AI-optimized snapshots for claude-flow agents
version: 1.0.0
triggers:
  - /browser
  - browse
  - web automation
  - scrape
  - navigate
  - screenshot
tools:
  - browser/open
  - browser/snapshot
  - browser/click
  - browser/fill
  - browser/screenshot
  - browser/close
---

# Browser Automation Skill

Web browser automation using agent-browser with AI-optimized snapshots. Reduces context by 93% using element refs (@e1, @e2) instead of full DOM.

## Core Workflow

```bash
# 1. Navigate to page
agent-browser open <url>

# 2. Get accessibility tree with element refs
agent-browser snapshot -i    # -i = interactive elements only

# 3. Interact using refs from snapshot
agent-browser click @e2
agent-browser fill @e3 "text"

# 4. Re-snapshot after page changes
agent-browser snapshot -i
```

## Quick Reference

### Navigation
| Command | Description |
|---------|-------------|
| `open <url>` | Navigate to URL |
| `back` | Go back |
| `forward` | Go forward |
| `reload` | Reload page |
| `close` | Close browser |

### Snapshots (AI-Optimized)
| Command | Description |
|---------|-------------|
| `snapshot` | Full accessibility tree |
| `snapshot -i` | Interactive elements only (buttons, links, inputs) |
| `snapshot -c` | Compact (remove empty elements) |
| `snapshot -d 3` | Limit depth to 3 levels |
| `screenshot [path]` | Capture screenshot (base64 if no path) |

### Interaction
| Command | Description |
|---------|-------------|
| `click <sel>` | Click element |
| `fill <sel> <text>` | Clear and fill input |
| `type <sel> <text>` | Type with key events |
| `press <key>` | Press key (Enter, Tab, etc.) |
| `hover <sel>` | Hover element |
| `select <sel> <val>` | Select dropdown option |
| `check/uncheck <sel>` | Toggle checkbox |
| `scroll <dir> [px]` | Scroll page |

### Get Info
| Command | Description |
|---------|-------------|
| `get text <sel>` | Get text content |
| `get html <sel>` | Get innerHTML |
| `get value <sel>` | Get input value |
| `get attr <sel> <attr>` | Get attribute |
| `get title` | Get page title |
| `get url` | Get current URL |

### Wait
| Command | Description |
|---------|-------------|
| `wait <selector>` | Wait for element |
| `wait <ms>` | Wait milliseconds |
| `wait --text "text"` | Wait for text |
| `wait --url "pattern"` | Wait for URL |
| `wait --load networkidle` | Wait for load state |

### Sessions
| Command | Description |
|---------|-------------|
| `--session <name>` | Use isolated session |
| `session list` | List active sessions |

## Selectors

### Element Refs (Recommended)
```bash
# Get refs from snapshot
agent-browser snapshot -i
# Output: button "Submit" [ref=e2]

# Use ref to interact
agent-browser click @e2
```

### CSS Selectors
```bash
agent-browser click "#submit"
agent-browser fill ".email-input" "test@test.com"
```

### Semantic Locators
```bash
agent-browser find role button click --name "Submit"
agent-browser find label "Email" fill "test@test.com"
agent-browser find testid "login-btn" click
```

## Examples

### Login Flow
```bash
agent-browser open https://example.com/login
agent-browser snapshot -i
agent-browser fill @e2 "user@example.com"
agent-browser fill @e3 "password123"
agent-browser click @e4
agent-browser wait --url "**/dashboard"
```

### Form Submission
```bash
agent-browser open https://example.com/contact
agent-browser snapshot -i
agent-browser fill @e1 "John Doe"
agent-browser fill @e2 "john@example.com"
agent-browser fill @e3 "Hello, this is my message"
agent-browser click @e4
agent-browser wait --text "Thank you"
```

### Data Extraction
```bash
agent-browser open https://example.com/products
agent-browser snapshot -i
# Iterate through product refs
agent-browser get text @e1  # Product name
agent-browser get text @e2  # Price
agent-browser get attr @e3 href  # Link
```

### Multi-Session (Swarm)
```bash
# Session 1: Navigator
agent-browser --session nav open https://example.com
agent-browser --session nav state save auth.json

# Session 2: Scraper (uses same auth)
agent-browser --session scrape state load auth.json
agent-browser --session scrape open https://example.com/data
agent-browser --session scrape snapshot -i
```

## Integration with Claude Flow

### MCP Tools
All browser operations are available as MCP tools with `browser/` prefix:
- `browser/open`
- `browser/snapshot`
- `browser/click`
- `browser/fill`
- `browser/screenshot`
- etc.

### Memory Integration
```bash
# Store successful patterns
npx @claude-flow/cli memory store --namespace browser-patterns --key "login-flow" --value "snapshot->fill->click->wait"

# Retrieve before similar task
npx @claude-flow/cli memory search --query "login automation"
```

### Hooks
```bash
# Pre-browse hook (get context)
npx @claude-flow/cli hooks pre-edit --file "browser-task.ts"

# Post-browse hook (record success)
npx @claude-flow/cli hooks post-task --task-id "browse-1" --success true
```

## Tips

1. **Always use snapshots** - They're optimized for AI with refs
2. **Prefer `-i` flag** - Gets only interactive elements, smaller output
3. **Use refs, not selectors** - More reliable, deterministic
4. **Re-snapshot after navigation** - Page state changes
5. **Use sessions for parallel work** - Each session is isolated
