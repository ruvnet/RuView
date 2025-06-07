# Insert Content Guidelines

## insert_content
```xml
<insert_content>
  <path>File path here</path>
  <operations>
    [{"start_line":10,"content":"New code"}]
  </operations>
</insert_content>
```

### Required Parameters:
- `path`: The file path to modify
- `operations`: JSON array of insertion operations

### Each Operation Must Include:
- `start_line`: The line number where content should be inserted (REQUIRED)
- `content`: The content to insert (REQUIRED)

### Common Errors to Avoid:
- Missing `start_line` parameter
- Missing `content` parameter
- Invalid JSON format in operations array
- Using non-numeric values for start_line
- Attempting to insert at line numbers beyond file length
- Attempting to modify non-existent files

### Best Practices:
- Always verify the file exists before attempting to modify it
- Check file length before specifying start_line
- Use read_file first to confirm file content and structure
- Ensure proper JSON formatting in the operations array
- Use for adding new content rather than modifying existing content
- Prefer for documentation additions and new code blocks