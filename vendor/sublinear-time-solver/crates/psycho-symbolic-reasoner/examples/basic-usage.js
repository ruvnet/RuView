#!/usr/bin/env node

/**
 * Basic Usage Example - Psycho-Symbolic Reasoner
 *
 * This example demonstrates the core functionality of the reasoner:
 * - Sentiment analysis
 * - Preference extraction
 * - Graph reasoning
 * - Basic planning
 */

import { PsychoSymbolicReasoner } from '../dist/index.js';

async function main() {
  console.log('🧠 Psycho-Symbolic Reasoner - Basic Usage Example\n');

  try {
    // Initialize the reasoner
    console.log('📋 Initializing reasoner...');
    const reasoner = new PsychoSymbolicReasoner({
      enableGraphReasoning: true,
      enableAffectExtraction: true,
      enablePlanning: true,
      logLevel: 'info'
    });

    await reasoner.initialize();
    console.log('✅ Reasoner initialized successfully\n');

    // Example 1: Sentiment Analysis
    console.log('😊 Example 1: Sentiment Analysis');
    console.log('─'.repeat(40));

    const sentimentTexts = [
      "I'm feeling overwhelmed with work deadlines",
      "This new project is exciting and challenging!",
      "I prefer quiet environments for deep thinking",
      "The team meeting was frustrating but productive"
    ];

    for (const text of sentimentTexts) {
      const sentiment = await reasoner.extractSentiment(text);
      console.log(`Text: "${text}"`);
      console.log(`Sentiment: ${sentiment.score.toFixed(2)} (${sentiment.primaryEmotion})`);
      console.log(`Confidence: ${(sentiment.confidence * 100).toFixed(1)}%`);
      console.log('');
    }

    // Example 2: Preference Extraction
    console.log('🎯 Example 2: Preference Extraction');
    console.log('─'.repeat(40));

    const preferenceTexts = [
      "I like working early in the morning when it's quiet",
      "I prefer collaborative projects over solo work",
      "I find meditation helpful for stress relief",
      "I dislike long meetings without clear agendas"
    ];

    for (const text of preferenceTexts) {
      const preferences = await reasoner.extractPreferences(text);
      console.log(`Text: "${text}"`);
      console.log(`Found ${preferences.preferences.length} preferences:`);

      preferences.preferences.forEach((pref, index) => {
        console.log(`  ${index + 1}. ${pref.type}: "${pref.subject}" → "${pref.object}" (strength: ${pref.strength.toFixed(2)})`);
      });
      console.log('');
    }

    // Example 3: Simple Graph Query
    console.log('🕸️ Example 3: Graph Reasoning');
    console.log('─'.repeat(40));

    // Load a basic knowledge base
    const basicKnowledgeBase = {
      nodes: [
        { id: 'stress', type: 'emotion', properties: { valence: -0.7, arousal: 0.8 } },
        { id: 'meditation', type: 'activity', properties: { duration: 15, energy: 'low' } },
        { id: 'exercise', type: 'activity', properties: { duration: 30, energy: 'high' } },
        { id: 'deep_breathing', type: 'technique', properties: { duration: 5, difficulty: 'easy' } }
      ],
      edges: [
        { from: 'meditation', to: 'stress', relationship: 'helps', weight: 0.8 },
        { from: 'exercise', to: 'stress', relationship: 'helps', weight: 0.7 },
        { from: 'deep_breathing', to: 'stress', relationship: 'helps', weight: 0.9 }
      ]
    };

    await reasoner.loadKnowledgeBase(basicKnowledgeBase);

    const graphQuery = {
      pattern: "find activities that help with stress",
      maxResults: 5,
      includeInference: true
    };

    const graphResult = await reasoner.queryGraph(graphQuery);
    console.log(`Query: "${graphQuery.pattern}"`);
    console.log(`Found ${graphResult.results.length} results:`);

    graphResult.results.forEach((result, index) => {
      console.log(`  ${index + 1}. ${result.activity} (confidence: ${result.confidence.toFixed(2)})`);
    });
    console.log('');

    // Example 4: Basic Planning
    console.log('📋 Example 4: Goal-Oriented Planning');
    console.log('─'.repeat(40));

    const planRequest = {
      goal: {
        description: "reduce stress and improve focus",
        type: "achievement",
        priority: 0.8,
        successCriteria: [
          { metric: "stress_level", target: "< 0.3" },
          { metric: "focus_duration", target: "> 30 minutes" }
        ]
      },
      currentState: {
        facts: [
          { predicate: "hasEmotion", object: "stress", value: 0.7 },
          { predicate: "timeAvailable", object: "30 minutes" },
          { predicate: "energyLevel", object: "medium" }
        ],
        context: {
          environment: "office",
          timeOfDay: "afternoon",
          urgency: "medium"
        }
      },
      preferences: [
        { type: 'like', subject: 'user', object: 'quick_techniques', strength: 0.8 },
        { type: 'dislike', subject: 'user', object: 'strenuous_activity', strength: 0.6 }
      ],
      constraints: [
        { type: 'time', limit: 30, unit: 'minutes' },
        { type: 'energy', max: 'medium' }
      ]
    };

    const plan = await reasoner.createPlan(planRequest);
    console.log(`Goal: ${planRequest.goal.description}`);
    console.log(`Generated plan with ${plan.plan.length} steps:`);
    console.log(`Confidence: ${(plan.confidence * 100).toFixed(1)}%`);
    console.log(`Estimated duration: ${plan.estimatedDuration} minutes\n`);

    plan.plan.forEach((action, index) => {
      console.log(`  Step ${index + 1}: ${action.name}`);
      console.log(`    Description: ${action.description}`);
      console.log(`    Duration: ${action.duration} minutes`);
      console.log(`    Priority: ${action.priority.toFixed(2)}`);
      console.log('');
    });

    // Example 5: Integrated Analysis
    console.log('🔄 Example 5: Integrated Psycho-Symbolic Analysis');
    console.log('─'.repeat(50));

    const userInput = "I'm feeling anxious about the presentation tomorrow. I usually calm down with some music and quiet time.";

    console.log(`User input: "${userInput}"\n`);

    // Multi-step analysis
    const sentiment = await reasoner.extractSentiment(userInput);
    const preferences = await reasoner.extractPreferences(userInput);

    console.log('🔍 Analysis Results:');
    console.log(`  Emotional state: ${sentiment.primaryEmotion} (${sentiment.score.toFixed(2)})`);
    console.log(`  Stress indicators: ${sentiment.emotions.filter(e => e.emotion.includes('anx')).length > 0 ? 'Present' : 'None'}`);
    console.log(`  Preferences detected: ${preferences.preferences.length}`);

    preferences.preferences.forEach(pref => {
      console.log(`    - ${pref.type}s: ${pref.object}`);
    });

    // Create personalized plan
    const personalizedPlan = await reasoner.createPlan({
      goal: {
        description: "reduce anxiety and prepare for presentation",
        type: "achievement",
        priority: 0.9
      },
      currentState: {
        facts: [
          { predicate: "hasEmotion", object: "anxiety", value: Math.abs(sentiment.score) },
          { predicate: "hasEvent", object: "presentation", time: "tomorrow" }
        ],
        context: { primaryConcern: "performance" }
      },
      preferences: preferences.preferences
    });

    console.log('\n📋 Personalized Recommendations:');
    personalizedPlan.plan.forEach((action, index) => {
      console.log(`  ${index + 1}. ${action.name} (${action.duration}min)`);
    });

    console.log(`\n💡 Plan rationale: ${personalizedPlan.explanation}`);

    // Display statistics
    console.log('\n📊 Session Statistics');
    console.log('─'.repeat(30));
    const stats = reasoner.getStats();
    console.log(`Queries processed: ${stats.queriesProcessed}`);
    console.log(`Average response time: ${stats.averageResponseTime}ms`);
    console.log(`Memory usage: ${stats.memoryUsage}MB`);

    await reasoner.dispose();
    console.log('\n✅ Example completed successfully!');

  } catch (error) {
    console.error('❌ Error:', error.message);
    console.error('Stack:', error.stack);
    process.exit(1);
  }
}

// Helper function to simulate async operations during development
async function simulateReasoner() {
  return {
    initialize: async () => {},
    extractSentiment: async (text) => ({
      score: Math.random() * 2 - 1,
      confidence: 0.7 + Math.random() * 0.3,
      primaryEmotion: ['joy', 'sadness', 'anger', 'fear', 'surprise'][Math.floor(Math.random() * 5)],
      emotions: [
        { emotion: 'neutral', score: 0.1, confidence: 0.8 }
      ]
    }),
    extractPreferences: async (text) => ({
      preferences: [
        {
          type: Math.random() > 0.5 ? 'like' : 'dislike',
          subject: 'user',
          object: text.includes('quiet') ? 'quiet_environment' : 'activity',
          strength: 0.6 + Math.random() * 0.4,
          confidence: 0.7
        }
      ],
      confidence: 0.8,
      categories: ['environment', 'activity']
    }),
    loadKnowledgeBase: async () => {},
    queryGraph: async () => ({
      results: [
        { activity: 'meditation', confidence: 0.9 },
        { activity: 'deep breathing', confidence: 0.8 },
        { activity: 'light exercise', confidence: 0.7 }
      ],
      executionTime: 25
    }),
    createPlan: async (request) => ({
      plan: [
        {
          name: 'Deep breathing exercise',
          description: 'Practice 4-7-8 breathing technique',
          duration: 5,
          priority: 0.9
        },
        {
          name: 'Listen to calming music',
          description: 'Play soft instrumental music',
          duration: 15,
          priority: 0.7
        }
      ],
      confidence: 0.85,
      estimatedDuration: 20,
      explanation: 'Plan combines immediate stress relief with user preferences for music and quiet time'
    }),
    getStats: () => ({
      queriesProcessed: 12,
      averageResponseTime: 45,
      memoryUsage: 85
    }),
    dispose: async () => {}
  };
}

// Use simulated reasoner for demonstration if real one isn't available
if (process.argv.includes('--demo')) {
  console.log('🚧 Running in demo mode with simulated responses\n');
  global.PsychoSymbolicReasoner = function() { return simulateReasoner(); };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export { main as basicUsageExample };