# hook pre-edit

Execute pre-edit validations and agent assignment before file modifications.

## Usage

```bash
npx claude-flow hook pre-edit [options]
```

## Options

- `--file, -f <path>` - File path to be edited
- `--auto-assign-agent` - Automatically assign best agent (default: true)
- `--validate-syntax` - Pre-validate syntax before edit
- `--check-conflicts` - Check for merge conflicts
- `--backup-file` - Create backup before editing

## Examples

### Basic pre-edit hook

```bash
npx claude-flow hook pre-edit --file "src/auth/login.js"
```

### With validation

```bash
npx claude-flow hook pre-edit -f "config/database.js" --validate-syntax
```

### Manual agent assignment

```bash
npx claude-flow hook pre-edit -f "api/users.ts" --auto-assign-agent false
```

### Safe editing with backup

```bash
npx claude-flow hook pre-edit -f "production.env" --backup-file --check-conflicts
```

## Features

### Auto Agent Assignment

- Analyzes file type and content
- Assigns specialist agents
- TypeScript → TypeScript expert
- Database → Data specialist
- Tests → QA engineer

### Syntax Validation

- Pre-checks syntax validity
- Identifies potential errors
- Suggests corrections
- Prevents broken code

### Conflict Detection

- Checks for git conflicts
- Identifies concurrent edits
- Warns about stale files
- Suggests merge strategies

### File Backup

- Creates safety backups
- Enables quick rollback
- Tracks edit history
- Preserves originals

## Integration

This hook is automatically called by Claude Code when:

- Using Edit or MultiEdit tools
- Before file modifications
- During refactoring operations
- When updating critical files

Manual usage in agents:

```bash
# Before editing files
npx claude-flow hook pre-edit --file "path/to/file.js" --validate-syntax
```

## Output

Returns JSON with:

```json
{
  "continue": true,
  "file": "src/auth/login.js",
  "assignedAgent": "auth-specialist",
  "syntaxValid": true,
  "conflicts": false,
  "backupPath": ".backups/login.js.bak",
  "warnings": []
}
```

## See Also

- `hook post-edit` - Post-edit processing
- `Edit` - File editing tool
- `MultiEdit` - Multiple edits tool
- `agent spawn` - Manual agent creation
