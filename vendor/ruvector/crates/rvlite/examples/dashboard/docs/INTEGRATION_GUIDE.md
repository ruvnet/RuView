# Bulk Vector Import - Integration Guide

## Overview
Complete implementation guide for adding CSV/JSON bulk vector import to the RvLite dashboard.

## Files Created

### 1. Documentation
- `BULK_IMPORT_IMPLEMENTATION.md` - Detailed implementation guide with line numbers
- `docs/bulk-import-code.tsx` - All code snippets ready to copy
- `docs/INTEGRATION_GUIDE.md` - This file
- `apply-bulk-import.sh` - Automated script for basic changes

### 2. Sample Data
- `docs/sample-bulk-import.csv` - Sample CSV data with 8 vectors
- `docs/sample-bulk-import.json` - Sample JSON data with 8 vectors

## Quick Integration (3 Steps)

### Step 1: Run Automated Script
```bash
cd /workspaces/ruvector/crates/rvlite/examples/dashboard
chmod +x apply-bulk-import.sh
./apply-bulk-import.sh
```

This will add:
- FileSpreadsheet icon import
- Modal disclosure hook
- State variables

### Step 2: Add Utility Functions

Open `src/App.tsx` and find line ~545 (after state declarations).

Copy from `docs/bulk-import-code.tsx`:
- Section 4: CSV Parser Function
- Section 5: JSON Parser Function
- Section 6: Preview Handler
- Section 7: Bulk Import Handler
- Section 8: File Upload Handler

Paste all functions in order after the state declarations.

### Step 3: Add UI Components

**3A. Add Button (line ~1964)**

Find the Quick Actions section and add the Bulk Import button:
```typescript
<Button fullWidth variant="flat" color="success" className="justify-start" onPress={onBulkImportOpen}>
  <FileSpreadsheet className="w-4 h-4 mr-2" />
  Bulk Import Vectors
</Button>
```

**3B. Add Modal (line ~2306)**

After the Import Modal closing tag, add the entire Bulk Import Modal from Section 10 of `docs/bulk-import-code.tsx`.

## Manual Integration (Alternative)

If you prefer manual integration or the script fails:

### 1. Icon Import (~line 78)
```typescript
  XCircle,
  FileSpreadsheet,  // ADD THIS
} from 'lucide-react';
```

### 2. Modal Hook (~line 526)
```typescript
const { isOpen: isBulkImportOpen, onOpen: onBulkImportOpen, onClose: onBulkImportClose } = useDisclosure();
```

### 3. State Variables (~line 539)
```typescript
const [bulkImportData, setBulkImportData] = useState('');
const [bulkImportFormat, setBulkImportFormat] = useState<'csv' | 'json'>('json');
const [bulkImportPreview, setBulkImportPreview] = useState<Array<{id: string, embedding: number[], metadata?: Record<string, unknown>}>>([]);
const [bulkImportProgress, setBulkImportProgress] = useState({ current: 0, total: 0, errors: 0 });
const [isBulkImporting, setIsBulkImporting] = useState(false);
```

### 4-8. Functions
Copy all functions from `docs/bulk-import-code.tsx` sections 4-8.

### 9-10. UI Components
Copy button and modal from `docs/bulk-import-code.tsx` sections 9-10.

## Testing

### Test 1: CSV Upload
1. Start the dashboard: `npm run dev`
2. Click "Bulk Import Vectors" in Quick Actions
3. Select "CSV" format
4. Upload `docs/sample-bulk-import.csv` OR paste its contents
5. Click "Preview" - should show 5 vectors
6. Click "Import" - should import all 8 vectors
7. Verify in Vectors tab

### Test 2: JSON Upload
1. Click "Bulk Import Vectors"
2. Select "JSON" format
3. Upload `docs/sample-bulk-import.json` OR paste its contents
4. Click "Preview" - should show 5 vectors
5. Click "Import" - should import all 8 vectors
6. Verify success message and vector count

### Test 3: Error Handling
1. Try invalid CSV (missing header)
2. Try invalid JSON (malformed)
3. Try empty data
4. Verify error messages in logs

### Test 4: Progress Indicator
1. Create a larger dataset (50+ vectors)
2. Import and watch progress bar
3. Verify it completes and closes modal

## Expected Behavior

### CSV Format
```csv
id,embedding,metadata
vec1,"[1.0,2.0,3.0]","{\"category\":\"test\"}"
vec2,"[4.0,5.0,6.0]","{}"
```

### JSON Format
```json
[
  { "id": "vec1", "embedding": [1.0, 2.0, 3.0], "metadata": { "category": "test" } },
  { "id": "vec2", "embedding": [4.0, 5.0, 6.0] }
]
```

### Features
- ✅ File upload (.csv, .json)
- ✅ Direct text paste
- ✅ Format selector (CSV/JSON)
- ✅ Preview (first 5 vectors)
- ✅ Progress indicator
- ✅ Error tracking
- ✅ Auto-close on success
- ✅ Dark theme styling
- ✅ Responsive layout

## File Structure After Integration

```
src/
  App.tsx                              (modified)
  hooks/
    useRvLite.ts                       (unchanged)
docs/
  BULK_IMPORT_IMPLEMENTATION.md        (new)
  INTEGRATION_GUIDE.md                 (new)
  bulk-import-code.tsx                 (new)
  sample-bulk-import.csv               (new)
  sample-bulk-import.json              (new)
apply-bulk-import.sh                   (new)
```

## Troubleshooting

### Issue: Import button not showing
**Fix:** Verify FileSpreadsheet icon imported and onBulkImportOpen defined

### Issue: Modal not opening
**Fix:** Check useDisclosure hook added and isBulkImportOpen variable exists

### Issue: Preview fails
**Fix:** Verify parseCsvVectors and parseJsonVectors functions added

### Issue: Import fails silently
**Fix:** Check insertVectorWithId and refreshVectors are in dependency arrays

### Issue: File upload not working
**Fix:** Verify handleBulkImportFileUpload function added

## Integration Checklist

- [ ] Run apply-bulk-import.sh or manually add imports/hooks/state
- [ ] Add all 5 utility functions (CSV parser, JSON parser, preview, import, file upload)
- [ ] Add Bulk Import button to Quick Actions
- [ ] Add Bulk Import Modal component
- [ ] Test with sample CSV file
- [ ] Test with sample JSON file
- [ ] Test error handling
- [ ] Test progress indicator
- [ ] Verify dark theme styling matches
- [ ] Check logs for success/error messages

## Support

If you encounter issues:
1. Check browser console for errors
2. Verify all functions copied correctly
3. Ensure no duplicate state variables
4. Check dependency arrays in useCallback
5. Verify modal disclosure hooks match

## Success Metrics

After integration, you should be able to:
- ✅ Import 100+ vectors in under 2 seconds
- ✅ Preview data before import
- ✅ See real-time progress
- ✅ Handle errors gracefully
- ✅ Auto-close modal on success
- ✅ View imported vectors immediately

## Next Steps

After successful integration:
1. Test with production data
2. Consider adding batch size limits
3. Add export to CSV/JSON
4. Implement undo for bulk operations
5. Add data validation rules
6. Create import templates

---

**Implementation Status:** Code complete, ready for integration
**Testing Status:** Sample data provided, manual testing required
**File Location:** `/workspaces/ruvector/crates/rvlite/examples/dashboard/`
