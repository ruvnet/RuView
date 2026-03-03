#!/usr/bin/env node

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Load environment variables manually
const envPath = join(__dirname, '.env');
const envContent = readFileSync(envPath, 'utf-8');
const envVars = {};

envContent.split('\n').forEach(line => {
    if (line && !line.startsWith('#')) {
        const [key, value] = line.split('=');
        if (key && value) {
            envVars[key.trim()] = value.trim();
        }
    }
});

async function testPerplexitySearchAPI() {
    const apiKey = envVars.PERPLEXITY_API_KEY;

    if (!apiKey) {
        console.error('❌ PERPLEXITY_API_KEY not found in .env file');
        process.exit(1);
    }

    console.log('🔑 API Key found:', apiKey.substring(0, 10) + '...' + apiKey.substring(apiKey.length - 4));
    console.log('\n📡 Testing Perplexity Search API...\n');

    // Test 1: Basic search with Sonar model
    console.log('1️⃣ Testing basic search with Sonar model...');
    try {
        const response = await fetch('https://api.perplexity.ai/chat/completions', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${apiKey}`,
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                model: 'sonar',
                messages: [
                    {
                        role: 'user',
                        content: 'What are the latest developments in AI reasoning models in 2024?'
                    }
                ],
                search_domain_filter: ["openai.com", "anthropic.com", "deepmind.com"],
                search_recency_filter: "month",
                return_citations: true
            })
        });

        const data = await response.json();

        if (response.ok) {
            console.log('✅ Basic search successful!');
            console.log('   Response length:', data.choices[0].message.content.length, 'chars');
            console.log('   Citations:', data.citations?.length || 0, 'sources\n');
        } else {
            console.error('❌ Basic search failed:', data.error);
        }
    } catch (error) {
        console.error('❌ Error:', error.message);
    }

    // Test 2: Multi-turn conversation
    console.log('2️⃣ Testing multi-turn conversation...');
    try {
        const response = await fetch('https://api.perplexity.ai/chat/completions', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${apiKey}`,
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                model: 'sonar',
                messages: [
                    {
                        role: 'user',
                        content: 'What is GOAP planning?'
                    },
                    {
                        role: 'assistant',
                        content: 'GOAP (Goal-Oriented Action Planning) is an AI planning technique used primarily in game development...'
                    },
                    {
                        role: 'user',
                        content: 'How does it compare to behavior trees?'
                    }
                ]
            })
        });

        const data = await response.json();

        if (response.ok) {
            console.log('✅ Multi-turn conversation successful!');
            console.log('   Response preview:', data.choices[0].message.content.substring(0, 100) + '...\n');
        } else {
            console.error('❌ Multi-turn failed:', data.error);
        }
    } catch (error) {
        console.error('❌ Error:', error.message);
    }

    // Test 3: Academic search mode (if available)
    console.log('3️⃣ Testing with different parameters...');
    try {
        const response = await fetch('https://api.perplexity.ai/chat/completions', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${apiKey}`,
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                model: 'sonar',
                messages: [
                    {
                        role: 'system',
                        content: 'You are a helpful research assistant. Be concise.'
                    },
                    {
                        role: 'user',
                        content: 'Explain transformer architecture in one paragraph'
                    }
                ],
                temperature: 0.1,
                max_tokens: 300
            })
        });

        const data = await response.json();

        if (response.ok) {
            console.log('✅ Custom parameters test successful!');
            console.log('   Model used:', data.model);
            console.log('   Token usage:', JSON.stringify(data.usage || {}), '\n');
        } else {
            console.error('❌ Custom params failed:', data.error);
        }
    } catch (error) {
        console.error('❌ Error:', error.message);
    }

    console.log('🎉 All tests completed!');
}

// Run the tests
testPerplexitySearchAPI();