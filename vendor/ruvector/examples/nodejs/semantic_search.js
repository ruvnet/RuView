/**
 * Semantic Search Example (Node.js)
 *
 * Demonstrates building a semantic search system with Ruvector
 */

const { VectorDB } = require('ruvector');

// Mock embedding function (in production, use a real embedding model)
function mockEmbedding(text, dims = 384) {
    // Simple hash-based mock embedding
    let hash = 0;
    for (let i = 0; i < text.length; i++) {
        hash = ((hash << 5) - hash) + text.charCodeAt(i);
        hash = hash & hash;
    }

    const embedding = new Float32Array(dims);
    for (let i = 0; i < dims; i++) {
        embedding[i] = Math.sin((hash + i) * 0.01);
    }
    return embedding;
}

async function main() {
    console.log('🔍 Semantic Search Example\n');

    // 1. Setup database
    console.log('1. Setting up search index...');
    const db = new VectorDB({
        dimensions: 384,
        storagePath: './semantic_search.db',
        distanceMetric: 'cosine',
        hnsw: {
            m: 32,
            efConstruction: 200,
            efSearch: 100
        }
    });
    console.log('   ✓ Database created\n');

    // 2. Index documents
    console.log('2. Indexing documents...');
    const documents = [
        {
            id: 'doc_001',
            text: 'The quick brown fox jumps over the lazy dog',
            category: 'animals'
        },
        {
            id: 'doc_002',
            text: 'Machine learning is a subset of artificial intelligence',
            category: 'technology'
        },
        {
            id: 'doc_003',
            text: 'Python is a popular programming language for data science',
            category: 'technology'
        },
        {
            id: 'doc_004',
            text: 'The cat sat on the mat while birds sang outside',
            category: 'animals'
        },
        {
            id: 'doc_005',
            text: 'Neural networks are inspired by biological neurons',
            category: 'technology'
        },
        {
            id: 'doc_006',
            text: 'Dogs are loyal companions and great pets',
            category: 'animals'
        },
        {
            id: 'doc_007',
            text: 'Deep learning requires large amounts of training data',
            category: 'technology'
        },
        {
            id: 'doc_008',
            text: 'Birds migrate south during winter months',
            category: 'animals'
        }
    ];

    const entries = documents.map(doc => ({
        id: doc.id,
        vector: mockEmbedding(doc.text),
        metadata: {
            text: doc.text,
            category: doc.category
        }
    }));

    await db.insertBatch(entries);
    console.log(`   ✓ Indexed ${documents.length} documents\n`);

    // 3. Perform semantic searches
    const queries = [
        'artificial intelligence and neural networks',
        'pets and domestic animals',
        'programming and software development'
    ];

    for (const query of queries) {
        console.log(`Query: "${query}"`);
        console.log('─'.repeat(60));

        const queryEmbedding = mockEmbedding(query);
        const results = await db.search({
            vector: queryEmbedding,
            k: 3,
            includeMetadata: true
        });

        results.forEach((result, i) => {
            console.log(`${i + 1}. ${result.metadata.text}`);
            console.log(`   Category: ${result.metadata.category}, Similarity: ${(1 - result.distance).toFixed(4)}`);
        });
        console.log();
    }

    // 4. Filtered semantic search
    console.log('Filtered search (category: technology)');
    console.log('─'.repeat(60));

    const techQuery = mockEmbedding('computers and algorithms');
    const filteredResults = await db.search({
        vector: techQuery,
        k: 3,
        filter: { category: 'technology' },
        includeMetadata: true
    });

    filteredResults.forEach((result, i) => {
        console.log(`${i + 1}. ${result.metadata.text}`);
        console.log(`   Similarity: ${(1 - result.distance).toFixed(4)}`);
    });
    console.log();

    console.log('✅ Semantic search example completed!');
    console.log('\n💡 In production:');
    console.log('   • Use a real embedding model (OpenAI, Sentence Transformers, etc.)');
    console.log('   • Add more documents to your knowledge base');
    console.log('   • Implement filters for category, date, author, etc.');
    console.log('   • Add hybrid search (vector + keyword) for better results');
}

main().catch(console.error);
