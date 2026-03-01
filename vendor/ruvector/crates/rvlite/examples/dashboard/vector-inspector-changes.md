# Vector Inspector Implementation Guide

## Changes to /workspaces/ruvector/crates/rvlite/examples/dashboard/src/App.tsx

### 1. Add VectorEntry import (Line 90)
**FIND:**
```typescript
import useRvLite, { type SearchResult, type CypherResult, type SparqlResult, type SqlResult } from './hooks/useRvLite';
```

**REPLACE WITH:**
```typescript
import useRvLite, { type SearchResult, type CypherResult, type SparqlResult, type SqlResult, type VectorEntry } from './hooks/useRvLite';
```

### 2. Add Eye icon import (Line 31-74)
**FIND the lucide-react imports ending with:**
```typescript
  ChevronDown,
  ChevronRight,
} from 'lucide-react';
```

**REPLACE WITH:**
```typescript
  ChevronDown,
  ChevronRight,
  Eye,
} from 'lucide-react';
```

### 3. Add getVector to useRvLite destructuring (Line ~460)
**FIND:**
```typescript
  const {
    isReady,
    isLoading,
    error: rvliteError,
    stats,
    insertVector,
    insertVectorWithId,
    searchVectors,
    searchVectorsWithFilter,
    deleteVector,
    getAllVectors,
```

**REPLACE WITH:**
```typescript
  const {
    isReady,
    isLoading,
    error: rvliteError,
    stats,
    insertVector,
    insertVectorWithId,
    searchVectors,
    searchVectorsWithFilter,
    getVector,
    deleteVector,
    getAllVectors,
```

### 4. Add modal disclosure (after line ~507)
**FIND:**
```typescript
  const { isOpen: isScenariosOpen, onOpen: onScenariosOpen, onClose: onScenariosClose } = useDisclosure();
```

**ADD AFTER:**
```typescript
  const { isOpen: isVectorDetailOpen, onOpen: onVectorDetailOpen, onClose: onVectorDetailClose } = useDisclosure();
```

### 5. Add state variables (after line ~519)
**FIND:**
```typescript
  const [importJson, setImportJson] = useState('');
```

**ADD AFTER:**
```typescript
  const [selectedVectorId, setSelectedVectorId] = useState<string | null>(null);
  const [selectedVectorData, setSelectedVectorData] = useState<VectorEntry | null>(null);
```

### 6. Add handler function (after refreshVectors function, around line ~650)
**ADD NEW FUNCTION:**
```typescript
  const handleViewVector = useCallback(async (id: string) => {
    try {
      const vectorData = getVector(id);
      if (vectorData) {
        setSelectedVectorId(id);
        setSelectedVectorData(vectorData);
        onVectorDetailOpen();
        addLog('info', `Viewing vector: ${id}`);
      } else {
        addLog('error', `Vector not found: ${id}`);
      }
    } catch (err) {
      addLog('error', `Failed to get vector: ${formatError(err)}`);
    }
  }, [getVector, onVectorDetailOpen, addLog]);
```

### 7. Update Vector Table Cell to make ID clickable (around line 1218-1222)
**FIND:**
```typescript
                            <TableCell>
                              <div className="flex items-center gap-2">
                                <FileJson className="w-4 h-4 text-primary" />
                                <span className="font-mono text-sm">{vector.id}</span>
                              </div>
                            </TableCell>
```

**REPLACE WITH:**
```typescript
                            <TableCell>
                              <div
                                className="flex items-center gap-2 cursor-pointer hover:text-primary transition-colors"
                                onClick={() => handleViewVector(vector.id)}
                              >
                                <FileJson className="w-4 h-4 text-primary" />
                                <span className="font-mono text-sm">{vector.id}</span>
                              </div>
                            </TableCell>
```

### 8. Update View Details button (around line 1236-1240)
**FIND:**
```typescript
                                <Tooltip content="View Details">
                                  <Button isIconOnly size="sm" variant="light">
                                    <Code className="w-4 h-4" />
                                  </Button>
                                </Tooltip>
```

**REPLACE WITH:**
```typescript
                                <Tooltip content="View Details">
                                  <Button
                                    isIconOnly
                                    size="sm"
                                    variant="light"
                                    onPress={() => handleViewVector(vector.id)}
                                  >
                                    <Eye className="w-4 h-4" />
                                  </Button>
                                </Tooltip>
```

### 9. Add Vector Detail Modal (after Sample Scenarios Modal, around line 2350+)
**ADD NEW MODAL:**
```typescript
      {/* Vector Detail Modal */}
      <Modal isOpen={isVectorDetailOpen} onClose={onVectorDetailClose} size="3xl" scrollBehavior="inside">
        <ModalContent className="bg-gray-900 border border-gray-700">
          <ModalHeader className="flex items-center gap-2 border-b border-gray-700">
            <Eye className="w-5 h-5 text-primary" />
            <span>Vector Inspector</span>
            {selectedVectorId && (
              <Chip size="sm" variant="flat" color="primary" className="ml-2">
                {selectedVectorId}
              </Chip>
            )}
          </ModalHeader>
          <ModalBody className="py-6 space-y-6">
            {selectedVectorData ? (
              <>
                {/* Vector ID Section */}
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                    <Hash className="w-4 h-4" />
                    <span>Vector ID</span>
                  </div>
                  <Snippet
                    symbol=""
                    className="bg-gray-800/50 border border-gray-700"
                    classNames={{
                      pre: "font-mono text-sm text-gray-200"
                    }}
                  >
                    {selectedVectorData.id}
                  </Snippet>
                </div>

                {/* Dimensions */}
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                    <Layers className="w-4 h-4" />
                    <span>Dimensions</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Chip size="lg" variant="flat" color="primary">
                      {selectedVectorData.vector.length}D
                    </Chip>
                    <span className="text-xs text-gray-400">
                      ({selectedVectorData.vector.length} values)
                    </span>
                  </div>
                </div>

                {/* Embedding Values */}
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                      <Code className="w-4 h-4" />
                      <span>Embedding Values</span>
                    </div>
                    <Button
                      size="sm"
                      variant="flat"
                      onPress={() => {
                        navigator.clipboard.writeText(JSON.stringify(selectedVectorData.vector));
                        addLog('success', 'Embedding copied to clipboard');
                      }}
                    >
                      <Copy className="w-3 h-3 mr-1" />
                      Copy Array
                    </Button>
                  </div>
                  <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 max-h-60 overflow-auto">
                    <pre className="text-xs font-mono text-gray-300">
                      {selectedVectorData.vector.length <= 20
                        ? `[${selectedVectorData.vector.map(v => v.toFixed(6)).join(', ')}]`
                        : `[${selectedVectorData.vector.slice(0, 20).map(v => v.toFixed(6)).join(', ')}\n  ... ${selectedVectorData.vector.length - 20} more values]`
                      }
                    </pre>
                  </div>
                  {selectedVectorData.vector.length > 20 && (
                    <p className="text-xs text-gray-400 italic">
                      Showing first 20 of {selectedVectorData.vector.length} values
                    </p>
                  )}
                </div>

                {/* Metadata */}
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                      <FileJson className="w-4 h-4" />
                      <span>Metadata</span>
                    </div>
                    {selectedVectorData.metadata && (
                      <Button
                        size="sm"
                        variant="flat"
                        onPress={() => {
                          navigator.clipboard.writeText(JSON.stringify(selectedVectorData.metadata, null, 2));
                          addLog('success', 'Metadata copied to clipboard');
                        }}
                      >
                        <Copy className="w-3 h-3 mr-1" />
                        Copy JSON
                      </Button>
                    )}
                  </div>
                  {selectedVectorData.metadata ? (
                    <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 max-h-60 overflow-auto">
                      <pre className="text-xs font-mono text-gray-300">
                        {JSON.stringify(selectedVectorData.metadata, null, 2)}
                      </pre>
                    </div>
                  ) : (
                    <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 text-center">
                      <p className="text-sm text-gray-500 italic">No metadata</p>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <div className="text-center py-8">
                <AlertCircle className="w-12 h-12 text-gray-600 mx-auto mb-3" />
                <p className="text-gray-400">Vector data not available</p>
              </div>
            )}
          </ModalBody>
          <ModalFooter className="border-t border-gray-700">
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onVectorDetailClose}>
              Close
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
```

## Summary of Changes

1. **Imports**: Added `VectorEntry` type and `Eye` icon
2. **Hook**: Added `getVector` function from useRvLite
3. **State**: Added `selectedVectorId` and `selectedVectorData` state variables
4. **Modal**: Added `isVectorDetailOpen` disclosure
5. **Handler**: Created `handleViewVector` function
6. **Table**: Made vector IDs clickable and updated View Details button
7. **Modal Component**: Added complete Vector Inspector modal with:
   - Vector ID display with copy button
   - Dimensions display
   - Embedding values (first 20 + count if more)
   - Metadata as formatted JSON
   - Copy buttons for embedding and metadata
   - Dark theme styling matching existing UI
