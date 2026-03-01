# 🎥 Agentic-Synth Video Tutorial Script

**Duration**: 8-10 minutes
**Target Audience**: Developers, ML engineers, data scientists
**Format**: Screen recording with voice-over

---

## Video Structure

1. **Introduction** (1 min)
2. **Installation & Setup** (1 min)
3. **Basic Usage** (2 mins)
4. **Advanced Features** (2 mins)
5. **Real-World Example** (2 mins)
6. **Performance & Wrap-up** (1 min)

---

## Script

### Scene 1: Introduction (0:00 - 1:00)

**Visual**: Title card, then switch to terminal

**Voice-over**:
> "Hi! Today I'll show you agentic-synth - a high-performance synthetic data generator that makes it incredibly easy to create realistic test data for your AI and ML projects.
>
> Whether you're training machine learning models, building RAG systems, or just need to seed your development database, agentic-synth has you covered with AI-powered data generation.
>
> Let's dive in!"

**Screen**: Show README on GitHub with badges

---

### Scene 2: Installation (1:00 - 2:00)

**Visual**: Terminal with command prompts

**Voice-over**:
> "Installation is straightforward. You can use it as a global CLI tool or add it to your project."

**Type in terminal**:
```bash
# Global installation
npm install -g @ruvector/agentic-synth

# Or use directly with npx
npx agentic-synth --help
```

**Voice-over**:
> "You'll need an API key from Google Gemini or OpenRouter. Let's set that up quickly."

**Type**:
```bash
export GEMINI_API_KEY="your-key-here"
```

**Voice-over**:
> "And we're ready to go!"

---

### Scene 3: Basic Usage - CLI (2:00 - 3:00)

**Visual**: Terminal showing CLI commands

**Voice-over**:
> "Let's start with the CLI. Generating data is as simple as running a single command."

**Type**:
```bash
npx agentic-synth generate \
  --type structured \
  --count 10 \
  --schema '{"name": "string", "email": "email", "age": "number"}' \
  --output users.json
```

**Voice-over**:
> "In just a few seconds, we have 10 realistic user records with names, emails, and ages. Let's look at the output."

**Type**:
```bash
cat users.json | jq '.[0:3]'
```

**Visual**: Show JSON output with realistic data

**Voice-over**:
> "Notice how the data looks realistic - real names, valid email formats, appropriate ages. This is all powered by AI."

---

### Scene 4: SDK Usage (3:00 - 4:00)

**Visual**: VS Code with TypeScript file

**Voice-over**:
> "For more control, you can use the SDK directly in your code. Let me show you how simple that is."

**Type in editor** (`demo.ts`):
```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

// Initialize with configuration
const synth = new AgenticSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  cacheStrategy: 'memory', // Enable caching for 95%+ speedup
  cacheTTL: 3600
});

// Generate structured data
const users = await synth.generateStructured({
  count: 100,
  schema: {
    user_id: 'UUID',
    name: 'full name',
    email: 'valid email',
    age: 'number (18-80)',
    country: 'country name',
    subscription: 'free | pro | enterprise'
  }
});

console.log(`Generated ${users.data.length} users`);
console.log('Sample:', users.data[0]);
```

**Voice-over**:
> "Run this code..."

**Type in terminal**:
```bash
npx tsx demo.ts
```

**Visual**: Show output with generated data

**Voice-over**:
> "And we instantly get 100 realistic user profiles. Notice the caching - if we run this again with the same options, it's nearly instant!"

---

### Scene 5: Advanced Features - Time Series (4:00 - 5:00)

**Visual**: Split screen - editor on left, output on right

**Voice-over**:
> "agentic-synth isn't just for simple records. It can generate complex time-series data, perfect for financial or IoT applications."

**Type in editor**:
```typescript
const stockData = await synth.generateTimeSeries({
  count: 365,
  startDate: '2024-01-01',
  interval: '1d',
  schema: {
    date: 'ISO date',
    open: 'number (100-200)',
    high: 'number (105-210)',
    low: 'number (95-195)',
    close: 'number (100-200)',
    volume: 'number (1000000-10000000)'
  },
  constraints: [
    'high must be >= open and close',
    'low must be <= open and close',
    'close influences next day open'
  ]
});

console.log('Generated stock data for 1 year');
```

**Voice-over**:
> "The constraints ensure our data follows real-world patterns - high prices are actually higher than opens and closes, and there's continuity between days."

**Show output**: Chart visualization of stock data

---

### Scene 6: Advanced Features - Streaming (5:00 - 6:00)

**Visual**: Editor showing streaming code

**Voice-over**:
> "Need to generate millions of records? Use streaming to avoid memory issues."

**Type**:
```typescript
let count = 0;
for await (const record of synth.generateStream('structured', {
  count: 1_000_000,
  schema: {
    id: 'UUID',
    timestamp: 'ISO timestamp',
    value: 'number'
  }
})) {
  // Process each record individually
  await saveToDatabase(record);

  count++;
  if (count % 10000 === 0) {
    console.log(`Processed ${count.toLocaleString()}...`);
  }
}
```

**Voice-over**:
> "This streams records one at a time, so you can process a million records without loading everything into memory."

**Visual**: Show progress counter incrementing

---

### Scene 7: Real-World Example - ML Training Data (6:00 - 7:30)

**Visual**: Complete working example

**Voice-over**:
> "Let me show you a real-world use case: generating training data for a machine learning model that predicts customer churn."

**Type**:
```typescript
// Generate training dataset with features
const trainingData = await synth.generateStructured({
  count: 5000,
  schema: {
    customer_age: 'number (18-80)',
    annual_income: 'number (20000-200000)',
    credit_score: 'number (300-850)',
    account_tenure_months: 'number (1-360)',
    num_products: 'number (1-5)',
    balance: 'number (0-250000)',
    num_transactions_12m: 'number (0-200)',

    // Target variable
    churn: 'boolean (higher likelihood if credit_score < 600, balance < 1000)'
  },
  constraints: [
    'Churn rate should be ~15-20%',
    'Higher income correlates with higher balance',
    'Customers with 1 product more likely to churn'
  ]
});

// Split into train/test
const trainSize = Math.floor(trainingData.data.length * 0.8);
const trainSet = trainingData.data.slice(0, trainSize);
const testSet = trainingData.data.slice(trainSize);

console.log(`Training set: ${trainSet.length} samples`);
console.log(`Test set: ${testSet.length} samples`);
console.log(`Churn rate: ${(trainSet.filter(d => d.churn).length / trainSet.length * 100).toFixed(1)}%`);
```

**Voice-over**:
> "In minutes, we have a complete ML dataset with realistic distributions and correlations. The AI understands the constraints and generates data that actually makes sense for training models."

---

### Scene 8: Performance Highlights (7:30 - 8:30)

**Visual**: Show benchmark results

**Voice-over**:
> "Let's talk performance. agentic-synth is incredibly fast, thanks to intelligent caching."

**Visual**: Show PERFORMANCE_REPORT.md metrics

**Voice-over**:
> "All operations complete in sub-millisecond to low-millisecond latencies. Cache hits are essentially instant. And with an 85% cache hit rate in production, you're looking at 95%+ performance improvement for repeated queries.
>
> The package also handles 1000+ requests per second with linear scaling, making it perfect for production workloads."

---

### Scene 9: Wrap-up (8:30 - 9:00)

**Visual**: Return to terminal, show final commands

**Voice-over**:
> "That's agentic-synth! To recap:
> - Simple CLI and SDK interfaces
> - AI-powered realistic data generation
> - Time-series, events, and structured data support
> - Streaming for large datasets
> - Built-in caching for incredible performance
> - Perfect for ML training, RAG systems, and testing
>
> Check out the documentation for more advanced examples, and give it a try in your next project!"

**Type**:
```bash
npm install @ruvector/agentic-synth
```

**Visual**: Show GitHub repo with Star button

**Voice-over**:
> "If you found this useful, star the repo on GitHub and let me know what you build with it. Thanks for watching!"

**Visual**: End card with links

---

## Visual Assets Needed

1. **Title Cards**:
   - Intro card with logo
   - Feature highlights card
   - End card with links

2. **Code Examples**:
   - Syntax highlighted in VS Code
   - Font: Fira Code or JetBrains Mono
   - Theme: Dark+ or Material Theme

3. **Terminal**:
   - Oh My Zsh with clean prompt
   - Colors: Nord or Dracula theme

4. **Data Visualizations**:
   - JSON output formatted with jq
   - Stock chart for time-series example
   - Progress bars for streaming

5. **Documentation**:
   - README.md rendered
   - Performance metrics table
   - Benchmark results

---

## Recording Tips

1. **Screen Setup**:
   - 1920x1080 resolution
   - Clean desktop, no distractions
   - Close unnecessary applications
   - Disable notifications

2. **Terminal Settings**:
   - Large font size (16-18pt)
   - High contrast theme
   - Slow down typing with tool like "Keycastr"

3. **Editor Settings**:
   - Zoom to 150-200%
   - Hide sidebars for cleaner view
   - Use presentation mode

4. **Audio**:
   - Use quality microphone
   - Record in quiet room
   - Speak clearly and at moderate pace
   - Add background music (subtle, low volume)

5. **Pacing**:
   - Pause between steps
   - Let output display for 2-3 seconds
   - Don't rush through commands
   - Leave time for viewers to read

---

## Post-Production Checklist

- [ ] Add title cards
- [ ] Add transitions between scenes
- [ ] Highlight important commands/output
- [ ] Add annotations/callouts where helpful
- [ ] Background music at 10-15% volume
- [ ] Export at 1080p, 60fps
- [ ] Generate subtitles/captions
- [ ] Create thumbnail image
- [ ] Upload to YouTube
- [ ] Add to README as embedded video

---

## Video Description (for YouTube)

```markdown
# Agentic-Synth: High-Performance Synthetic Data Generator

Generate realistic synthetic data for AI/ML training, RAG systems, and database seeding in minutes!

🔗 Links:
- NPM: https://www.npmjs.com/package/@ruvector/agentic-synth
- GitHub: https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth
- Documentation: https://github.com/ruvnet/ruvector/blob/main/packages/agentic-synth/README.md

⚡ Performance:
- Sub-millisecond P99 latencies
- 85% cache hit rate
- 1000+ req/s throughput
- 95%+ speedup with caching

🎯 Use Cases:
- Machine learning training data
- RAG system data generation
- Database seeding
- API testing
- Load testing

📚 Chapters:
0:00 Introduction
1:00 Installation & Setup
2:00 CLI Usage
3:00 SDK Usage
4:00 Time-Series Data
5:00 Streaming Large Datasets
6:00 ML Training Example
7:30 Performance Highlights
8:30 Wrap-up

#machinelearning #AI #syntheticdata #typescript #nodejs #datascience #RAG
```

---

## Alternative: Live Coding Demo (15 min)

For a longer, more in-depth tutorial:

1. **Setup** (3 min): Project initialization, dependencies
2. **Basic Generation** (3 min): Simple examples
3. **Complex Schemas** (3 min): Nested structures, constraints
4. **Integration** (3 min): Database seeding example
5. **Performance** (2 min): Benchmarks and optimization
6. **Q&A** (1 min): Common questions

---

**Script Version**: 1.0
**Last Updated**: 2025-11-22
**Status**: Ready for Recording 🎬
