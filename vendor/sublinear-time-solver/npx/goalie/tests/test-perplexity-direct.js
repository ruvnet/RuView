import dotenv from 'dotenv';
dotenv.config();

async function testPerplexity() {
  const apiKey = process.env.PERPLEXITY_API_KEY;

  if (!apiKey) {
    console.error('❌ PERPLEXITY_API_KEY not found');
    process.exit(1);
  }

  console.log('🔑 API Key found:', apiKey.substring(0, 20) + '...');

  try {
    // Test search API
    console.log('\n📡 Testing Perplexity Search API...');
    const searchResponse = await fetch('https://api.perplexity.ai/search', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${apiKey}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        q: 'quantum computing cryptography',
        search_domain_filter: ['edu', 'gov'],
        return_citations: true,
        search_recency_filter: 'month'
      })
    });

    const searchData = await searchResponse.json();
    console.log('Search Status:', searchResponse.status);
    console.log('Search Response:', JSON.stringify(searchData, null, 2).substring(0, 500));

    // Test chat API
    console.log('\n💬 Testing Perplexity Chat API...');
    const chatResponse = await fetch('https://api.perplexity.ai/chat/completions', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${apiKey}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        model: 'sonar',
        messages: [
          { role: 'user', content: 'What is quantum computing?' }
        ]
      })
    });

    const chatData = await chatResponse.json();
    console.log('Chat Status:', chatResponse.status);
    console.log('Chat Response:', JSON.stringify(chatData, null, 2).substring(0, 500));

  } catch (error) {
    console.error('❌ Error:', error.message);
  }
}

testPerplexity();