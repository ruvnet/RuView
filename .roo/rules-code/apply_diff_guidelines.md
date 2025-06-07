# Preventing apply_diff Errors

## CRITICAL: When using apply_diff, never include literal diff markers in your code examples

## CORRECT FORMAT for apply_diff:
```
<apply_diff>
  <path>file/path.js</path>
  <diff>
    <<<<<<< SEARCH
    // Original code to find (exact match)
    =======
    // New code to replace with
    >>>>>>> REPLACE
  </diff>
</apply_diff>
```

## COMMON ERRORS to AVOID:
1. Including literal diff markers in code examples or comments
2. Nesting diff blocks inside other diff blocks
3. Using incomplete diff blocks (missing SEARCH or REPLACE markers)
4. Using incorrect diff marker syntax
5. Including backticks inside diff blocks when showing code examples

## When showing code examples that contain diff syntax:
- Escape the markers or use alternative syntax
- Use HTML entities or alternative symbols
- Use code block comments to indicate diff sections

## SAFE ALTERNATIVE for showing diff examples:
```
// Example diff (DO NOT COPY DIRECTLY):
// [SEARCH]
// function oldCode() {}
// [REPLACE]
// function newCode() {}
```

## ALWAYS validate your diff blocks before executing apply_diff
- Ensure exact text matching
- Verify proper marker syntax
- Check for balanced markers
- Avoid nested markers