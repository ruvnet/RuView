#!/bin/bash
# Script to apply bulk import feature to App.tsx
# This script makes all the necessary changes for the bulk vector import feature

set -e

APP_FILE="src/App.tsx"
BACKUP_FILE="src/App.tsx.backup.$(date +%s)"

echo "Creating backup at $BACKUP_FILE..."
cp "$APP_FILE" "$BACKUP_FILE"

echo "Applying changes to $APP_FILE..."

# 1. Add FileSpreadsheet import
echo "1. Adding FileSpreadsheet icon import..."
sed -i '/XCircle,/a\  FileSpreadsheet,' "$APP_FILE"

# 2. Add bulk import disclosure hook (after line with isScenariosOpen)
echo "2. Adding modal disclosure hook..."
sed -i '/const { isOpen: isScenariosOpen.*useDisclosure/a\  const { isOpen: isBulkImportOpen, onOpen: onBulkImportOpen, onClose: onBulkImportClose } = useDisclosure();' "$APP_FILE"

# 3. Add state variables (after importJson state)
echo "3. Adding state variables..."
sed -i "/const \[importJson, setImportJson\] = useState('');/a\\
\\
  // Bulk import states\\
  const [bulkImportData, setBulkImportData] = useState('');\\
  const [bulkImportFormat, setBulkImportFormat] = useState<'csv' | 'json'>('json');\\
  const [bulkImportPreview, setBulkImportPreview] = useState<Array<{id: string, embedding: number[], metadata?: Record<string, unknown>}>>([]);\\
  const [bulkImportProgress, setBulkImportProgress] = useState({ current: 0, total: 0, errors: 0 });\\
  const [isBulkImporting, setIsBulkImporting] = useState(false);" "$APP_FILE"

echo "✅ Basic changes applied!"
echo ""
echo "⚠️  Manual steps required:"
echo ""
echo "1. Add the utility functions (CSV parser, JSON parser, handlers) after the state declarations"
echo "2. Add the Bulk Import button to Quick Actions section"
echo "3. Add the Bulk Import Modal component"
echo ""
echo "Please refer to BULK_IMPORT_IMPLEMENTATION.md for the complete code to add."
echo ""
echo "Backup saved at: $BACKUP_FILE"
