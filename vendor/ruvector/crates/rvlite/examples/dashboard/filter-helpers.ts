// Filter Builder Helper Functions
// Add these to App.tsx after the addLog callback (around line 544)

import { useCallback, useEffect } from 'react';

interface FilterCondition {
  id: string;
  field: string;
  operator: 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte' | 'contains' | 'exists';
  value: string | number | boolean;
}

// Copy these into your App.tsx component:

export const useFilterConditionHelpers = (
  setFilterConditions: React.Dispatch<React.SetStateAction<FilterCondition[]>>,
  setFilterJson: React.Dispatch<React.SetStateAction<string>>
) => {
  const addFilterCondition = useCallback(() => {
    const newCondition: FilterCondition = {
      id: `condition_${Date.now()}`,
      field: '',
      operator: 'eq',
      value: '',
    };
    setFilterConditions(prev => [...prev, newCondition]);
  }, [setFilterConditions]);

  const updateFilterCondition = useCallback((id: string, updates: Partial<FilterCondition>) => {
    setFilterConditions(prev =>
      prev.map(cond => cond.id === id ? { ...cond, ...updates } : cond)
    );
  }, [setFilterConditions]);

  const removeFilterCondition = useCallback((id: string) => {
    setFilterConditions(prev => prev.filter(cond => cond.id !== id));
  }, [setFilterConditions]);

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

  return {
    addFilterCondition,
    updateFilterCondition,
    removeFilterCondition,
    conditionsToFilterJson,
  };
};
