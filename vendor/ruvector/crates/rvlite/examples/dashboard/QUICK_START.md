# Filter Builder - Quick Start Guide

## 🚀 3 Steps to Integrate

### Step 1: Import (Line ~92)
```typescript
import FilterBuilder from './FilterBuilder';
```

### Step 2: Add Helpers (Line ~545, after `addLog`)
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

  useEffect(() => {
    if (useFilter && filterConditions.length > 0) {
      const jsonStr = conditionsToFilterJson(filterConditions);
      setFilterJson(jsonStr);
    }
  }, [filterConditions, useFilter, conditionsToFilterJson]);
```

### Step 3: Replace UI (Line ~1190, search for "Use metadata filter")

**Find this:**
```typescript
{/* Filter option */}
<div className="flex items-center gap-4">
  <Switch size="sm" isSelected={useFilter} onValueChange={setUseFilter}>
    Use metadata filter
  </Switch>
  {useFilter && (
    <Input
      size="sm"
      placeholder='{"category": "ML"}'
      value={filterJson}
      onChange={(e) => setFilterJson(e.target.value)}
      ...
    />
  )}
</div>
```

**Replace with:**
```typescript
{/* Filter option */}
<div className="space-y-3">
  <Switch size="sm" isSelected={useFilter} onValueChange={setUseFilter}>
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

## ✅ Test

```bash
npm run dev
```

1. Toggle "Use metadata filter" ON
2. Click "+ Add Condition"
3. Add filter: `category` = `ML`
4. Click "Show JSON"
5. Perform search

## 📚 Full Documentation

- **Implementation Guide**: `src/IMPLEMENTATION_GUIDE.md`
- **Code Snippets**: `src/CODE_SNIPPETS.md`
- **Visual Demo**: `src/FILTER_BUILDER_DEMO.md`
- **README**: `README_FILTER_BUILDER.md`

## 📁 Files

- ✓ `src/FilterBuilder.tsx` (component)
- ✓ `src/App.tsx` (modify this)

## ⚡ Already Done

- ✓ State variables added (lines 531-534)
- ✓ FilterCondition interface (lines 100-105)
- ✓ HeroUI components imported
- ✓ Lucide icons imported

---

**Time to integrate:** ~10 minutes
