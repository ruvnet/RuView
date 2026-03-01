#!/bin/bash

# Script to apply Filter Builder integration to App.tsx
# This applies all changes atomically to avoid ESLint conflicts

APP_FILE="src/App.tsx"
BACKUP_FILE="src/App.tsx.backup"

echo "Creating backup..."
cp "$APP_FILE" "$BACKUP_FILE"

echo "Step 1: Adding FilterBuilder import..."
# Add import after useLearning
sed -i "/import useLearning from '.\/hooks\/useLearning';/a import FilterBuilder from './FilterBuilder';" "$APP_FILE"

echo "Step 2: Adding filter helper functions..."
# Create a temporary file with the helper functions
cat > /tmp/filter_helpers.txt << 'EOF'

  // Filter condition helpers
  const addFilterCondition = useCallback(() => {
    const newCondition: FilterCondition = {
      id: \`condition_\${Date.now()}\`,
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
          filter[fieldName] = { \$ne: cond.value };
          break;
        case 'gt':
          filter[fieldName] = { ...(filter[fieldName] || {}), \$gt: cond.value };
          break;
        case 'lt':
          filter[fieldName] = { ...(filter[fieldName] || {}), \$lt: cond.value };
          break;
        case 'gte':
          filter[fieldName] = { ...(filter[fieldName] || {}), \$gte: cond.value };
          break;
        case 'lte':
          filter[fieldName] = { ...(filter[fieldName] || {}), \$lte: cond.value };
          break;
        case 'contains':
          filter[fieldName] = { \$contains: cond.value };
          break;
        case 'exists':
          filter[fieldName] = { \$exists: cond.value === 'true' || cond.value === true };
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
EOF

# Insert after the addLog function
sed -i '/^  \/\/ Track if we.ve initialized to prevent re-running effects$/e cat /tmp/filter_helpers.txt' "$APP_FILE"

echo "Step 3: Replacing filter UI..."
# This is tricky - we need to replace a multi-line section
# For now, let's create a manual instruction file

cat > src/FilterBuilderIntegration.txt << 'EOF'
MANUAL STEP REQUIRED:

Find this section in App.tsx (around line 1189-1212):

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

REPLACE WITH:

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
EOF

echo ""
echo "✓ Steps 1 & 2 completed!"
echo "✗ Step 3 requires manual edit (see src/FilterBuilderIntegration.txt)"
echo ""
echo "Backup saved to: $BACKUP_FILE"
echo "If something goes wrong, restore with: cp $BACKUP_FILE $APP_FILE"
