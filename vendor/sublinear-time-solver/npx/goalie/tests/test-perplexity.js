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

async function testPerplexityAPI() {
    const apiKey = envVars.PERPLEXITY_API_KEY;

    if (!apiKey) {
        console.error('❌ PERPLEXITY_API_KEY not found in .env file');
        process.exit(1);
    }

    console.log('🔑 API Key found:', apiKey.substring(0, 10) + '...' + apiKey.substring(apiKey.length - 4));
    console.log('\n📡 Testing Perplexity API...\n');

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
                        content: 'What is 2+2?'
                    }
                ]
            })
        });

        const responseText = await response.text();

        if (response.ok) {
            try {
                const data = JSON.parse(responseText);
                console.log('✅ API Key is valid!\n');
                console.log('📝 Response:');
                console.log('   Model:', data.model);
                console.log('   Message:', data.choices[0].message.content);
                if (data.citations && data.citations.length > 0) {
                    console.log('   Citations:', data.citations.length, 'sources');
                }
                console.log('\n🎉 Perplexity API test successful!');
                return true;
            } catch (e) {
                console.error('❌ Failed to parse response as JSON');
                console.error('   Response:', responseText.substring(0, 200));
                return false;
            }
        } else {
            console.error('❌ API request failed:');
            console.error('   Status:', response.status);
            console.error('   Status Text:', response.statusText);
            console.error('   Response:', responseText.substring(0, 200));
            return false;
        }
    } catch (error) {
        console.error('❌ Failed to connect to Perplexity API:');
        console.error('   Error:', error.message);
        return false;
    }
}

// Run the test
testPerplexityAPI().then(success => {
    process.exit(success ? 0 : 1);
});