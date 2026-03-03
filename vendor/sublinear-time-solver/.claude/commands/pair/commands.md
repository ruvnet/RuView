# Pair Programming Commands Reference

Complete reference for all pair programming session commands.

## Session Control Commands

### /start
Start a new pair programming session.
```
/start [--mode <mode>] [--agent <agent>]
```

### /end
End the current session.
```
/end [--save] [--report]
```

### /pause
Pause the current session.
```
/pause [--reason <reason>]
```

### /resume
Resume a paused session.
```
/resume
```

### /status
Show current session status.
```
/status [--verbose]
```

### /switch
Switch driver/navigator roles.
```
/switch [--immediate]
```

## Code Commands

### /explain
Explain the current code or selection.
```
/explain [--level basic|detailed|expert]
```

### /suggest
Get improvement suggestions.
```
/suggest [--type refactor|optimize|security|style]
```

### /implement
Request implementation (navigator mode).
```
/implement <description>
```

### /refactor
Refactor selected code.
```
/refactor [--pattern <pattern>] [--scope function|file|module]
```

### /optimize
Optimize code for performance.
```
/optimize [--target speed|memory|both]
```

### /document
Add documentation to code.
```
/document [--format jsdoc|markdown|inline]
```

### /comment
Add inline comments.
```
/comment [--verbose]
```

### /pattern
Apply a design pattern.
```
/pattern <pattern-name> [--example]
```

## Testing Commands

### /test
Run test suite.
```
/test [--watch] [--coverage] [--only <pattern>]
```

### /test-gen
Generate tests for current code.
```
/test-gen [--type unit|integration|e2e]
```

### /coverage
Check test coverage.
```
/coverage [--report html|json|terminal]
```

### /mock
Generate mock data or functions.
```
/mock <target> [--realistic]
```

### /test-watch
Enable test watching.
```
/test-watch [--on-save]
```

### /snapshot
Create test snapshots.
```
/snapshot [--update]
```

## Review Commands

### /review
Perform code review.
```
/review [--scope current|file|changes] [--strict]
```

### /security
Security analysis.
```
/security [--deep] [--fix]
```

### /perf
Performance analysis.
```
/perf [--profile] [--suggestions]
```

### /quality
Check code quality metrics.
```
/quality [--detailed]
```

### /lint
Run linters.
```
/lint [--fix] [--config <config>]
```

### /complexity
Analyze code complexity.
```
/complexity [--threshold <value>]
```

## Navigation Commands

### /goto
Navigate to file or location.
```
/goto <file>[:line[:column]]
```

### /find
Search in project.
```
/find <pattern> [--regex] [--case-sensitive]
```

### /recent
Show recent files.
```
/recent [--limit <n>]
```

### /bookmark
Manage bookmarks.
```
/bookmark [add|list|goto|remove] [<name>]
```

### /history
Show command history.
```
/history [--limit <n>] [--filter <pattern>]
```

### /tree
Show project structure.
```
/tree [--depth <n>] [--filter <pattern>]
```

## Git Commands

### /diff
Show git diff.
```
/diff [--staged] [--file <file>]
```

### /commit
Commit with verification.
```
/commit [--message <msg>] [--amend]
```

### /branch
Branch operations.
```
/branch [create|switch|delete|list] [<name>]
```

### /stash
Stash operations.
```
/stash [save|pop|list|apply] [<message>]
```

### /log
View git log.
```
/log [--oneline] [--limit <n>]
```

### /blame
Show git blame.
```
/blame [<file>]
```

## AI Partner Commands

### /agent
Manage AI agent.
```
/agent [switch|info|config] [<agent-name>]
```

### /teach
Teach the AI your preferences.
```
/teach <preference>
```

### /feedback
Provide feedback to AI.
```
/feedback [positive|negative] <message>
```

### /personality
Adjust AI personality.
```
/personality [professional|friendly|concise|verbose]
```

### /expertise
Set AI expertise focus.
```
/expertise [add|remove|list] [<domain>]
```

## Metrics Commands

### /metrics
Show session metrics.
```
/metrics [--period today|session|week|all]
```

### /score
Show quality scores.
```
/score [--breakdown]
```

### /productivity
Show productivity metrics.
```
/productivity [--chart]
```

### /leaderboard
Show improvement leaderboard.
```
/leaderboard [--personal|team]
```

## Configuration Commands

### /config
Manage configuration.
```
/config [get|set|reset] [<key>] [<value>]
```

### /profile
Manage profiles.
```
/profile [use|create|list|delete] [<name>]
```

### /theme
Change interface theme.
```
/theme [dark|light|auto|<custom>]
```

### /shortcuts
Manage keyboard shortcuts.
```
/shortcuts [list|set|reset] [<action>] [<keys>]
```

## Collaboration Commands

### /share
Share session or code.
```
/share [session|code|screen] [--with <user>]
```

### /invite
Invite collaborator.
```
/invite <email> [--role observer|participant]
```

### /chat
Send message to team.
```
/chat <message>
```

### /note
Add session note.
```
/note <text> [--tag <tag>]
```

## Learning Commands

### /learn
Access learning resources.
```
/learn [<topic>] [--level beginner|intermediate|advanced]
```

### /example
Show code examples.
```
/example <pattern-or-concept> [--language <lang>]
```

### /quiz
Take a quiz on current topic.
```
/quiz [--difficulty easy|medium|hard]
```

### /tip
Get a coding tip.
```
/tip [--topic <topic>]
```

## Utility Commands

### /help
Show help.
```
/help [<command>]
```

### /undo
Undo last action.
```
/undo [--steps <n>]
```

### /redo
Redo undone action.
```
/redo [--steps <n>]
```

### /clear
Clear screen or cache.
```
/clear [screen|cache|history]
```

### /export
Export session data.
```
/export [--format json|md|html] [--file <path>]
```

### /import
Import configuration or data.
```
/import <file> [--type config|session|profile]
```

## Debugging Commands

### /debug
Enter debug mode.
```
/debug [on|off|toggle]
```

### /breakpoint
Manage breakpoints.
```
/breakpoint [add|remove|list|clear] [<location>]
```

### /trace
Enable tracing.
```
/trace [on|off|show]
```

### /inspect
Inspect variable or object.
```
/inspect <variable> [--deep]
```

### /watch
Watch expression.
```
/watch [add|remove|list] [<expression>]
```

## Advanced Commands

### /macro
Record or run macros.
```
/macro [record|stop|play|list|delete] [<name>]
```

### /plugin
Manage plugins.
```
/plugin [install|remove|list|config] [<plugin>]
```

### /api
API operations.
```
/api <endpoint> [--method GET|POST|PUT|DELETE] [--data <json>]
```

### /exec
Execute system command.
```
/exec <command> [--background]
```

## Quick Command Aliases

| Alias | Full Command |
|-------|-------------|
| `/s` | `/suggest` |
| `/e` | `/explain` |
| `/t` | `/test` |
| `/r` | `/review` |
| `/c` | `/commit` |
| `/g` | `/goto` |
| `/f` | `/find` |
| `/h` | `/help` |
| `/sw` | `/switch` |
| `/st` | `/status` |

## Command Modifiers

### Global Modifiers
- `--quiet` - Suppress output
- `--verbose` - Detailed output
- `--json` - JSON output format
- `--no-cache` - Skip cache
- `--timeout <s>` - Command timeout

### Chaining Commands
Use `&&` to chain commands:
```
/test && /commit && /push
```

### Command History
- `↑/↓` - Navigate history
- `Ctrl+R` - Search history
- `!!` - Repeat last command
- `!<n>` - Run command n from history

## Custom Commands

Define custom commands in configuration:

```json
{
  "customCommands": {
    "tdd": "/test-gen && /test --watch",
    "full-review": "/lint --fix && /test && /review --strict",
    "quick-fix": "/suggest --type fix && /implement && /test"
  }
}
```

Use custom commands:
```
/custom tdd
/custom full-review
```

## Best Practices

1. **Learn Core Commands** - Master frequently used commands
2. **Use Aliases** - Speed up common operations
3. **Chain Commands** - Automate workflows
4. **Custom Commands** - Create your workflows
5. **Keyboard Shortcuts** - Faster than typing

## Related Documentation

- [Session Management](./session.md)
- [Configuration](./config.md)
- [Keyboard Shortcuts](./shortcuts.md)
- [Getting Started](./README.md)