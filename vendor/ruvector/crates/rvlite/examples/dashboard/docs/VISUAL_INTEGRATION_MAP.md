# Visual Integration Map - Bulk Vector Import

## App.tsx Structure with Integration Points

```
App.tsx
│
├── IMPORTS (Lines 1-90)
│   ├── React imports
│   ├── HeroUI components
│   ├── Lucide icons (Lines 31-77)
│   │   └── ✨ ADD: FileSpreadsheet (line ~78)
│   ├── Recharts
│   └── Custom hooks
│
├── TYPE DEFINITIONS (Lines 91-500)
│   ├── LogEntry interface
│   ├── VectorDisplay interface
│   └── Other types...
│
├── COMPONENT FUNCTION START (Line ~501)
│   │
│   ├── HOOKS (Lines 502-526)
│   │   ├── useRvLite hook
│   │   ├── useLearning hook
│   │   ├── useState hooks
│   │   ├── useRef hooks
│   │   └── Modal disclosure hooks (Lines 512-526)
│   │       ├── isAddOpen
│   │       ├── isSettingsOpen
│   │       ├── isTripleOpen
│   │       ├── isImportOpen
│   │       ├── isScenariosOpen
│   │       └── ✨ ADD: isBulkImportOpen (line ~527)
│   │
│   ├── FORM STATE (Lines 527-538)
│   │   ├── newVector
│   │   ├── searchQuery
│   │   ├── filterConditions
│   │   ├── newTriple
│   │   └── importJson (line 538)
│   │
│   ├── ✨ NEW BULK IMPORT STATE (Insert after line ~538)
│   │   ├── bulkImportData
│   │   ├── bulkImportFormat
│   │   ├── bulkImportPreview
│   │   ├── bulkImportProgress
│   │   └── isBulkImporting
│   │
│   ├── UTILITY FUNCTIONS (Lines 539-850)
│   │   ├── addLog callback
│   │   ├── loadSampleData
│   │   ├── handleAddVector
│   │   ├── handleSearch
│   │   ├── handleImport
│   │   ├── ...other handlers...
│   │   │
│   │   └── ✨ ADD NEW FUNCTIONS (after existing handlers, ~line 850)
│   │       ├── parseCsvVectors()
│   │       ├── parseJsonVectors()
│   │       ├── handleGeneratePreview()
│   │       ├── handleBulkImport()
│   │       └── handleBulkImportFileUpload()
│   │
│   ├── JSX RETURN (Lines 851-2500+)
│   │   │
│   │   ├── Header Section (Lines 851-1000)
│   │   │
│   │   ├── Main Dashboard (Lines 1000-1900)
│   │   │
│   │   ├── Quick Actions Panel (Lines 1920-1962)
│   │   │   ├── Card Header
│   │   │   └── Card Body (Lines 1940-1961)
│   │   │       ├── Load Sample Scenarios button
│   │   │       ├── Save to Browser button
│   │   │       ├── Export JSON button
│   │   │       ├── Import Data button (line ~1953)
│   │   │       ├── ✨ ADD: Bulk Import Vectors button (after line ~1956)
│   │   │       └── Clear All Data button
│   │   │
│   │   ├── Other Dashboard Sections (Lines 1963-2270)
│   │   │
│   │   └── MODALS Section (Lines 2271-2500+)
│   │       ├── Add Vector Modal (Lines 2100-2180)
│   │       ├── Settings Modal (Lines 2181-2230)
│   │       ├── RDF Triple Modal (Lines 2231-2273)
│   │       ├── Import Modal (Lines 2274-2296)
│   │       ├── ✨ ADD: Bulk Import Modal (after line ~2296)
│   │       └── Sample Scenarios Modal (Lines 2298+)
│   │
│   └── COMPONENT FUNCTION END
│
└── EXPORTS (Line ~2500+)
```

## Integration Points Summary

### 🎯 Point 1: Icon Import (Line ~78)
```typescript
Location: After XCircle in lucide-react imports
Action: Add 1 line
Code: See bulk-import-code.tsx Section 1
```

### 🎯 Point 2: Modal Hook (Line ~527)
```typescript
Location: After isScenariosOpen useDisclosure
Action: Add 1 line
Code: See bulk-import-code.tsx Section 2
```

### 🎯 Point 3: State Variables (Line ~539)
```typescript
Location: After importJson useState
Action: Add 5 lines
Code: See bulk-import-code.tsx Section 3
```

### 🎯 Point 4-8: Functions (Line ~850)
```typescript
Location: After existing handler functions
Action: Add ~200 lines (5 functions)
Code: See bulk-import-code.tsx Sections 4-8
```

### 🎯 Point 9: Button (Line ~1964)
```typescript
Location: In Quick Actions CardBody, after Import Data button
Action: Add 4 lines
Code: See bulk-import-code.tsx Section 9
```

### 🎯 Point 10: Modal (Line ~2306)
```typescript
Location: After Import Modal, before Sample Scenarios Modal
Action: Add ~155 lines
Code: See bulk-import-code.tsx Section 10
```

## Code Flow Diagram

```
User Action: "Bulk Import Vectors" button clicked
                    ↓
            onBulkImportOpen()
                    ↓
            Modal Opens
                    ↓
      ┌─────────────┴─────────────┐
      │                           │
      ↓                           ↓
  Upload File              Paste Text
      ↓                           ↓
handleBulkImportFileUpload()   setBulkImportData()
      │                           │
      └─────────────┬─────────────┘
                    ↓
            Select Format (CSV/JSON)
                    ↓
            Click "Preview"
                    ↓
        handleGeneratePreview()
                    ↓
           ┌────────┴────────┐
           ↓                 ↓
    parseCsvVectors()  parseJsonVectors()
           │                 │
           └────────┬────────┘
                    ↓
          setBulkImportPreview()
                    ↓
        Display first 5 vectors
                    ↓
          User clicks "Import"
                    ↓
         handleBulkImport()
                    ↓
           ┌────────┴────────┐
           ↓                 ↓
    parseCsvVectors()  parseJsonVectors()
           │                 │
           └────────┬────────┘
                    ↓
        Loop through vectors
                    ↓
        For each vector:
          ├─ insertVectorWithId()
          ├─ Update progress
          └─ Handle errors
                    ↓
          refreshVectors()
                    ↓
        addLog(success/error)
                    ↓
          Wait 1.5 seconds
                    ↓
            Reset state
                    ↓
          onBulkImportClose()
```

## Data Flow Diagram

```
CSV File                           JSON File
    ↓                                  ↓
    ├── File Upload ──────────────────┤
    ↓                                  ↓
FileReader.readAsText()           FileReader.readAsText()
    ↓                                  ↓
    ├── Raw Text ─────────────────────┤
    ↓                                  ↓
setBulkImportData(text)           setBulkImportData(text)
    │                                  │
    │   Format: CSV                    │   Format: JSON
    ↓                                  ↓
parseCsvVectors()                 parseJsonVectors()
    │                                  │
    ├── Split lines                    ├── JSON.parse()
    ├── Parse header                   ├── Validate array
    ├── Parse each row                 ├── Validate fields
    ├── Extract id                     ├── Extract id
    ├── Parse embedding                ├── Extract embedding
    ├── Parse metadata                 ├── Extract metadata
    └── Validate types                 └── Validate types
    │                                  │
    └──────────┬──────────────────────┘
               ↓
    Vector Array: [{id, embedding, metadata}, ...]
               ↓
               ├── Preview Mode ─────→ setBulkImportPreview(first 5)
               │                               ↓
               │                       Display in modal
               ↓
         Import Mode
               ↓
      For each vector:
        insertVectorWithId(id, embedding, metadata)
               ↓
      refreshVectors()
               ↓
      Update dashboard
```

## State Management Flow

```
Initial State:
├── bulkImportData = ''
├── bulkImportFormat = 'json'
├── bulkImportPreview = []
├── bulkImportProgress = {current: 0, total: 0, errors: 0}
└── isBulkImporting = false

User Uploads File:
├── bulkImportData = '<file contents>'
├── bulkImportFormat = 'csv' (auto-detected)
└── Other states unchanged

User Clicks Preview:
├── bulkImportPreview = [vec1, vec2, vec3, vec4, vec5]
└── Other states unchanged

User Clicks Import:
├── isBulkImporting = true
├── bulkImportProgress updates in loop:
│   ├── current: 0 → 1 → 2 → ... → total
│   ├── total: <vector count>
│   └── errors: <error count>
└── Other states unchanged

Import Complete:
├── isBulkImporting = false (after delay)
├── bulkImportData = '' (reset)
├── bulkImportPreview = [] (reset)
├── bulkImportProgress = {current: 0, total: 0, errors: 0} (reset)
└── Modal closes
```

## Component Hierarchy

```
App Component
└── JSX Return
    ├── Header
    ├── Dashboard Grid
    │   ├── Left Panel (Charts)
    │   └── Right Panel
    │       └── Quick Actions Card
    │           └── Button List
    │               ├── Load Scenarios
    │               ├── Save to Browser
    │               ├── Export JSON
    │               ├── Import Data
    │               ├── ✨ Bulk Import Vectors  ← NEW
    │               └── Clear All Data
    └── Modals
        ├── Add Vector Modal
        ├── Settings Modal
        ├── RDF Triple Modal
        ├── Import Modal
        ├── ✨ Bulk Import Modal  ← NEW
        │   ├── Modal Header (title + icon)
        │   ├── Modal Body
        │   │   ├── Format Selector (CSV/JSON)
        │   │   ├── File Upload Button
        │   │   ├── Preview Button
        │   │   ├── Format Guide Card
        │   │   ├── Data Textarea
        │   │   ├── Preview Card (conditional)
        │   │   │   └── Vector List (first 5)
        │   │   └── Progress Card (conditional)
        │   │       ├── Progress Bar
        │   │       └── Statistics
        │   └── Modal Footer
        │       ├── Cancel Button
        │       └── Import Button
        └── Sample Scenarios Modal
```

## File Structure Impact

```
dashboard/
├── src/
│   └── App.tsx  ← MODIFIED
│       ├── + 1 import line
│       ├── + 1 hook line
│       ├── + 5 state lines
│       ├── + ~200 function lines
│       ├── + 4 button lines
│       └── + ~155 modal lines
│       TOTAL: ~366 new lines
│
├── docs/  ← NEW FOLDER
│   ├── BULK_IMPORT_IMPLEMENTATION.md
│   ├── INTEGRATION_GUIDE.md
│   ├── IMPLEMENTATION_SUMMARY.md
│   ├── VISUAL_INTEGRATION_MAP.md  ← This file
│   ├── bulk-import-code.tsx
│   ├── sample-bulk-import.csv
│   └── sample-bulk-import.json
│
└── apply-bulk-import.sh  ← NEW SCRIPT
```

## Dependencies Graph

```
Bulk Import Feature
├── React (useState, useCallback)
├── HeroUI Components
│   ├── Modal
│   ├── Button
│   ├── Select
│   ├── Textarea
│   ├── Card
│   └── Progress
├── Lucide Icons
│   ├── FileSpreadsheet
│   ├── FileJson
│   ├── Upload
│   └── Eye
└── RvLite Hooks
    ├── insertVectorWithId()
    ├── refreshVectors()
    └── addLog()

NO NEW DEPENDENCIES REQUIRED
```

## Testing Checklist with Line References

- [ ] Line ~78: FileSpreadsheet icon imported
- [ ] Line ~527: isBulkImportOpen hook added
- [ ] Line ~539: All 5 state variables added
- [ ] Line ~850: All 5 functions added (parseCsv, parseJson, preview, import, fileUpload)
- [ ] Line ~1964: Bulk Import button added to Quick Actions
- [ ] Line ~2306: Bulk Import Modal added
- [ ] Test CSV upload with sample-bulk-import.csv
- [ ] Test JSON upload with sample-bulk-import.json
- [ ] Test preview functionality
- [ ] Test progress indicator
- [ ] Test error handling
- [ ] Test auto-close on success
- [ ] Verify dark theme styling
- [ ] Verify logs show success/error messages

---

**Quick Reference**: All code snippets are in `docs/bulk-import-code.tsx`
**Integration Guide**: See `docs/INTEGRATION_GUIDE.md`
**Full Details**: See `docs/BULK_IMPORT_IMPLEMENTATION.md`
