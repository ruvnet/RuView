# Troubleshooting Guide

Common issues and solutions for Agentic-Synth.

## Table of Contents

- [Installation Issues](#installation-issues)
- [Generation Problems](#generation-problems)
- [Performance Issues](#performance-issues)
- [Quality Problems](#quality-problems)
- [Integration Issues](#integration-issues)
- [API and Authentication](#api-and-authentication)
- [Memory and Resource Issues](#memory-and-resource-issues)

---

## Installation Issues

### npm install fails

**Symptoms:**
```bash
npm ERR! code ENOENT
npm ERR! syscall open
npm ERR! path /path/to/package.json
```

**Solutions:**
1. Ensure you're in the correct directory
2. Verify Node.js version (>=18.0.0):
   ```bash
   node --version
   ```
3. Clear npm cache:
   ```bash
   npm cache clean --force
   npm install
   ```
4. Try with different package manager:
   ```bash
   pnpm install
   # or
   yarn install
   ```

### TypeScript type errors

**Symptoms:**
```
Cannot find module 'agentic-synth' or its corresponding type declarations
```

**Solutions:**
1. Ensure TypeScript version >=5.0:
   ```bash
   npm install -D typescript@latest
   ```
2. Check tsconfig.json:
   ```json
   {
     "compilerOptions": {
       "moduleResolution": "node",
       "esModuleInterop": true
     }
   }
   ```

### Native dependencies fail to build

**Symptoms:**
```
gyp ERR! build error
```

**Solutions:**
1. Install build tools:
   - **Windows**: `npm install --global windows-build-tools`
   - **Mac**: `xcode-select --install`
   - **Linux**: `sudo apt-get install build-essential`
2. Use pre-built binaries if available

---

## Generation Problems

### Generation returns empty results

**Symptoms:**
```typescript
const data = await synth.generate({ schema, count: 1000 });
console.log(data.data.length); // 0
```

**Solutions:**

1. **Check API key configuration:**
   ```typescript
   const synth = new SynthEngine({
     provider: 'openai',
     apiKey: process.env.OPENAI_API_KEY, // Ensure this is set
   });
   ```

2. **Verify schema validity:**
   ```typescript
   import { validateSchema } from 'agentic-synth/utils';

   const isValid = validateSchema(schema);
   if (!isValid.valid) {
     console.error('Schema errors:', isValid.errors);
   }
   ```

3. **Check for errors in generation:**
   ```typescript
   try {
     const data = await synth.generate({ schema, count: 1000 });
   } catch (error) {
     console.error('Generation failed:', error);
   }
   ```

### Generation hangs indefinitely

**Symptoms:**
- Generation never completes
- No progress updates
- No error messages

**Solutions:**

1. **Add timeout:**
   ```typescript
   const controller = new AbortController();
   const timeout = setTimeout(() => controller.abort(), 60000); // 1 minute

   try {
     await synth.generate({
       schema,
       count: 1000,
       abortSignal: controller.signal,
     });
   } finally {
     clearTimeout(timeout);
   }
   ```

2. **Enable verbose logging:**
   ```typescript
   const synth = new SynthEngine({
     provider: 'openai',
     debug: true, // Enable debug logs
   });
   ```

3. **Reduce batch size:**
   ```typescript
   const synth = new SynthEngine({
     batchSize: 10, // Start small
   });
   ```

### Invalid data generated

**Symptoms:**
- Data doesn't match schema
- Missing required fields
- Type mismatches

**Solutions:**

1. **Enable strict validation:**
   ```typescript
   const synth = new SynthEngine({
     validationEnabled: true,
     strictMode: true,
   });
   ```

2. **Add constraints to schema:**
   ```typescript
   const schema = Schema.define({
     name: 'User',
     type: 'object',
     properties: {
       email: {
         type: 'string',
         format: 'email',
         pattern: '^[a-z0-9._%+-]+@[a-z0-9.-]+\\.[a-z]{2,}$',
       },
     },
     required: ['email'],
   });
   ```

3. **Increase temperature for diversity:**
   ```typescript
   const synth = new SynthEngine({
     temperature: 0.8, // Higher for more variation
   });
   ```

---

## Performance Issues

### Slow generation speed

**Symptoms:**
- Generation takes much longer than expected
- Low throughput (< 100 items/minute)

**Solutions:**

1. **Enable streaming mode:**
   ```typescript
   for await (const item of synth.generateStream({ schema, count: 10000 })) {
     // Process item immediately
   }
   ```

2. **Increase batch size:**
   ```typescript
   const synth = new SynthEngine({
     batchSize: 1000, // Larger batches
     maxWorkers: 8,   // More parallel workers
   });
   ```

3. **Use faster model:**
   ```typescript
   const synth = new SynthEngine({
     provider: 'openai',
     model: 'gpt-3.5-turbo', // Faster than gpt-4
   });
   ```

4. **Cache embeddings:**
   ```typescript
   const synth = new SynthEngine({
     cacheEnabled: true,
     cacheTTL: 3600, // 1 hour
   });
   ```

5. **Profile generation:**
   ```typescript
   import { profiler } from 'agentic-synth/utils';

   const profile = await profiler.profile(() => {
     return synth.generate({ schema, count: 1000 });
   });

   console.log('Bottlenecks:', profile.bottlenecks);
   ```

### High memory usage

**Symptoms:**
```
FATAL ERROR: Reached heap limit Allocation failed
```

**Solutions:**

1. **Use streaming:**
   ```typescript
   // Instead of loading all in memory
   const data = await synth.generate({ schema, count: 1000000 }); // ❌

   // Stream and process incrementally
   for await (const item of synth.generateStream({ schema, count: 1000000 })) { // ✅
     await processItem(item);
   }
   ```

2. **Reduce batch size:**
   ```typescript
   const synth = new SynthEngine({
     batchSize: 100, // Smaller batches
   });
   ```

3. **Increase Node.js heap size:**
   ```bash
   NODE_OPTIONS="--max-old-space-size=4096" npm start
   ```

4. **Process in chunks:**
   ```typescript
   const chunkSize = 10000;
   const totalCount = 1000000;

   for (let i = 0; i < totalCount; i += chunkSize) {
     const chunk = await synth.generate({
       schema,
       count: Math.min(chunkSize, totalCount - i),
     });
     await exportChunk(chunk, i);
   }
   ```

---

## Quality Problems

### Low realism scores

**Symptoms:**
```typescript
const metrics = await QualityMetrics.evaluate(data);
console.log(metrics.realism); // 0.45 (too low)
```

**Solutions:**

1. **Improve schema descriptions:**
   ```typescript
   const schema = Schema.define({
     name: 'User',
     description: 'A realistic user profile with authentic details',
     properties: {
       name: {
         type: 'string',
         description: 'Full name following cultural naming conventions',
       },
     },
   });
   ```

2. **Add examples to schema:**
   ```typescript
   const schema = Schema.define({
     properties: {
       bio: {
         type: 'string',
         examples: [
           'Passionate about machine learning and open source',
           'Software engineer with 10 years of experience',
         ],
       },
     },
   });
   ```

3. **Adjust temperature:**
   ```typescript
   const synth = new SynthEngine({
     temperature: 0.9, // Higher for more natural variation
   });
   ```

4. **Use better model:**
   ```typescript
   const synth = new SynthEngine({
     provider: 'anthropic',
     model: 'claude-3-opus-20240229', // Higher quality
   });
   ```

### Low diversity scores

**Symptoms:**
- Many duplicate or nearly identical examples
- Limited variation in generated data

**Solutions:**

1. **Increase temperature:**
   ```typescript
   const synth = new SynthEngine({
     temperature: 0.95, // Maximum diversity
   });
   ```

2. **Add diversity constraints:**
   ```typescript
   const schema = Schema.define({
     constraints: [
       {
         type: 'diversity',
         field: 'content',
         minSimilarity: 0.3, // Max 30% similarity
       },
     ],
   });
   ```

3. **Use varied prompts:**
   ```typescript
   const synth = new SynthEngine({
     promptVariation: true,
     variationStrategies: ['paraphrase', 'reframe', 'alternative-angle'],
   });
   ```

### Biased data detected

**Symptoms:**
```typescript
const metrics = await QualityMetrics.evaluate(data, { bias: true });
console.log(metrics.bias); // { gender: 0.85 } (too high)
```

**Solutions:**

1. **Add fairness constraints:**
   ```typescript
   const schema = Schema.define({
     constraints: [
       {
         type: 'fairness',
         attributes: ['gender', 'age', 'ethnicity'],
         distribution: 'uniform',
       },
     ],
   });
   ```

2. **Explicit diversity instructions:**
   ```typescript
   const schema = Schema.define({
     description: 'Generate diverse examples representing all demographics equally',
   });
   ```

3. **Post-generation filtering:**
   ```typescript
   import { BiasDetector } from 'agentic-synth/utils';

   const detector = new BiasDetector();
   const balanced = data.filter(item => {
     const bias = detector.detect(item);
     return bias.overall < 0.3; // Keep low-bias items
   });
   ```

---

## Integration Issues

### Ruvector connection fails

**Symptoms:**
```
Error: Cannot connect to Ruvector at localhost:8080
```

**Solutions:**

1. **Verify Ruvector is running:**
   ```bash
   # Check if Ruvector service is running
   curl http://localhost:8080/health
   ```

2. **Check connection configuration:**
   ```typescript
   const db = new VectorDB({
     host: 'localhost',
     port: 8080,
     timeout: 5000,
   });
   ```

3. **Use retry logic:**
   ```typescript
   import { retry } from 'agentic-synth/utils';

   const db = await retry(() => new VectorDB(), {
     attempts: 3,
     delay: 1000,
   });
   ```

### Vector insertion fails

**Symptoms:**
```
Error: Failed to insert vectors into collection
```

**Solutions:**

1. **Verify collection exists:**
   ```typescript
   const collections = await db.listCollections();
   if (!collections.includes('my-collection')) {
     await db.createCollection('my-collection', { dimensions: 384 });
   }
   ```

2. **Check vector dimensions match:**
   ```typescript
   const schema = Schema.define({
     properties: {
       embedding: {
         type: 'embedding',
         dimensions: 384, // Must match collection config
       },
     },
   });
   ```

3. **Use batching:**
   ```typescript
   await synth.generateAndInsert({
     schema,
     count: 10000,
     collection: 'vectors',
     batchSize: 1000, // Insert in batches
   });
   ```

---

## API and Authentication

### OpenAI API errors

**Symptoms:**
```
Error: Incorrect API key provided
```

**Solutions:**

1. **Verify API key:**
   ```bash
   echo $OPENAI_API_KEY
   ```

2. **Set environment variable:**
   ```bash
   export OPENAI_API_KEY="sk-..."
   ```

3. **Pass key explicitly:**
   ```typescript
   const synth = new SynthEngine({
     provider: 'openai',
     apiKey: 'sk-...', // Not recommended for production
   });
   ```

### Rate limit exceeded

**Symptoms:**
```
Error: Rate limit exceeded. Please try again later.
```

**Solutions:**

1. **Implement exponential backoff:**
   ```typescript
   const synth = new SynthEngine({
     retryConfig: {
       maxRetries: 5,
       backoffMultiplier: 2,
       initialDelay: 1000,
     },
   });
   ```

2. **Reduce request rate:**
   ```typescript
   const synth = new SynthEngine({
     rateLimit: {
       requestsPerMinute: 60,
       tokensPerMinute: 90000,
     },
   });
   ```

3. **Use multiple API keys:**
   ```typescript
   const synth = new SynthEngine({
     provider: 'openai',
     apiKeys: [
       process.env.OPENAI_API_KEY_1,
       process.env.OPENAI_API_KEY_2,
       process.env.OPENAI_API_KEY_3,
     ],
     keyRotationStrategy: 'round-robin',
   });
   ```

---

## Memory and Resource Issues

### Out of memory errors

**Solutions:**

1. **Use streaming mode (recommended):**
   ```typescript
   for await (const item of synth.generateStream({ schema, count: 1000000 })) {
     await processAndDiscard(item);
   }
   ```

2. **Process in smaller batches:**
   ```typescript
   async function generateInChunks(totalCount: number, chunkSize: number) {
     for (let i = 0; i < totalCount; i += chunkSize) {
       const chunk = await synth.generate({
         schema,
         count: chunkSize,
       });
       await processChunk(chunk);
       // Chunk is garbage collected after processing
     }
   }
   ```

3. **Increase Node.js memory:**
   ```bash
   node --max-old-space-size=8192 script.js
   ```

### Disk space issues

**Symptoms:**
```
Error: ENOSPC: no space left on device
```

**Solutions:**

1. **Stream directly to storage:**
   ```typescript
   import { createWriteStream } from 'fs';

   const stream = createWriteStream('./output.jsonl');
   for await (const item of synth.generateStream({ schema, count: 1000000 })) {
     stream.write(JSON.stringify(item) + '\n');
   }
   stream.end();
   ```

2. **Use compression:**
   ```typescript
   import { createGzip } from 'zlib';
   import { pipeline } from 'stream/promises';

   await pipeline(
     synth.generateStream({ schema, count: 1000000 }),
     createGzip(),
     createWriteStream('./output.jsonl.gz')
   );
   ```

3. **Export to remote storage:**
   ```typescript
   import { S3Client } from '@aws-sdk/client-s3';

   const s3 = new S3Client({ region: 'us-east-1' });
   await synth.generate({ schema, count: 1000000 }).export({
     format: 'parquet',
     destination: 's3://my-bucket/synthetic-data.parquet',
   });
   ```

---

## Debugging Tips

### Enable debug logging

```typescript
import { setLogLevel } from 'agentic-synth';

setLogLevel('debug');

const synth = new SynthEngine({
  debug: true,
  verbose: true,
});
```

### Use profiler

```typescript
import { profiler } from 'agentic-synth/utils';

const results = await profiler.profile(async () => {
  return await synth.generate({ schema, count: 1000 });
});

console.log('Performance breakdown:', results.breakdown);
console.log('Bottlenecks:', results.bottlenecks);
```

### Test with small datasets first

```typescript
// Test with 10 examples first
const test = await synth.generate({ schema, count: 10 });
console.log('Sample:', test.data[0]);

// Validate quality
const quality = await QualityMetrics.evaluate(test.data);
console.log('Quality:', quality);

// If quality is good, scale up
if (quality.overall > 0.85) {
  const full = await synth.generate({ schema, count: 100000 });
}
```

---

## Getting Help

If you're still experiencing issues:

1. **Check documentation**: https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth/docs
2. **Search issues**: https://github.com/ruvnet/ruvector/issues
3. **Ask on Discord**: https://discord.gg/ruvnet
4. **Open an issue**: https://github.com/ruvnet/ruvector/issues/new

When reporting issues, include:
- Agentic-Synth version: `npm list agentic-synth`
- Node.js version: `node --version`
- Operating system
- Minimal reproduction code
- Error messages and stack traces
- Schema definition (if relevant)

---

## FAQ

**Q: Why is generation slow?**
A: Enable streaming, increase batch size, use faster models, or cache embeddings.

**Q: How do I improve data quality?**
A: Use better models, add detailed schema descriptions, include examples, adjust temperature.

**Q: Can I use multiple LLM providers?**
A: Yes, configure fallback providers or rotate between them.

**Q: How do I handle rate limits?**
A: Implement exponential backoff, reduce rate, or use multiple API keys.

**Q: Is there a size limit for generation?**
A: No hard limit, but use streaming for datasets > 10,000 items.

---

## Additional Resources

- [API Reference](./API.md)
- [Examples](./EXAMPLES.md)
- [Integration Guides](./INTEGRATIONS.md)
- [Best Practices](./BEST_PRACTICES.md)
