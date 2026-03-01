import { Button, Input, Select, SelectItem, Card, CardBody, Textarea } from '@heroui/react';
import { Plus, Trash2, Code, Filter as FilterIcon } from 'lucide-react';

interface FilterCondition {
  id: string;
  field: string;
  operator: 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte' | 'contains' | 'exists';
  value: string | number | boolean;
}

interface FilterBuilderProps {
  conditions: FilterCondition[];
  onAddCondition: () => void;
  onUpdateCondition: (id: string, updates: Partial<FilterCondition>) => void;
  onRemoveCondition: (id: string) => void;
  generatedJson: string;
  showJson: boolean;
  onToggleJson: () => void;
}

const OPERATORS = [
  { key: 'eq', label: 'Equals (=)' },
  { key: 'ne', label: 'Not Equals (≠)' },
  { key: 'gt', label: 'Greater Than (>)' },
  { key: 'lt', label: 'Less Than (<)' },
  { key: 'gte', label: 'Greater or Equal (≥)' },
  { key: 'lte', label: 'Less or Equal (≤)' },
  { key: 'contains', label: 'Contains' },
  { key: 'exists', label: 'Exists' },
];

export default function FilterBuilder({
  conditions,
  onAddCondition,
  onUpdateCondition,
  onRemoveCondition,
  generatedJson,
  showJson,
  onToggleJson,
}: FilterBuilderProps) {
  return (
    <Card className="bg-gray-800/50 border border-gray-700">
      <CardBody className="space-y-3">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <FilterIcon className="w-4 h-4 text-primary" />
            <span className="text-sm font-semibold">Filter Builder</span>
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="flat"
              onPress={onToggleJson}
              startContent={<Code className="w-3 h-3" />}
              className="bg-gray-700/50 hover:bg-gray-700"
            >
              {showJson ? 'Hide' : 'Show'} JSON
            </Button>
            <Button
              size="sm"
              color="primary"
              variant="flat"
              onPress={onAddCondition}
              startContent={<Plus className="w-3 h-3" />}
            >
              Add Condition
            </Button>
          </div>
        </div>

        {/* Conditions */}
        {conditions.length === 0 ? (
          <div className="text-center py-4 text-gray-500 text-sm">
            No filter conditions. Click "Add Condition" to get started.
          </div>
        ) : (
          <div className="space-y-2">
            {conditions.map((condition, index) => (
              <div key={condition.id} className="flex items-center gap-2">
                {/* AND label for subsequent conditions */}
                {index > 0 && (
                  <div className="text-xs text-gray-500 font-semibold w-10">AND</div>
                )}
                {index === 0 && <div className="w-10" />}

                {/* Field Input */}
                <Input
                  size="sm"
                  placeholder="field name"
                  value={condition.field}
                  onChange={(e) => onUpdateCondition(condition.id, { field: e.target.value })}
                  classNames={{
                    input: "bg-gray-800/50 text-white placeholder:text-gray-500",
                    inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
                  }}
                  className="flex-1"
                />

                {/* Operator Select */}
                <Select
                  size="sm"
                  placeholder="operator"
                  selectedKeys={[condition.operator]}
                  onChange={(e) => onUpdateCondition(condition.id, { operator: e.target.value as any })}
                  classNames={{
                    trigger: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
                    value: "text-white text-xs",
                  }}
                  className="w-48"
                >
                  {OPERATORS.map((op) => (
                    <SelectItem key={op.key}>
                      {op.label}
                    </SelectItem>
                  ))}
                </Select>

                {/* Value Input */}
                {condition.operator === 'exists' ? (
                  <Select
                    size="sm"
                    placeholder="value"
                    selectedKeys={[String(condition.value)]}
                    onChange={(e) => onUpdateCondition(condition.id, { value: e.target.value === 'true' })}
                    classNames={{
                      trigger: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
                      value: "text-white text-xs",
                    }}
                    className="flex-1"
                  >
                    <SelectItem key="true">True</SelectItem>
                    <SelectItem key="false">False</SelectItem>
                  </Select>
                ) : (
                  <Input
                    size="sm"
                    placeholder="value"
                    value={String(condition.value)}
                    onChange={(e) => {
                      const val = e.target.value;
                      // Try to parse as number for numeric operators
                      if (['gt', 'lt', 'gte', 'lte'].includes(condition.operator)) {
                        const num = parseFloat(val);
                        onUpdateCondition(condition.id, { value: isNaN(num) ? val : num });
                      } else {
                        onUpdateCondition(condition.id, { value: val });
                      }
                    }}
                    classNames={{
                      input: "bg-gray-800/50 text-white placeholder:text-gray-500",
                      inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
                    }}
                    className="flex-1"
                  />
                )}

                {/* Delete Button */}
                <Button
                  isIconOnly
                  size="sm"
                  color="danger"
                  variant="flat"
                  onPress={() => onRemoveCondition(condition.id)}
                  className="min-w-8"
                >
                  <Trash2 className="w-3 h-3" />
                </Button>
              </div>
            ))}
          </div>
        )}

        {/* Generated JSON Preview */}
        {showJson && (
          <div className="pt-2 border-t border-gray-700">
            <div className="text-xs text-gray-500 mb-1 font-semibold">Generated Filter JSON:</div>
            <Textarea
              value={generatedJson}
              readOnly
              minRows={3}
              maxRows={8}
              classNames={{
                input: "bg-gray-900 text-green-400 font-mono text-xs",
                inputWrapper: "bg-gray-900 border-gray-700",
              }}
            />
          </div>
        )}

        {/* Helper Text */}
        {conditions.length > 0 && (
          <div className="text-xs text-gray-500 pt-1">
            All conditions are combined with AND logic. Use the generated JSON for your vector search filter.
          </div>
        )}
      </CardBody>
    </Card>
  );
}
