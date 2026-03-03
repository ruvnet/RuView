#!/usr/bin/env node

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Load environment variables
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

const API_KEY = envVars.PERPLEXITY_API_KEY;

async function testDirectAPI() {
    console.log('🎯 Testing Direct Perplexity API for Goalie MCP\n');

    const query = "What are the advantages of GOAP planning over behavior trees?";

    console.log('📝 Query:', query);
    console.log('🔑 API Key:', API_KEY.substring(0, 10) + '...\n');

    try {
        const startTime = Date.now();

        const response = await fetch('https://api.perplexity.ai/chat/completions', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${API_KEY}`,
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                model: 'sonar',
                messages: [
                    {
                        role: 'system',
                        content: 'You are a helpful AI assistant specializing in game AI and planning algorithms.'
                    },
                    {
                        role: 'user',
                        content: query
                    }
                ],
                temperature: 0.1,
                return_citations: true,
                search_domain_filter: ["gamasutra.com", "gamedevs.org", "aigamedev.com"],
                max_tokens: 500
            })
        });

        const data = await response.json();
        const endTime = Date.now();

        if (response.ok) {
            console.log('✅ API Response Success!\n');
            console.log('📊 Performance Metrics:');
            console.log('   Response Time:', endTime - startTime, 'ms');
            console.log('   Citations:', data.citations?.length || 0);
            console.log('   Token Usage:', JSON.stringify(data.usage || {}));
            console.log('\n📝 Answer:');
            console.log(data.choices[0].message.content);

            if (data.citations && data.citations.length > 0) {
                console.log('\n📚 Sources:');
                data.citations.slice(0, 3).forEach((citation, i) => {
                    console.log(`   ${i + 1}. ${citation}`);
                });
            }
        } else {
            console.error('❌ API Error:', data.error);
        }
    } catch (error) {
        console.error('❌ Request Failed:', error.message);
    }
}

testDirectAPI();