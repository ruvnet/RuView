# Filter Builder UI Demo

## Visual Preview

### Before (Current UI)
```
┌─────────────────────────────────────────────────────┐
│ ☑ Use metadata filter                              │
│                                                     │
│ ┌─────────────────────────────────────────────┐   │
│ │  🔍  {"category": "ML"}                     │   │
│ └─────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### After (New Filter Builder UI)
```
┌────────────────────────────────────────────────────────────────┐
│ ☑ Use metadata filter                                         │
│                                                                │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │  🔍 Filter Builder              [Show JSON]  [+ Add]     │ │
│ ├──────────────────────────────────────────────────────────┤ │
│ │                                                          │ │
│ │      ┌────────┐  ┌──────────┐  ┌────────┐  [🗑]        │ │
│ │      │category│  │Equals (=)│  │   ML   │              │ │
│ │      └────────┘  └──────────┘  └────────┘              │ │
│ │                                                          │ │
│ │ AND  ┌────────┐  ┌──────────┐  ┌────────┐  [🗑]        │ │
│ │      │ price  │  │   < (<)  │  │  100   │              │ │
│ │      └────────┘  └──────────┘  └────────┘              │ │
│ │                                                          │ │
│ ├──────────────────────────────────────────────────────────┤ │
│ │  Generated Filter JSON:                                 │ │
│ │  ┌────────────────────────────────────────────────────┐ │ │
│ │  │  {                                                  │ │ │
│ │  │    "category": "ML",                                │ │ │
│ │  │    "price": { "$lt": 100 }                          │ │ │
│ │  │  }                                                  │ │ │
│ │  └────────────────────────────────────────────────────┘ │ │
│ │                                                          │ │
│ │  All conditions are combined with AND logic.            │ │
│ └──────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────┘
```

## UI Components

### Filter Condition Row
Each condition has 4 components in a row:

```
[AND] [Field Input] [Operator Select] [Value Input] [Delete Button]
 (1)       (2)            (3)              (4)           (5)
```

1. **AND Label**: Shows for 2nd+ conditions
2. **Field Input**: Text input for metadata field name
3. **Operator Select**: Dropdown with options:
   - Equals (=)
   - Not Equals (≠)
   - Greater Than (>)
   - Less Than (<)
   - Greater or Equal (≥)
   - Less or Equal (≤)
   - Contains
   - Exists
4. **Value Input**:
   - Text/number input for most operators
   - True/False dropdown for "Exists" operator
5. **Delete Button**: Trash icon to remove condition

### Header Controls
```
┌──────────────────────────────────────────────────────┐
│ 🔍 Filter Builder    [Show JSON] [+ Add Condition]  │
└──────────────────────────────────────────────────────┘
```

- **Title**: "Filter Builder" with filter icon
- **Show/Hide JSON Button**: Toggle JSON preview
- **Add Condition Button**: Add new filter condition

### JSON Preview (Collapsible)
```
┌────────────────────────────────────────┐
│ Generated Filter JSON:                 │
│ ┌────────────────────────────────────┐ │
│ │ {                                  │ │
│ │   "category": "ML",                │ │
│ │   "price": { "$lt": 100 }          │ │
│ │ }                                  │ │
│ └────────────────────────────────────┘ │
└────────────────────────────────────────┘
```

Read-only textarea with syntax-highlighted JSON

### Empty State
```
┌──────────────────────────────────────────────────────┐
│ 🔍 Filter Builder    [Show JSON] [+ Add Condition]  │
├──────────────────────────────────────────────────────┤
│                                                      │
│     No filter conditions.                           │
│     Click "Add Condition" to get started.           │
│                                                      │
└──────────────────────────────────────────────────────┘
```

## Interaction Flow

### Step 1: Enable Filter
User toggles "Use metadata filter" switch

### Step 2: Add Condition
1. Click "+ Add Condition" button
2. New row appears with empty fields

### Step 3: Configure Condition
1. Type field name (e.g., "category")
2. Select operator (e.g., "Equals (=)")
3. Enter value (e.g., "ML")

### Step 4: View JSON (Optional)
1. Click "Show JSON" button
2. See generated filter in JSON format
3. Verify the filter is correct

### Step 5: Add More Conditions (Optional)
1. Click "+ Add Condition" again
2. Configure second condition
3. Both conditions combine with AND

### Step 6: Perform Search
1. Enter search query
2. Click search button
3. Filter is automatically applied

### Step 7: Remove Condition (Optional)
1. Click trash icon next to any condition
2. Condition is removed
3. Filter JSON updates automatically

## Example Usage Scenarios

### Scenario 1: Simple Category Filter
```
Goal: Find all ML-related vectors

Steps:
1. Add condition: category = ML
2. Click search

Result Filter:
{
  "category": "ML"
}
```

### Scenario 2: Price Range Filter
```
Goal: Find products between $50 and $200

Steps:
1. Add condition: price > 50
2. Add condition: price < 200
3. Click search

Result Filter:
{
  "price": {
    "$gt": 50,
    "$lt": 200
  }
}
```

### Scenario 3: Complex Multi-Field Filter
```
Goal: Find ML documents with "sample" tag and recent scores

Steps:
1. Add condition: category = ML
2. Add condition: tags Contains sample
3. Add condition: score >= 0.8
4. Click search

Result Filter:
{
  "category": "ML",
  "tags": { "$contains": "sample" },
  "score": { "$gte": 0.8 }
}
```

### Scenario 4: Existence Check
```
Goal: Find vectors that have metadata field

Steps:
1. Add condition: description Exists true
2. Click search

Result Filter:
{
  "description": { "$exists": true }
}
```

## Color Scheme (Dark Theme)

- **Background**: Dark gray (bg-gray-800/50)
- **Borders**: Medium gray (border-gray-700)
- **Text**: White
- **Inputs**: Dark background (bg-gray-800/50)
- **Primary Button**: Blue accent (color="primary")
- **Danger Button**: Red (color="danger")
- **JSON Text**: Green (text-green-400) for syntax highlighting

## Responsive Behavior

- Condition rows stack vertically
- Each row remains horizontal with flex layout
- Inputs resize proportionally:
  - Field: flex-1 (flexible width)
  - Operator: w-48 (fixed 192px)
  - Value: flex-1 (flexible width)
  - Delete: min-w-8 (32px square)

## Accessibility

- All inputs have proper labels
- Keyboard navigation supported
- Select dropdowns are keyboard-accessible
- Delete buttons have aria-labels
- Color contrast meets WCAG AA standards

---

This visual guide helps you understand what the Filter Builder will look like and how users will interact with it!
