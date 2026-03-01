/**
 * BULK IMPORT FEATURE CODE SNIPPETS
 *
 * Copy these code blocks into src/App.tsx at the specified locations
 */

// ============================================================================
// 1. ICON IMPORT (Add to lucide-react imports, around line 78)
// ============================================================================
// Add FileSpreadsheet after XCircle:
/*
  XCircle,
  FileSpreadsheet,  // <-- ADD THIS
} from 'lucide-react';
*/

// ============================================================================
// 2. MODAL DISCLOSURE HOOK (Around line 526, after isScenariosOpen)
// ============================================================================
const { isOpen: isBulkImportOpen, onOpen: onBulkImportOpen, onClose: onBulkImportClose } = useDisclosure();

// ============================================================================
// 3. STATE VARIABLES (Around line 539, after importJson state)
// ============================================================================
// Bulk import states
const [bulkImportData, setBulkImportData] = useState('');
const [bulkImportFormat, setBulkImportFormat] = useState<'csv' | 'json'>('json');
const [bulkImportPreview, setBulkImportPreview] = useState<Array<{id: string, embedding: number[], metadata?: Record<string, unknown>}>>([]);
const [bulkImportProgress, setBulkImportProgress] = useState({ current: 0, total: 0, errors: 0 });
const [isBulkImporting, setIsBulkImporting] = useState(false);

// ============================================================================
// 4. CSV PARSER FUNCTION (Add after state declarations, around line 545)
// ============================================================================
// CSV Parser for bulk import
const parseCsvVectors = useCallback((csvText: string): Array<{id: string, embedding: number[], metadata?: Record<string, unknown>}> => {
  const lines = csvText.trim().split('\n');
  if (lines.length < 2) {
    throw new Error('CSV must have header row and at least one data row');
  }

  const header = lines[0].toLowerCase().split(',').map(h => h.trim());
  const idIndex = header.indexOf('id');
  const embeddingIndex = header.indexOf('embedding');
  const metadataIndex = header.indexOf('metadata');

  if (idIndex === -1 || embeddingIndex === -1) {
    throw new Error('CSV must have "id" and "embedding" columns');
  }

  const vectors: Array<{id: string, embedding: number[], metadata?: Record<string, unknown>}> = [];

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line) continue;

    // Simple CSV parsing (handles quoted fields)
    const values: string[] = [];
    let current = '';
    let inQuotes = false;

    for (let j = 0; j < line.length; j++) {
      const char = line[j];
      if (char === '"') {
        inQuotes = !inQuotes;
      } else if (char === ',' && !inQuotes) {
        values.push(current.trim());
        current = '';
      } else {
        current += char;
      }
    }
    values.push(current.trim());

    if (values.length < header.length) continue;

    try {
      const id = values[idIndex].replace(/^"(.*)"$/, '$1');
      const embeddingStr = values[embeddingIndex].replace(/^"(.*)"$/, '$1');
      const embedding = JSON.parse(embeddingStr);

      if (!Array.isArray(embedding) || !embedding.every(n => typeof n === 'number')) {
        throw new Error(`Invalid embedding format at row ${i + 1}`);
      }

      let metadata: Record<string, unknown> = {};
      if (metadataIndex !== -1 && values[metadataIndex]) {
        const metadataStr = values[metadataIndex].replace(/^"(.*)"$/, '$1').replace(/""/g, '"');
        metadata = JSON.parse(metadataStr);
      }

      vectors.push({ id, embedding, metadata });
    } catch (err) {
      console.error(`Error parsing row ${i + 1}:`, err);
      throw new Error(`Failed to parse row ${i + 1}: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  return vectors;
}, []);

// ============================================================================
// 5. JSON PARSER FUNCTION (After CSV parser)
// ============================================================================
// JSON Parser for bulk import
const parseJsonVectors = useCallback((jsonText: string): Array<{id: string, embedding: number[], metadata?: Record<string, unknown>}> => {
  try {
    const data = JSON.parse(jsonText);

    if (!Array.isArray(data)) {
      throw new Error('JSON must be an array of vectors');
    }

    return data.map((item, index) => {
      if (!item.id || !item.embedding) {
        throw new Error(`Vector at index ${index} missing required "id" or "embedding" field`);
      }

      if (!Array.isArray(item.embedding) || !item.embedding.every((n: unknown) => typeof n === 'number')) {
        throw new Error(`Vector at index ${index} has invalid embedding format`);
      }

      return {
        id: String(item.id),
        embedding: item.embedding,
        metadata: item.metadata || {}
      };
    });
  } catch (err) {
    throw new Error(`Failed to parse JSON: ${err instanceof Error ? err.message : String(err)}`);
  }
}, []);

// ============================================================================
// 6. PREVIEW HANDLER (After parsing functions)
// ============================================================================
// Handle preview generation
const handleGeneratePreview = useCallback(() => {
  if (!bulkImportData.trim()) {
    addLog('warning', 'No data to preview');
    return;
  }

  try {
    const vectors = bulkImportFormat === 'csv'
      ? parseCsvVectors(bulkImportData)
      : parseJsonVectors(bulkImportData);

    setBulkImportPreview(vectors.slice(0, 5));
    addLog('success', `Preview generated: ${vectors.length} vectors found (showing first 5)`);
  } catch (err) {
    addLog('error', `Preview failed: ${err instanceof Error ? err.message : String(err)}`);
    setBulkImportPreview([]);
  }
}, [bulkImportData, bulkImportFormat, parseCsvVectors, parseJsonVectors, addLog]);

// ============================================================================
// 7. BULK IMPORT HANDLER (After preview handler)
// ============================================================================
// Handle bulk import execution
const handleBulkImport = useCallback(async () => {
  if (!bulkImportData.trim()) {
    addLog('warning', 'No data to import');
    return;
  }

  try {
    setIsBulkImporting(true);
    const vectors = bulkImportFormat === 'csv'
      ? parseCsvVectors(bulkImportData)
      : parseJsonVectors(bulkImportData);

    setBulkImportProgress({ current: 0, total: vectors.length, errors: 0 });

    let successCount = 0;
    let errorCount = 0;

    for (let i = 0; i < vectors.length; i++) {
      try {
        const { id, embedding, metadata } = vectors[i];
        insertVectorWithId(id, embedding, metadata || {});
        successCount++;
      } catch (err) {
        console.error(`Failed to import vector ${vectors[i].id}:`, err);
        errorCount++;
      }

      setBulkImportProgress({ current: i + 1, total: vectors.length, errors: errorCount });

      // Small delay to prevent UI blocking
      if (i % 10 === 0) {
        await new Promise(resolve => setTimeout(resolve, 0));
      }
    }

    refreshVectors();
    addLog('success', `Bulk import complete: ${successCount} success, ${errorCount} errors`);

    // Reset and close
    setTimeout(() => {
      setBulkImportData('');
      setBulkImportPreview([]);
      setBulkImportProgress({ current: 0, total: 0, errors: 0 });
      setIsBulkImporting(false);
      onBulkImportClose();
    }, 1500);

  } catch (err) {
    addLog('error', `Bulk import failed: ${err instanceof Error ? err.message : String(err)}`);
    setIsBulkImporting(false);
  }
}, [bulkImportData, bulkImportFormat, parseCsvVectors, parseJsonVectors, insertVectorWithId, refreshVectors, addLog, onBulkImportClose]);

// ============================================================================
// 8. FILE UPLOAD HANDLER (After bulk import handler)
// ============================================================================
// Handle file upload
const handleBulkImportFileUpload = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
  const file = event.target.files?.[0];
  if (!file) return;

  const reader = new FileReader();
  reader.onload = (e) => {
    const text = e.target?.result as string;
    setBulkImportData(text);

    // Auto-detect format from file extension
    const extension = file.name.split('.').pop()?.toLowerCase();
    if (extension === 'csv') {
      setBulkImportFormat('csv');
    } else if (extension === 'json') {
      setBulkImportFormat('json');
    }

    addLog('info', `File loaded: ${file.name} (${(file.size / 1024).toFixed(1)} KB)`);
  };
  reader.onerror = () => {
    addLog('error', 'Failed to read file');
  };
  reader.readAsText(file);
}, [addLog]);

// ============================================================================
// 9. BULK IMPORT BUTTON (Add to Quick Actions, around line 1964)
// ============================================================================
/*
Add this button after the "Import Data" button in Quick Actions CardBody:

              <Button fullWidth variant="flat" className="justify-start" onPress={onImportOpen}>
                <Upload className="w-4 h-4 mr-2" />
                Import Data
              </Button>
              <Button fullWidth variant="flat" color="success" className="justify-start" onPress={onBulkImportOpen}>
                <FileSpreadsheet className="w-4 h-4 mr-2" />
                Bulk Import Vectors
              </Button>
              <Button fullWidth variant="flat" color="danger" className="justify-start" onPress={handleClearAll}>
                <Trash2 className="w-4 h-4 mr-2" />
                Clear All Data
              </Button>
*/

// ============================================================================
// 10. BULK IMPORT MODAL (Add after Import Modal, around line 2306)
// ============================================================================
/*
Add this modal component after the {/* Import Modal *\\/} section:
*/

export const BulkImportModal = () => (
  <Modal isOpen={isBulkImportOpen} onClose={onBulkImportClose} size="4xl" scrollBehavior="inside">
    <ModalContent className="bg-gray-900 border border-gray-700">
      <ModalHeader className="text-white border-b border-gray-700">
        <div className="flex items-center gap-2">
          <FileSpreadsheet className="w-5 h-5 text-green-400" />
          <span>Bulk Import Vectors</span>
        </div>
      </ModalHeader>
      <ModalBody className="py-6">
        <div className="space-y-4">
          {/* Format Selector */}
          <div className="flex gap-4 items-end">
            <Select
              label="Format"
              selectedKeys={[bulkImportFormat]}
              onChange={(e) => setBulkImportFormat(e.target.value as 'csv' | 'json')}
              className="max-w-xs"
              classNames={{
                label: "text-gray-300",
                value: "text-white",
                trigger: "bg-gray-800 border-gray-600 hover:border-gray-500",
              }}
            >
              <SelectItem key="json" value="json">
                <div className="flex items-center gap-2">
                  <FileJson className="w-4 h-4" />
                  <span>JSON</span>
                </div>
              </SelectItem>
              <SelectItem key="csv" value="csv">
                <div className="flex items-center gap-2">
                  <FileSpreadsheet className="w-4 h-4" />
                  <span>CSV</span>
                </div>
              </SelectItem>
            </Select>

            {/* File Upload */}
            <div className="flex-1">
              <label className="block">
                <input
                  type="file"
                  accept=".csv,.json"
                  onChange={handleBulkImportFileUpload}
                  className="hidden"
                  id="bulk-import-file"
                />
                <Button
                  as="span"
                  variant="flat"
                  color="primary"
                  className="cursor-pointer"
                  onPress={() => document.getElementById('bulk-import-file')?.click()}
                >
                  <Upload className="w-4 h-4 mr-2" />
                  Upload File
                </Button>
              </label>
            </div>

            <Button
              variant="flat"
              color="secondary"
              onPress={handleGeneratePreview}
              isDisabled={!bulkImportData.trim()}
            >
              <Eye className="w-4 h-4 mr-2" />
              Preview
            </Button>
          </div>

          {/* Format Guide */}
          <Card className="bg-gray-800/50 border border-gray-700">
            <CardBody className="p-3">
              <p className="text-xs text-gray-400 mb-2">
                <strong className="text-gray-300">Expected Format:</strong>
              </p>
              {bulkImportFormat === 'csv' ? (
                <pre className="text-xs font-mono text-green-400 overflow-x-auto">
{`id,embedding,metadata
vec1,"[1.0,2.0,3.0]","{\\"category\\":\\"test\\"}"
vec2,"[4.0,5.0,6.0]","{}"`}
                </pre>
              ) : (
                <pre className="text-xs font-mono text-blue-400 overflow-x-auto">
{`[
  { "id": "vec1", "embedding": [1.0, 2.0, 3.0], "metadata": { "category": "test" } },
  { "id": "vec2", "embedding": [4.0, 5.0, 6.0], "metadata": {} }
]`}
                </pre>
              )}
            </CardBody>
          </Card>

          {/* Data Input */}
          <Textarea
            label={`Paste ${bulkImportFormat.toUpperCase()} Data`}
            placeholder={`Paste your ${bulkImportFormat.toUpperCase()} data here or upload a file...`}
            value={bulkImportData}
            onChange={(e) => setBulkImportData(e.target.value)}
            minRows={8}
            maxRows={15}
            classNames={{
              label: "text-gray-300",
              input: "font-mono bg-gray-800/50 text-white placeholder:text-gray-500",
              inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
            }}
          />

          {/* Preview Section */}
          {bulkImportPreview.length > 0 && (
            <Card className="bg-gray-800/50 border border-gray-700">
              <CardHeader>
                <div className="flex items-center gap-2">
                  <Eye className="w-4 h-4 text-cyan-400" />
                  <span className="text-sm font-semibold text-white">Preview (first 5 vectors)</span>
                </div>
              </CardHeader>
              <CardBody>
                <div className="space-y-2 max-h-60 overflow-y-auto">
                  {bulkImportPreview.map((vec, idx) => (
                    <div key={idx} className="p-2 bg-gray-900/50 rounded text-xs font-mono border border-gray-700">
                      <div className="text-cyan-400">ID: {vec.id}</div>
                      <div className="text-gray-400">
                        Embedding: [{vec.embedding.slice(0, 3).join(', ')}
                        {vec.embedding.length > 3 && `, ... (${vec.embedding.length} dims)`}]
                      </div>
                      {vec.metadata && Object.keys(vec.metadata).length > 0 && (
                        <div className="text-purple-400">
                          Metadata: {JSON.stringify(vec.metadata)}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </CardBody>
            </Card>
          )}

          {/* Progress Indicator */}
          {isBulkImporting && (
            <Card className="bg-gray-800/50 border border-gray-700">
              <CardBody className="p-4">
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="text-gray-300">Importing vectors...</span>
                    <span className="text-white font-mono">
                      {bulkImportProgress.current} / {bulkImportProgress.total}
                    </span>
                  </div>
                  <Progress
                    value={(bulkImportProgress.current / bulkImportProgress.total) * 100}
                    color="success"
                    className="max-w-full"
                  />
                  {bulkImportProgress.errors > 0 && (
                    <p className="text-xs text-red-400">
                      Errors: {bulkImportProgress.errors}
                    </p>
                  )}
                </div>
              </CardBody>
            </Card>
          )}
        </div>
      </ModalBody>
      <ModalFooter className="border-t border-gray-700">
        <Button
          variant="flat"
          className="bg-gray-800 text-white hover:bg-gray-700"
          onPress={onBulkImportClose}
          isDisabled={isBulkImporting}
        >
          Cancel
        </Button>
        <Button
          color="success"
          onPress={handleBulkImport}
          isDisabled={!bulkImportData.trim() || isBulkImporting}
          isLoading={isBulkImporting}
        >
          <Upload className="w-4 h-4 mr-2" />
          Import {bulkImportPreview.length > 0 && `(${bulkImportPreview.length} vectors)`}
        </Button>
      </ModalFooter>
    </ModalContent>
  </Modal>
);
