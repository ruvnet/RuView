# Code Editing Guidelines

## apply_diff
```xml
<apply_diff>
  <path>File path here</path>
  <diff>
    <<<<<<< SEARCH
    Original code
    =======
    Updated code
    >>>>>>> REPLACE
  </diff>
</apply_diff>
```

### Required Parameters:
- `path`: The file path to modify
- `diff`: The diff block containing search and replace content

### Common Errors to Avoid:
- Incomplete diff blocks (missing SEARCH or REPLACE markers)
- Including literal diff markers in code examples
- Nesting diff blocks inside other diff blocks
- Using incorrect diff marker syntax
- Including backticks inside diff blocks when showing code examples

### Best Practices:
- Always verify the file exists before applying diffs
- Ensure exact text matching for the search block
- Use read_file first to confirm content before modifying
- Keep diff blocks simple and focused on specific changes