# Plaid Local Learning System

> **Privacy-preserving financial intelligence that runs 100% in the browser**

## Overview

The Plaid Local Learning System enables sophisticated financial analysis and machine learning while keeping all data on the user's device. No financial information, learned patterns, or AI models ever leave the browser.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        USER'S BROWSER (All Data Stays Here)                 │
│                                                                             │
│  ┌─────────────────┐     ┌──────────────────┐     ┌───────────────────┐    │
│  │   Plaid Link    │────▶│   Transaction    │────▶│  Local Learning   │    │
│  │   (OAuth)       │     │   Processor      │     │  Engine (WASM)    │    │
│  └─────────────────┘     └──────────────────┘     └───────────────────┘    │
│           │                      │                        │                 │
│           ▼                      ▼                        ▼                 │
│  ┌─────────────────┐     ┌──────────────────┐     ┌───────────────────┐    │
│  │   IndexedDB     │     │   IndexedDB      │     │   IndexedDB       │    │
│  │   (Tokens)      │     │   (Embeddings)   │     │   (Q-Values)      │    │
│  └─────────────────┘     └──────────────────┘     └───────────────────┘    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     RuVector WASM Engine                             │   │
│  │                                                                      │   │
│  │  • HNSW Vector Index ─────── 150x faster similarity search          │   │
│  │  • Spiking Neural Network ── Temporal pattern learning (STDP)       │   │
│  │  • Q-Learning ────────────── Spending optimization                  │   │
│  │  • LSH (Locality-Sensitive)─ Semantic categorization                │   │
│  │  • Anomaly Detection ─────── Statistical outlier detection          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ HTTPS (only OAuth + API calls)
                                    ▼
                         ┌─────────────────────┐
                         │    Plaid Servers    │
                         │  (Auth & Raw Data)  │
                         └─────────────────────┘
```

## Privacy Guarantees

| Guarantee | Description |
|-----------|-------------|
| 🔒 **No Data Exfiltration** | Financial transactions never leave the browser |
| 🧠 **Local-Only Learning** | All ML models train and run in WebAssembly |
| 🔐 **Encrypted Storage** | Optional AES-256-GCM encryption for IndexedDB |
| 📊 **No Analytics** | Zero tracking, telemetry, or data collection |
| 🌐 **Offline-Capable** | Works without network after initial Plaid sync |
| 🗑️ **User Control** | Instant, complete data deletion on request |

## Features

### 1. Smart Transaction Categorization
ML-based categorization using semantic embeddings and HNSW similarity search.

```typescript
const prediction = learner.predictCategory(transaction);
// { category: "Food and Drink", confidence: 0.92, similar_transactions: [...] }
```

### 2. Anomaly Detection
Identify unusual transactions compared to learned spending patterns.

```typescript
const anomaly = learner.detectAnomaly(transaction);
// { is_anomaly: true, anomaly_score: 2.3, reason: "Amount $500 is 5x typical", expected_amount: 100 }
```

### 3. Budget Recommendations
Q-learning based budget optimization that improves over time.

```typescript
const recommendation = learner.getBudgetRecommendation("Food", currentSpending, budget);
// { category: "Food", recommended_limit: 450, current_avg: 380, trend: "stable", confidence: 0.85 }
```

### 4. Temporal Pattern Analysis
Understand weekly and monthly spending habits.

```typescript
const heatmap = learner.getTemporalHeatmap();
// { day_of_week: [100, 50, 60, 80, 120, 200, 180], day_of_month: [...] }
```

### 5. Similar Transaction Search
Find transactions similar to a given one using vector similarity.

```typescript
const similar = learner.findSimilar(transaction, 5);
// [{ id: "tx_123", distance: 0.05 }, { id: "tx_456", distance: 0.12 }, ...]
```

## Quick Start

### Installation

```bash
npm install @ruvector/edge
```

### Basic Usage

```typescript
import { PlaidLocalLearner } from '@ruvector/edge';

// Initialize (loads WASM, opens IndexedDB)
const learner = new PlaidLocalLearner();
await learner.init();

// Optional: Use encryption password
await learner.init('your-secure-password');

// Process transactions from Plaid
const insights = await learner.processTransactions(transactions);
console.log(`Processed ${insights.transactions_processed} transactions`);
console.log(`Learned ${insights.patterns_learned} patterns`);

// Get analysis
const category = learner.predictCategory(newTransaction);
const anomaly = learner.detectAnomaly(newTransaction);
const budget = learner.getBudgetRecommendation("Groceries", 320, 400);

// Record user feedback for Q-learning
learner.recordOutcome("Groceries", "under_budget", 1.0);

// Save state (persists to IndexedDB)
await learner.save();

// Export for backup
const backup = await learner.exportData();

// Clear all data (privacy feature)
await learner.clearAllData();
```

### With Plaid Link

```typescript
import { PlaidLocalLearner, PlaidLinkHandler } from '@ruvector/edge';

// Initialize Plaid Link handler
const plaidHandler = new PlaidLinkHandler({
  environment: 'sandbox',
  products: ['transactions'],
  countryCodes: ['US'],
  language: 'en',
});
await plaidHandler.init();

// After successful Plaid Link flow, store token locally
await plaidHandler.storeToken(itemId, accessToken);

// Later: retrieve token for API calls
const token = await plaidHandler.getToken(itemId);
```

## Machine Learning Components

### HNSW Vector Index
- **Purpose**: Fast similarity search for transaction categorization
- **Performance**: 150x faster than brute-force search
- **Memory**: Sub-linear space complexity

### Q-Learning
- **Purpose**: Optimize budget recommendations over time
- **Algorithm**: Temporal difference learning with ε-greedy exploration
- **Learning Rate**: 0.1 (configurable)
- **States**: Category + spending ratio
- **Actions**: under_budget, at_budget, over_budget

### Spiking Neural Network
- **Purpose**: Temporal pattern recognition (weekday vs weekend spending)
- **Architecture**: 21 input → 32 hidden → 8 output neurons
- **Learning**: Spike-Timing Dependent Plasticity (STDP)

### Feature Extraction
Each transaction is converted to a 21-dimensional feature vector:
- Amount (log-normalized)
- Day of week (0-6)
- Day of month (1-31)
- Hour of day (0-23)
- Weekend indicator
- Category LSH hash (8 dims)
- Merchant LSH hash (8 dims)

## Data Storage

### IndexedDB Schema

| Store | Key | Value | Purpose |
|-------|-----|-------|---------|
| `learning_state` | `main` | Encrypted JSON | Q-values, patterns, embeddings |
| `plaid_tokens` | Item ID | Access token | Plaid API authentication |
| `transactions` | Transaction ID | Transaction | Raw transaction storage |
| `insights` | Date | Insights | Daily aggregated insights |

### Storage Limits
- IndexedDB quota: ~50MB - 1GB (browser dependent)
- Typical usage: ~1KB per 100 transactions
- Learning state: ~10KB for 1000 patterns

## Security Considerations

### Encryption
```typescript
// Initialize with encryption
await learner.init('user-password');

// Password is never stored
// PBKDF2 key derivation (100,000 iterations)
// AES-256-GCM encryption for all stored data
```

### Token Storage
```typescript
// Plaid tokens are stored in IndexedDB
// Never sent to any third party
// Automatically cleared with clearAllData()
```

### Cross-Origin Isolation
The WASM module runs in the browser's sandbox with no network access.
Only the JavaScript wrapper can make network requests (to Plaid).

## API Reference

### PlaidLocalLearner

| Method | Description |
|--------|-------------|
| `init(password?)` | Initialize WASM and IndexedDB |
| `processTransactions(tx[])` | Process and learn from transactions |
| `predictCategory(tx)` | Predict category for transaction |
| `detectAnomaly(tx)` | Check if transaction is anomalous |
| `getBudgetRecommendation(cat, spent, budget)` | Get budget advice |
| `recordOutcome(cat, action, reward)` | Record for Q-learning |
| `getPatterns()` | Get all learned patterns |
| `getTemporalHeatmap()` | Get spending heatmap |
| `findSimilar(tx, k)` | Find similar transactions |
| `getStats()` | Get learning statistics |
| `save()` | Persist state to IndexedDB |
| `load()` | Load state from IndexedDB |
| `exportData()` | Export encrypted backup |
| `importData(data)` | Import from backup |
| `clearAllData()` | Delete all local data |

### Types

```typescript
interface Transaction {
  transaction_id: string;
  account_id: string;
  amount: number;
  date: string;           // YYYY-MM-DD
  name: string;
  merchant_name?: string;
  category: string[];
  pending: boolean;
  payment_channel: string;
}

interface SpendingPattern {
  pattern_id: string;
  category: string;
  avg_amount: number;
  frequency_days: number;
  confidence: number;     // 0-1
  last_seen: number;      // timestamp
}

interface CategoryPrediction {
  category: string;
  confidence: number;
  similar_transactions: string[];
}

interface AnomalyResult {
  is_anomaly: boolean;
  anomaly_score: number;  // 0 = normal, >1 = anomalous
  reason: string;
  expected_amount: number;
}

interface BudgetRecommendation {
  category: string;
  recommended_limit: number;
  current_avg: number;
  trend: 'increasing' | 'stable' | 'decreasing';
  confidence: number;
}

interface LearningStats {
  version: number;
  patterns_count: number;
  q_values_count: number;
  embeddings_count: number;
  index_size: number;
}
```

## Performance

| Metric | Value | Notes |
|--------|-------|-------|
| WASM Load | ~50ms | First load, cached after |
| Process 100 tx | ~10ms | Vector indexing + learning |
| Category Prediction | <1ms | HNSW search |
| Anomaly Detection | <1ms | Pattern lookup |
| IndexedDB Save | ~5ms | Async, non-blocking |
| Memory Usage | ~2-5MB | Depends on index size |

## Browser Compatibility

| Browser | Status | Notes |
|---------|--------|-------|
| Chrome 80+ | ✅ Full Support | Best performance |
| Firefox 75+ | ✅ Full Support | Good performance |
| Safari 14+ | ✅ Full Support | WebAssembly SIMD may be limited |
| Edge 80+ | ✅ Full Support | Chromium-based |
| Mobile Safari | ✅ Supported | IndexedDB quota may be limited |
| Mobile Chrome | ✅ Supported | Full feature support |

## Examples

### Complete Integration Example

See `pkg/plaid-demo.html` for a complete working example with:
- WASM initialization
- Transaction processing
- Pattern visualization
- Heatmap display
- Sample data loading
- Data export/import

### Running the Demo

```bash
# Build WASM
./scripts/build-wasm.sh

# Serve the demo
npx serve pkg

# Open http://localhost:3000/plaid-demo.html
```

## Troubleshooting

### WASM Won't Load
- Ensure CORS headers allow `application/wasm`
- Check browser console for specific error
- Verify WASM file is accessible

### IndexedDB Errors
- Check browser's storage quota
- Ensure site isn't in private/incognito mode
- Try clearing site data and reinitializing

### Learning Not Improving
- Ensure `recordOutcome()` is called with correct rewards
- Check that transactions have varied categories
- Verify state is being saved (`save()` after changes)

## License

MIT License - See LICENSE file for details.
