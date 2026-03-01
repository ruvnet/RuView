# Advanced Filter Builder - Implementation Guide

This guide provides step-by-step instructions to integrate the Advanced Filter Builder into the RvLite Dashboard.

## Prerequisites

The following files have been created and are ready to use:
- `/workspaces/ruvector/crates/rvlite/examples/dashboard/src/FilterBuilder.tsx` ✓

## Integration Steps

### Step 1: Add Import Statement

**Location:** Line ~92 (after `import useLearning from './hooks/useLearning';`)

Add this line:
```typescript
import FilterBuilder from './FilterBuilder';
```

**Full context:**
```typescript
import useRvLite, { type SearchResult, type CypherResult, type SparqlResult, type SqlResult, type VectorEntry } from './hooks/useRvLite';
import useLearning from './hooks/useLearning';
import FilterBuilder from './FilterBuilder';  // <-- ADD THIS LINE
```

---

### Step 2: Add Filter Helper Functions

**Location:** Line ~545 (right after the `addLog` callback, before `hasInitialized`)

Add this code:

```typescript
  // Filter condition helpers
  const addFilterCondition = useCallback(() => {
    const newCondition: FilterCondition = {
      id: `condition_${Date.now()}`,
      field: '',
      operator: 'eq',
      value: '',
    };
    setFilterConditions(prev => [...prev, newCondition]);
  }, []);

  const updateFilterCondition = useCallback((id: string, updates: Partial<FilterCondition>) => {
    setFilterConditions(prev =>
      prev.map(cond => cond.id === id ? { ...cond, ...updates } : cond)
    );
  }, []);

  const removeFilterCondition = useCallback((id: string) => {
    setFilterConditions(prev => prev.filter(cond => cond.id !== id));
  }, []);

  const conditionsToFilterJson = useCallback((conditions: FilterCondition[]): string => {
    if (conditions.length === 0) return '{}';

    const filter: Record<string, any> = {};

    conditions.forEach(cond => {
      if (!cond.field.trim()) return;

      const fieldName = cond.field.trim();

      switch (cond.operator) {
        case 'eq':
          filter[fieldName] = cond.value;
          break;
        case 'ne':
          filter[fieldName] = { $ne: cond.value };
          break;
        case 'gt':
          filter[fieldName] = { ...(filter[fieldName] || {}), $gt: cond.value };
          break;
        case 'lt':
          filter[fieldName] = { ...(filter[fieldName] || {}), $lt: cond.value };
          break;
        case 'gte':
          filter[fieldName] = { ...(filter[fieldName] || {}), $gte: cond.value };
          break;
        case 'lte':
          filter[fieldName] = { ...(filter[fieldName] || {}), $lte: cond.value };
          break;
        case 'contains':
          filter[fieldName] = { $contains: cond.value };
          break;
        case 'exists':
          filter[fieldName] = { $exists: cond.value === 'true' || cond.value === true };
          break;
      }
    });

    return JSON.stringify(filter, null, 2);
  }, []);

  // Update filterJson whenever conditions change
  useEffect(() => {
    if (useFilter && filterConditions.length > 0) {
      const jsonStr = conditionsToFilterJson(filterConditions);
      setFilterJson(jsonStr);
    }
  }, [filterConditions, useFilter, conditionsToFilterJson]);
```

**Full context:**
```typescript
  // Logging
  const addLog = useCallback((type: LogEntry['type'], message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-99), { timestamp, type, message }]);
  }, []);

  // Filter condition helpers           <-- START ADDING HERE
  const addFilterCondition = useCallback(() => {
    // ... (code above)
  }, []);
  // ... (rest of the helper functions)

  useEffect(() => {
    // ... (update filterJson effect)
  }, [filterConditions, useFilter, conditionsToFilterJson]);
                                         <-- END HERE

  // Track if we've initialized to prevent re-running effects
  const hasInitialized = useRef(false);
```

---

### Step 3: Replace the Filter UI Section

**Location:** Around line 1190-1213

**FIND THIS CODE:**
```typescript
                      {/* Filter option */}
                      <div className="flex items-center gap-4">
                        <Switch
                          size="sm"
                          isSelected={useFilter}
                          onValueChange={setUseFilter}
                        >
                          Use metadata filter
                        </Switch>
                        {useFilter && (
                          <Input
                            size="sm"
                            placeholder='{"category": "ML"}'
                            value={filterJson}
                            onChange={(e) => setFilterJson(e.target.value)}
                            startContent={<Filter className="w-4 h-4 text-gray-400" />}
                            classNames={{
                              input: "bg-gray-800/50 text-white placeholder:text-gray-500 font-mono text-xs",
                              inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
                            }}
                            className="flex-1"
                          />
                        )}
                      </div>
```

**REPLACE WITH:**
```typescript
                      {/* Filter option */}
                      <div className="space-y-3">
                        <Switch
                          size="sm"
                          isSelected={useFilter}
                          onValueChange={setUseFilter}
                        >
                          Use metadata filter
                        </Switch>

                        {useFilter && (
                          <FilterBuilder
                            conditions={filterConditions}
                            onAddCondition={addFilterCondition}
                            onUpdateCondition={updateFilterCondition}
                            onRemoveCondition={removeFilterCondition}
                            generatedJson={filterJson}
                            showJson={showFilterJson}
                            onToggleJson={() => setShowFilterJson(!showFilterJson)}
                          />
                        )}
                      </div>
```

---

## Verification

After making the changes:

1. **Check for TypeScript errors:**
   ```bash
   npm run typecheck
   ```

2. **Start the dev server:**
   ```bash
   npm run dev
   ```

3. **Test the Filter Builder:**
   - Navigate to the Vector tab
   - Enable "Use metadata filter" switch
   - Click "Add Condition"
   - Add a filter: Field=`category`, Operator=`Equals`, Value=`ML`
   - Click "Show JSON" to verify the generated filter
   - Perform a search

## Expected Behavior

1. When you toggle "Use metadata filter" ON, the Filter Builder appears
2. Click "Add Condition" to add filter rows
3. Each row has:
   - Field input (for metadata field name)
   - Operator dropdown (equals, not equals, greater than, etc.)
   - Value input (auto-detects number vs string)
   - Delete button (trash icon)
4. Click "Show JSON" to see the generated filter JSON
5. Multiple conditions combine with AND logic
6. The filter is automatically applied when performing vector searches

## Troubleshooting

### Issue: TypeScript errors about FilterCondition
**Solution:** The `FilterCondition` interface is already defined in App.tsx at line 100-105. No action needed.

### Issue: Import error for FilterBuilder
**Solution:** Verify that `/workspaces/ruvector/crates/rvlite/examples/dashboard/src/FilterBuilder.tsx` exists.

### Issue: Filter doesn't apply to searches
**Solution:** Check the browser console for errors. Verify that `filterJson` state is being updated when conditions change.

### Issue: Can't find the UI section to replace
**Solution:** Search for the text "Use metadata filter" in App.tsx to find the exact location.

## Example Filters

### Example 1: Simple Equality
```
Field: category
Operator: Equals (=)
Value: ML
```
Generates: `{ "category": "ML" }`

### Example 2: Numeric Range
```
Condition 1: Field=price, Operator=Greater Than, Value=50
Condition 2: Field=price, Operator=Less Than, Value=100
```
Generates: `{ "price": { "$gt": 50, "$lt": 100 } }`

### Example 3: Multiple Fields
```
Condition 1: Field=category, Operator=Equals, Value=ML
Condition 2: Field=tags, Operator=Contains, Value=sample
```
Generates: `{ "category": "ML", "tags": { "$contains": "sample" } }`

---

## Summary

You need to make 3 changes to `src/App.tsx`:

1. ✓ Add import for FilterBuilder (line ~92)
2. ✓ Add filter helper functions (line ~545)
3. ✓ Replace filter UI section (line ~1190)

State variables (`filterConditions`, `showFilterJson`) are already defined (lines 531-534).

The FilterBuilder component is already created at `src/FilterBuilder.tsx`.
