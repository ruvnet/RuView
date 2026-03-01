# Filter Builder Integration Guide

This guide shows exactly how to integrate the Advanced Filter Builder UI into the RvLite Dashboard.

## Files Created

1. `/workspaces/ruvector/crates/rvlite/examples/dashboard/src/FilterBuilder.tsx` - The new Filter Builder component

## Changes Needed to App.tsx

### Step 1: Add Import (Line ~83, after existing imports)

Add this import after the `useLearning` import:

```typescript
import FilterBuilder from './FilterBuilder';
```

### Step 2: Add Helper Functions (After line 544, after `addLog` function)

Add these helper functions between the `addLog` callback and `hasInitialized`:

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

### Step 3: Replace Filter UI (Around line 1189-1212)

Replace the existing filter UI section:

**OLD CODE (lines ~1189-1212):**
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

**NEW CODE:**
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

## How It Works

### Filter Condition Interface
```typescript
interface FilterCondition {
  id: string;                    // Unique identifier
  field: string;                 // Metadata field name (e.g., "category", "price")
  operator: 'eq' | 'ne' | ...;  // Comparison operator
  value: string | number | boolean; // Filter value
}
```

### Operator Mappings

| Visual Operator | JSON Output | Example |
|----------------|-------------|---------|
| Equals (=) | `{ "field": value }` | `{ "category": "ML" }` |
| Not Equals (≠) | `{ "field": { "$ne": value }}` | `{ "status": { "$ne": "active" }}` |
| Greater Than (>) | `{ "field": { "$gt": value }}` | `{ "price": { "$gt": 100 }}` |
| Less Than (<) | `{ "field": { "$lt": value }}` | `{ "age": { "$lt": 30 }}` |
| Greater or Equal (≥) | `{ "field": { "$gte": value }}` | `{ "score": { "$gte": 0.8 }}` |
| Less or Equal (≤) | `{ "field": { "$lte": value }}` | `{ "quantity": { "$lte": 50 }}` |
| Contains | `{ "field": { "$contains": value }}` | `{ "tags": { "$contains": "ai" }}` |
| Exists | `{ "field": { "$exists": true/false }}` | `{ "metadata": { "$exists": true }}` |

### Multiple Conditions

All conditions are combined with AND logic. For example:

**Visual Builder:**
- Field: `category`, Operator: `Equals`, Value: `ML`
- Field: `price`, Operator: `Less Than`, Value: `100`

**Generated JSON:**
```json
{
  "category": "ML",
  "price": {
    "$lt": 100
  }
}
```

### Range Queries

Multiple conditions on the same field are merged. For example:

**Visual Builder:**
- Field: `price`, Operator: `Greater Than`, Value: `50`
- Field: `price`, Operator: `Less Than`, Value: `100`

**Generated JSON:**
```json
{
  "price": {
    "$gt": 50,
    "$lt": 100
  }
}
```

## Features

1. **Visual Interface**: No need to write JSON manually
2. **Dynamic Conditions**: Add/remove conditions on the fly
3. **Type-Aware**: Automatically handles strings, numbers, and booleans
4. **JSON Preview**: Toggle to see generated filter JSON
5. **Validation**: Empty fields are automatically skipped
6. **Dark Theme**: Matches existing dashboard styling
7. **HeroUI Components**: Consistent with dashboard design

## Usage Example

1. Enable the "Use metadata filter" switch
2. Click "Add Condition" to add a new filter condition
3. Fill in:
   - Field: The metadata field name (e.g., "category")
   - Operator: The comparison type (e.g., "Equals")
   - Value: The value to filter by (e.g., "ML")
4. Add more conditions as needed (they combine with AND)
5. Click "Show JSON" to see the generated filter
6. Perform a search - the filter will be applied automatically

## Testing

Test with sample data already in the dashboard:

### Example 1: Filter by Category
- Field: `category`
- Operator: `Equals (=)`
- Value: `ML`

This will find all vectors with `metadata.category === "ML"`

### Example 2: Filter by Multiple Conditions
- Condition 1: Field `category`, Operator `Equals`, Value `ML`
- Condition 2: Field `tags`, Operator `Contains`, Value `sample`

This will find vectors where category is "ML" AND tags contains "sample"

### Example 3: Range Filter
- Condition 1: Field `score`, Operator `Greater or Equal (≥)`, Value `0.5`
- Condition 2: Field `score`, Operator `Less or Equal (≤)`, Value `0.9`

This will find vectors with score between 0.5 and 0.9

## Troubleshooting

### If ESLint keeps modifying the file:
1. Save all changes
2. Wait for ESLint to finish auto-fixing
3. Then make the edits in a single operation

### If the filter doesn't work:
1. Check the "Show JSON" preview to see the generated filter
2. Ensure field names match your vector metadata exactly
3. Use the browser console to check for any errors

### If types are not recognized:
The `FilterCondition` interface is already defined in App.tsx (lines 100-105)
