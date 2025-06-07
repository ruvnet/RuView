# File Operations Guidelines

## read_file
```xml
<read_file>
  <path>File path here</path>
</read_file>
```

### Required Parameters:
- `path`: The file path to read

### Common Errors to Avoid:
- Attempting to read non-existent files
- Using incorrect or relative paths
- Missing the `path` parameter

### Best Practices:
- Always check if a file exists before attempting to modify it
- Use `read_file` before `apply_diff` or `search_and_replace` to verify content
- For large files, consider using start_line and end_line parameters to read specific sections

## write_to_file
```xml
<write_to_file>
  <path>File path here</path>
