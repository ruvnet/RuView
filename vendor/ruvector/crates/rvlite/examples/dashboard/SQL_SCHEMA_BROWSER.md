# SQL Schema Browser Feature

## Overview
The SQL Schema Browser is a new feature added to the RvLite dashboard that automatically tracks and displays SQL table schemas created through the SQL query interface.

## Implementation Summary

### Files Modified
- `/workspaces/ruvector/crates/rvlite/examples/dashboard/src/App.tsx`

### Key Components Added

#### 1. **TypeScript Interface**
```typescript
interface TableSchema {
  name: string;
  columns: Array<{
    name: string;
    type: string;
    isVector: boolean;
    dimensions?: number;
  }>;
  rowCount?: number;
}
```

#### 2. **State Management**
- `sqlTables: Map<string, TableSchema>` - Tracks all created tables
- `expandedTables: Set<string>` - Tracks which table details are expanded in UI

#### 3. **Icon Imports** (Lucide React)
- `Table2` - Table icon
- `Columns` - Column list icon
- `ChevronDown` / `ChevronRight` - Expand/collapse icons

#### 4. **Core Functions**

**`parseCreateTable(query: string): TableSchema | null`**
- Parses CREATE TABLE SQL statements
- Extracts table name, columns, and types
- Detects VECTOR columns with dimensions (e.g., VECTOR(3))

**`toggleTableExpansion(tableName: string)`**
- Expands/collapses table details in the UI

**`handleSelectTable(tableName: string)`**
- Auto-fills query input with `SELECT * FROM tableName`

**`handleDropTable(tableName: string)`**
- Confirms and executes DROP TABLE
- Updates schema browser state

#### 5. **Modified `handleExecuteSql`**
Now intercepts:
- **CREATE TABLE**: Parses schema and adds to `sqlTables`
- **DROP TABLE**: Removes table from `sqlTables`

### UI Features

The Schema Browser card appears in the SQL tab when tables exist:

1. **Table List**
   - Click table name to expand/collapse
   - Shows column count badge

2. **Per-Table Actions**
   - **Query button**: Auto-fills `SELECT * FROM table`
   - **Drop button**: Deletes table (with confirmation)

3. **Column Display** (when expanded)
   - Column name in monospace font
   - Type badges with color coding:
     - Purple: `VECTOR(n)`
     - Blue: `TEXT`
     - Green: `INTEGER`
     - Gray: Other types

### Example Usage

1. Run the sample query "Create Table":
   ```sql
   CREATE TABLE docs (id TEXT, content TEXT, embedding VECTOR(3))
   ```

2. The Schema Browser automatically appears showing:
   - Table name: `docs`
   - Columns: `id (TEXT)`, `content (TEXT)`, `embedding (VECTOR(3))`

3. Click the table to expand and see all column details

4. Click "Query" button to auto-fill: `SELECT * FROM docs`

5. Click "Drop" button to remove the table (with confirmation)

## Testing

Build successful:
```bash
npm run build
```

### Test Scenarios

1. **Create table with VECTOR column**
   - ✅ Schema parsed correctly
   - ✅ VECTOR dimension detected (e.g., VECTOR(3))
   - ✅ Table appears in Schema Browser

2. **Drop table**
   - ✅ Table removed from Schema Browser
   - ✅ Confirmation dialog shown

3. **Multiple tables**
   - ✅ All tables tracked independently
   - ✅ Expansion state preserved per table

4. **Column type detection**
   - ✅ TEXT columns (blue badge)
   - ✅ INTEGER columns (green badge)
   - ✅ VECTOR columns (purple badge with dimensions)

## Code Locations

- **Types**: Lines 104-113
- **State**: Lines 518-520
- **Parser**: Lines 872-907
- **Handlers**: Lines 909-946
- **Modified SQL executor**: Lines 948-980
- **UI Component**: Lines ~1701-1804 (SQL tab, before SQL Result card)

## Future Enhancements

Potential improvements:
- Row count tracking (execute `SELECT COUNT(*) FROM table`)
- Index visualization
- Table relationships/foreign keys
- Export schema as SQL DDL
- Schema comparison/diff view
