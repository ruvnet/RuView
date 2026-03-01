"use strict";
/**
 * @fileoverview Comprehensive examples for the embeddings integration module
 *
 * This file demonstrates all features of the ruvector-extensions embeddings module:
 * - Multiple embedding providers (OpenAI, Cohere, Anthropic, HuggingFace)
 * - Batch processing
 * - Error handling and retry logic
 * - Integration with VectorDB
 * - Search functionality
 *
 * @author ruv.io Team <info@ruv.io>
 * @license MIT
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.example1_OpenAIBasic = example1_OpenAIBasic;
exports.example2_OpenAICustomDimensions = example2_OpenAICustomDimensions;
exports.example3_CohereSearchTypes = example3_CohereSearchTypes;
exports.example4_AnthropicVoyage = example4_AnthropicVoyage;
exports.example5_HuggingFaceLocal = example5_HuggingFaceLocal;
exports.example6_BatchProcessing = example6_BatchProcessing;
exports.example7_ErrorHandling = example7_ErrorHandling;
exports.example8_VectorDBInsert = example8_VectorDBInsert;
exports.example9_VectorDBSearch = example9_VectorDBSearch;
exports.example10_CompareProviders = example10_CompareProviders;
exports.example11_ProgressiveLoading = example11_ProgressiveLoading;
const embeddings_js_1 = require("../embeddings.js");
// ============================================================================
// Example 1: OpenAI Embeddings - Basic Usage
// ============================================================================
async function example1_OpenAIBasic() {
    console.log('\n=== Example 1: OpenAI Embeddings - Basic Usage ===\n');
    // Initialize OpenAI embeddings provider
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
        model: 'text-embedding-3-small', // 1536 dimensions
    });
    // Embed a single text
    const singleEmbedding = await openai.embedText('Hello, world!');
    console.log('Single embedding dimension:', singleEmbedding.length);
    console.log('First 5 values:', singleEmbedding.slice(0, 5));
    // Embed multiple texts
    const texts = [
        'Machine learning is fascinating',
        'Deep learning uses neural networks',
        'Natural language processing is important',
    ];
    const result = await openai.embedTexts(texts);
    console.log('\nBatch embeddings:');
    console.log('Total embeddings:', result.embeddings.length);
    console.log('Total tokens used:', result.totalTokens);
    console.log('Provider:', result.metadata?.provider);
}
// ============================================================================
// Example 2: OpenAI with Custom Dimensions
// ============================================================================
async function example2_OpenAICustomDimensions() {
    console.log('\n=== Example 2: OpenAI with Custom Dimensions ===\n');
    // Use text-embedding-3-large with custom dimensions
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
        model: 'text-embedding-3-large',
        dimensions: 1024, // Reduce from default 3072 to 1024
    });
    const embedding = await openai.embedText('Custom dimension embedding');
    console.log('Embedding dimension:', embedding.length);
    console.log('Expected:', openai.getDimension());
}
// ============================================================================
// Example 3: Cohere Embeddings with Search Types
// ============================================================================
async function example3_CohereSearchTypes() {
    console.log('\n=== Example 3: Cohere Embeddings with Search Types ===\n');
    const cohere = new embeddings_js_1.CohereEmbeddings({
        apiKey: process.env.COHERE_API_KEY || 'your-key',
        model: 'embed-english-v3.0',
    });
    // Embed documents (for storage)
    const documentEmbedder = new embeddings_js_1.CohereEmbeddings({
        apiKey: process.env.COHERE_API_KEY || 'your-key',
        model: 'embed-english-v3.0',
        inputType: 'search_document',
    });
    const documents = [
        'The Eiffel Tower is in Paris',
        'The Statue of Liberty is in New York',
        'The Great Wall is in China',
    ];
    const docResult = await documentEmbedder.embedTexts(documents);
    console.log('Document embeddings created:', docResult.embeddings.length);
    // Embed query (for searching)
    const queryEmbedder = new embeddings_js_1.CohereEmbeddings({
        apiKey: process.env.COHERE_API_KEY || 'your-key',
        model: 'embed-english-v3.0',
        inputType: 'search_query',
    });
    const queryEmbedding = await queryEmbedder.embedText('famous landmarks in France');
    console.log('Query embedding dimension:', queryEmbedding.length);
}
// ============================================================================
// Example 4: Anthropic/Voyage Embeddings
// ============================================================================
async function example4_AnthropicVoyage() {
    console.log('\n=== Example 4: Anthropic/Voyage Embeddings ===\n');
    const anthropic = new embeddings_js_1.AnthropicEmbeddings({
        apiKey: process.env.VOYAGE_API_KEY || 'your-voyage-key',
        model: 'voyage-2',
        inputType: 'document',
    });
    const texts = [
        'Anthropic develops Claude AI',
        'Voyage AI provides embedding models',
    ];
    const result = await anthropic.embedTexts(texts);
    console.log('Embeddings created:', result.embeddings.length);
    console.log('Dimension:', anthropic.getDimension());
}
// ============================================================================
// Example 5: HuggingFace Local Embeddings
// ============================================================================
async function example5_HuggingFaceLocal() {
    console.log('\n=== Example 5: HuggingFace Local Embeddings ===\n');
    // Run embeddings locally - no API key needed!
    const hf = new embeddings_js_1.HuggingFaceEmbeddings({
        model: 'Xenova/all-MiniLM-L6-v2',
        normalize: true,
        batchSize: 32,
    });
    const texts = [
        'Local embeddings are fast',
        'No API calls required',
        'Privacy-friendly solution',
    ];
    console.log('Processing locally...');
    const result = await hf.embedTexts(texts);
    console.log('Local embeddings created:', result.embeddings.length);
    console.log('Dimension:', hf.getDimension());
}
// ============================================================================
// Example 6: Batch Processing Large Datasets
// ============================================================================
async function example6_BatchProcessing() {
    console.log('\n=== Example 6: Batch Processing Large Datasets ===\n');
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
    });
    // Generate 1000 sample texts
    const largeDataset = Array.from({ length: 1000 }, (_, i) => `Document ${i}: Sample text for embedding`);
    console.log('Processing 1000 texts...');
    const startTime = Date.now();
    const result = await openai.embedTexts(largeDataset);
    const duration = Date.now() - startTime;
    console.log(`Processed ${result.embeddings.length} texts in ${duration}ms`);
    console.log(`Average: ${(duration / result.embeddings.length).toFixed(2)}ms per text`);
    console.log(`Total tokens: ${result.totalTokens}`);
}
// ============================================================================
// Example 7: Error Handling and Retry Logic
// ============================================================================
async function example7_ErrorHandling() {
    console.log('\n=== Example 7: Error Handling and Retry Logic ===\n');
    // Configure custom retry logic
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
        retryConfig: {
            maxRetries: 5,
            initialDelay: 2000,
            maxDelay: 30000,
            backoffMultiplier: 2,
        },
    });
    try {
        // This will retry on rate limits or temporary errors
        const result = await openai.embedTexts(['Test text']);
        console.log('Success! Embeddings created:', result.embeddings.length);
    }
    catch (error) {
        console.error('Failed after retries:', error.message);
        console.error('Retryable:', error.retryable);
    }
}
// ============================================================================
// Example 8: Integration with VectorDB - Insert
// ============================================================================
async function example8_VectorDBInsert() {
    console.log('\n=== Example 8: Integration with VectorDB - Insert ===\n');
    // Note: This example assumes VectorDB is available
    // You'll need to import and initialize VectorDB first
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
    });
    // Sample documents to embed and insert
    const documents = [
        {
            id: 'doc1',
            text: 'Machine learning enables computers to learn from data',
            metadata: { category: 'AI', author: 'John Doe' },
        },
        {
            id: 'doc2',
            text: 'Deep learning uses neural networks with multiple layers',
            metadata: { category: 'AI', author: 'Jane Smith' },
        },
        {
            id: 'doc3',
            text: 'Natural language processing helps computers understand text',
            metadata: { category: 'NLP', author: 'John Doe' },
        },
    ];
    // Example usage (uncomment when VectorDB is available):
    /*
    const { VectorDB } = await import('ruvector');
    const db = new VectorDB({ dimension: openai.getDimension() });
  
    const insertedIds = await embedAndInsert(db, openai, documents, {
      overwrite: true,
      onProgress: (current, total) => {
        console.log(`Progress: ${current}/${total} documents inserted`);
      },
    });
  
    console.log('Inserted document IDs:', insertedIds);
    */
    console.log('Documents prepared:', documents.length);
    console.log('Ready for insertion when VectorDB is initialized');
}
// ============================================================================
// Example 9: Integration with VectorDB - Search
// ============================================================================
async function example9_VectorDBSearch() {
    console.log('\n=== Example 9: Integration with VectorDB - Search ===\n');
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
    });
    // Example usage (uncomment when VectorDB is available):
    /*
    const { VectorDB } = await import('ruvector');
    const db = new VectorDB({ dimension: openai.getDimension() });
  
    // First, insert some documents (see example 8)
    // ...
  
    // Now search for similar documents
    const results = await embedAndSearch(
      db,
      openai,
      'What is deep learning?',
      {
        topK: 5,
        threshold: 0.7,
        filter: { category: 'AI' },
      }
    );
  
    console.log('Search results:');
    results.forEach((result, i) => {
      console.log(`${i + 1}. ${result.id} (similarity: ${result.score})`);
      console.log(`   Text: ${result.metadata?.text}`);
    });
    */
    console.log('Search functionality ready when VectorDB is initialized');
}
// ============================================================================
// Example 10: Comparing Multiple Providers
// ============================================================================
async function example10_CompareProviders() {
    console.log('\n=== Example 10: Comparing Multiple Providers ===\n');
    const text = 'Artificial intelligence is transforming the world';
    // OpenAI
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
    });
    // Cohere
    const cohere = new embeddings_js_1.CohereEmbeddings({
        apiKey: process.env.COHERE_API_KEY || 'your-key',
    });
    // HuggingFace (local)
    const hf = new embeddings_js_1.HuggingFaceEmbeddings();
    // Compare dimensions
    console.log('Provider dimensions:');
    console.log('- OpenAI:', openai.getDimension());
    console.log('- Cohere:', cohere.getDimension());
    console.log('- HuggingFace:', hf.getDimension());
    // Compare batch sizes
    console.log('\nMax batch sizes:');
    console.log('- OpenAI:', openai.getMaxBatchSize());
    console.log('- Cohere:', cohere.getMaxBatchSize());
    console.log('- HuggingFace:', hf.getMaxBatchSize());
    // Generate embeddings (uncomment to actually run):
    /*
    console.log('\nGenerating embeddings...');
  
    const [openaiResult, cohereResult, hfResult] = await Promise.all([
      openai.embedText(text),
      cohere.embedText(text),
      hf.embedText(text),
    ]);
  
    console.log('All embeddings generated successfully!');
    */
}
// ============================================================================
// Example 11: Progressive Loading with Progress Tracking
// ============================================================================
async function example11_ProgressiveLoading() {
    console.log('\n=== Example 11: Progressive Loading with Progress ===\n');
    const openai = new embeddings_js_1.OpenAIEmbeddings({
        apiKey: process.env.OPENAI_API_KEY || 'sk-...',
    });
    const documents = Array.from({ length: 50 }, (_, i) => ({
        id: `doc${i}`,
        text: `Document ${i}: This is sample content for embedding`,
        metadata: { index: i, batch: Math.floor(i / 10) },
    }));
    // Track progress
    let processed = 0;
    const progressBar = (current, total) => {
        const percentage = Math.round((current / total) * 100);
        const bar = '█'.repeat(percentage / 2) + '░'.repeat(50 - percentage / 2);
        console.log(`[${bar}] ${percentage}% (${current}/${total})`);
    };
    // Example usage (uncomment when VectorDB is available):
    /*
    const { VectorDB } = await import('ruvector');
    const db = new VectorDB({ dimension: openai.getDimension() });
  
    await embedAndInsert(db, openai, documents, {
      onProgress: progressBar,
    });
    */
    console.log('Ready to process', documents.length, 'documents with progress tracking');
}
// ============================================================================
// Main Function - Run All Examples
// ============================================================================
async function runAllExamples() {
    console.log('╔════════════════════════════════════════════════════════════╗');
    console.log('║  RUVector Extensions - Embeddings Integration Examples     ║');
    console.log('╚════════════════════════════════════════════════════════════╝');
    // Note: Uncomment the examples you want to run
    // Make sure you have the required API keys set in environment variables
    try {
        // await example1_OpenAIBasic();
        // await example2_OpenAICustomDimensions();
        // await example3_CohereSearchTypes();
        // await example4_AnthropicVoyage();
        // await example5_HuggingFaceLocal();
        // await example6_BatchProcessing();
        // await example7_ErrorHandling();
        // await example8_VectorDBInsert();
        // await example9_VectorDBSearch();
        // await example10_CompareProviders();
        // await example11_ProgressiveLoading();
        console.log('\n✓ All examples completed successfully!');
    }
    catch (error) {
        console.error('\n✗ Error running examples:', error);
    }
}
// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
    runAllExamples();
}
//# sourceMappingURL=embeddings-example.js.map