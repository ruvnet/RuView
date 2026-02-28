# code-review

Automated code review with swarm intelligence.

## Usage
```bash
npx claude-flow github code-review [options]
```

## Options
- `--pr-number <n>` - Pull request to review
- `--focus <areas>` - Review focus (security, performance, style)
- `--suggest-fixes` - Suggest code fixes

## Examples
```bash
# Review PR
npx claude-flow github code-review --pr-number 456

# Security focus
npx claude-flow github code-review --pr-number 456 --focus security

# With fix suggestions
npx claude-flow github code-review --pr-number 456 --suggest-fixes
```
