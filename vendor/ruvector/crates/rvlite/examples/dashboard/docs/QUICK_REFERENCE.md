# Bulk Vector Import - Quick Reference Card

## 🚀 30-Second Integration

```bash
# 1. Run script
./apply-bulk-import.sh

# 2. Add functions (copy sections 4-8 from bulk-import-code.tsx)
# Paste after line ~850 in App.tsx

# 3. Add button (copy section 9 from bulk-import-code.tsx)
# Paste after line ~1956 in App.tsx

# 4. Add modal (copy section 10 from bulk-import-code.tsx)
# Paste after line ~2296 in App.tsx

# 5. Test
npm run dev
```

## 📝 What to Add Where

| What | Where | Lines | Section |
|------|-------|-------|---------|
| FileSpreadsheet icon | Line ~78 | 1 | 1 |
| Modal hook | Line ~527 | 1 | 2 |
| State variables | Line ~539 | 5 | 3 |
| CSV parser | Line ~850 | 45 | 4 |
| JSON parser | Line ~890 | 30 | 5 |
| Preview handler | Line ~920 | 15 | 6 |
| Import handler | Line ~935 | 55 | 7 |
| File handler | Line ~990 | 20 | 8 |
| Button | Line ~1964 | 4 | 9 |
| Modal | Line ~2306 | 155 | 10 |

## 📂 File Guide

| File | Purpose | When to Use |
|------|---------|-------------|
| INTEGRATION_GUIDE.md | Step-by-step instructions | First time integration |
| bulk-import-code.tsx | Copy-paste code | Actually coding |
| VISUAL_INTEGRATION_MAP.md | Diagrams & structure | Understanding flow |
| IMPLEMENTATION_SUMMARY.md | Feature overview | Before starting |
| sample-bulk-import.csv | Test CSV | Testing CSV import |
| sample-bulk-import.json | Test JSON | Testing JSON import |

## 🎯 Code Sections (bulk-import-code.tsx)

| Section | What | Lines |
|---------|------|-------|
| 1 | Icon import | 1 |
| 2 | Modal hook | 1 |
| 3 | State (5 vars) | 5 |
| 4 | CSV parser | 45 |
| 5 | JSON parser | 30 |
| 6 | Preview handler | 15 |
| 7 | Import handler | 55 |
| 8 | File handler | 20 |
| 9 | Button | 4 |
| 10 | Modal | 155 |

## 🧪 Testing Checklist

- [ ] CSV upload works
- [ ] JSON upload works
- [ ] Preview shows 5 vectors
- [ ] Progress bar appears
- [ ] Success message logged
- [ ] Vector count updates
- [ ] Modal auto-closes
- [ ] Error handling works

## 🔧 Common Issues

| Problem | Solution |
|---------|----------|
| Button not visible | Add icon import (Section 1) |
| Modal won't open | Add hook (Section 2) |
| Preview fails | Add parsers (Sections 4-5) |
| Import fails | Add handler (Section 7) |

## 📊 CSV Format

```csv
id,embedding,metadata
vec1,"[1.0,2.0,3.0]","{\"key\":\"value\"}"
```

## 📋 JSON Format

```json
[
  { "id": "vec1", "embedding": [1.0, 2.0, 3.0], "metadata": {} }
]
```

## ⚡ State Variables

```typescript
bulkImportData: string               // Raw text
bulkImportFormat: 'csv' | 'json'     // Format type
bulkImportPreview: Vector[]          // Preview data
bulkImportProgress: {current, total, errors}
isBulkImporting: boolean             // In progress
```

## 🔄 Functions

```typescript
parseCsvVectors(text)      → Vector[]
parseJsonVectors(text)     → Vector[]
handleGeneratePreview()    → void
handleBulkImport()         → Promise<void>
handleBulkImportFileUpload() → void
```

## 🎨 UI Components

```typescript
Button: Bulk Import Vectors        (Quick Actions)
Modal: Bulk Import Modal           (After Import Modal)
  ├─ Format Selector
  ├─ File Upload
  ├─ Preview Button
  ├─ Format Guide
  ├─ Data Textarea
  ├─ Preview Panel
  └─ Progress Indicator
```

## 📏 Line Numbers

```
~78   : Icon import
~527  : Modal hook
~539  : State variables
~850  : Functions start
~1964 : Button location
~2306 : Modal location
```

## 🎯 Integration Order

1. ✅ Icon (1 line)
2. ✅ Hook (1 line)
3. ✅ State (5 lines)
4. ✅ Functions (5 functions, ~165 lines)
5. ✅ Button (4 lines)
6. ✅ Modal (155 lines)

**Total: ~331 lines**

## 🚦 Status After Integration

| Feature | Status |
|---------|--------|
| CSV import | ✅ Working |
| JSON import | ✅ Working |
| File upload | ✅ Working |
| Preview | ✅ Working |
| Progress | ✅ Working |
| Errors | ✅ Handled |
| Theme | ✅ Dark |
| Tests | ⏳ Pending |

## 📞 Help

- Integration: `INTEGRATION_GUIDE.md`
- Code: `bulk-import-code.tsx`
- Visual: `VISUAL_INTEGRATION_MAP.md`
- Overview: `IMPLEMENTATION_SUMMARY.md`

---

**Quick Copy-Paste**:
```bash
cat docs/bulk-import-code.tsx
```
