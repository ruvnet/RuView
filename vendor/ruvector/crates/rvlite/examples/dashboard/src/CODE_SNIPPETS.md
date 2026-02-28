# Filter Builder Code Snippets

## Snippet 1: Import Statement (Line ~92)

```typescript
import FilterBuilder from './FilterBuilder';
```

---

## Snippet 2: Helper Functions (Line ~545)

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

---

## Snippet 3: UI Replacement (Line ~1190)

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

## Quick Reference

| Change | Location | Type | Lines |
|--------|----------|------|-------|
| Import | ~92 | Add | 1 |
| Helpers | ~545 | Add | 75 |
| UI | ~1190 | Replace | 20 |

Total changes: ~96 lines added/modified
