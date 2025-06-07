# Search and Replace Guidelines

## search_and_replace
```xml
<search_and_replace>
  <path>File path here</path>
  <operations>
    [{"search":"old_text","replace":"new_text","use_regex":true}]
  </operations>
</search_and_replace>
```

### Required Parameters:
- `path`: The file path to modify
- `operations`: JSON array of search and replace operations

### Each Operation Must Include:
- `search`: The text to search for (REQUIRED)
- `replace`: The text to replace with (REQUIRED)
- `use_regex`: Boolean indicating whether to use regex (optional, defaults to false)

### Common Errors to Avoid:
- Missing `search` parameter
- Missing `replace` parameter
- Invalid JSON format in operations array
- Attempting to modify non-existent files
- Malformed regex patterns when use_regex is true

### Best Practices:
- Always include both search and replace parameters
- Verify the file exists before attempting to modify it
- Use apply_diff for complex changes instead
- Test regex patterns separately before using them
- Escape special characters in regex patterns