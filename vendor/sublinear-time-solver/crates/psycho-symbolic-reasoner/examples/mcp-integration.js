#!/usr/bin/env node

/**
 * MCP Integration Example - Psycho-Symbolic Reasoner
 *
 * This example demonstrates how to integrate the psycho-symbolic reasoner
 * with the Model Context Protocol (MCP) for use with AI agents.
 */

import { FastMCP } from 'fastmcp';
import { createPsychoSymbolicTools } from '../dist/mcp/index.js';

async function main() {
  console.log('🔌 Psycho-Symbolic Reasoner - MCP Integration Example\n');

  try {
    // Create MCP server
    console.log('🚀 Creating FastMCP server...');
    const server = new FastMCP({
      name: "PsychoSymbolicReasoner",
      version: "1.0.0",
      description: "Psycho-symbolic reasoning tools for AI agents"
    });

    // Create and register psycho-symbolic tools
    console.log('🛠️ Registering psycho-symbolic tools...');
    const tools = await createPsychoSymbolicTools({
      knowledgeBasePath: './examples/knowledge-base.json',
      enableLogging: true
    });

    tools.forEach(tool => {
      server.addTool(tool);
      console.log(`  ✅ Registered tool: ${tool.name}`);
    });

    console.log(`\n📋 Available MCP Tools:`);
    console.log('─'.repeat(40));

    // List all available tools
    const toolList = [
      {
        name: 'queryGraph',
        description: 'Perform symbolic graph reasoning queries',
        example: 'find relaxation techniques for stressed users'
      },
      {
        name: 'extractSentiment',
        description: 'Analyze sentiment and emotional context',
        example: 'I\'m feeling overwhelmed with deadlines'
      },
      {
        name: 'extractPreferences',
        description: 'Extract user preferences from text',
        example: 'I prefer working in quiet environments'
      },
      {
        name: 'createPlan',
        description: 'Generate goal-oriented action plans',
        example: 'Goal: reduce stress, State: tired and anxious'
      },
      {
        name: 'analyzeContext',
        description: 'Comprehensive psycho-symbolic analysis',
        example: 'User seems frustrated with current workflow'
      }
    ];

    toolList.forEach((tool, index) => {
      console.log(`${index + 1}. ${tool.name}`);
      console.log(`   Description: ${tool.description}`);
      console.log(`   Example: "${tool.example}"`);
      console.log('');
    });

    // Example of testing tools programmatically
    console.log('🧪 Testing MCP Tools');
    console.log('─'.repeat(25));

    // Test sentiment extraction
    console.log('1. Testing sentiment extraction...');
    try {
      const sentimentResult = await server.callTool('extractSentiment', {
        text: "I'm excited about this new project but worried about the tight deadline",
        includeEmotions: true
      });
      console.log(`   Result: ${sentimentResult.score} (${sentimentResult.primaryEmotion})`);
    } catch (error) {
      console.log(`   Simulated result: Mixed emotions detected (excitement + worry)`);
    }

    // Test preference extraction
    console.log('\n2. Testing preference extraction...');
    try {
      const prefResult = await server.callTool('extractPreferences', {
        text: "I like collaborative work but need quiet time for deep thinking",
        domain: "work_environment"
      });
      console.log(`   Found ${prefResult.preferences?.length || 2} preferences`);
    } catch (error) {
      console.log(`   Simulated result: 2 preferences detected (likes collaboration, needs quiet)`);
    }

    // Test planning
    console.log('\n3. Testing planning...');
    try {
      const planResult = await server.callTool('createPlan', {
        goal: "improve work-life balance",
        currentState: {
          stress: "high",
          workload: "overwhelming",
          timeAvailable: "limited"
        },
        preferences: [
          { type: 'like', object: 'short_breaks' }
        ]
      });
      console.log(`   Generated plan with ${planResult.plan?.length || 3} steps`);
    } catch (error) {
      console.log(`   Simulated result: 3-step plan generated`);
    }

    // MCP Server Configuration Examples
    console.log('\n⚙️ MCP Server Configuration Examples');
    console.log('─'.repeat(45));

    console.log('For Claude Desktop (claude_desktop_config.json):');
    console.log(JSON.stringify({
      "mcpServers": {
        "psycho-reasoner": {
          "command": "npx",
          "args": ["psycho-symbolic-reasoner", "serve", "--transport", "stdio"],
          "env": {
            "PSR_LOG_LEVEL": "info"
          }
        }
      }
    }, null, 2));

    console.log('\nFor VS Code MCP Extension:');
    console.log(JSON.stringify({
      "name": "Psycho-Symbolic Reasoner",
      "command": ["npx", "psycho-symbolic-reasoner", "serve"],
      "args": ["--transport", "stdio"],
      "description": "Psycho-symbolic reasoning for AI agents"
    }, null, 2));

    // Usage examples for AI agents
    console.log('\n🤖 AI Agent Usage Examples');
    console.log('─'.repeat(35));

    const usageExamples = [
      {
        scenario: "Therapy Assistant",
        prompt: "A user says: 'I've been feeling anxious lately about work deadlines.'",
        steps: [
          "1. Use extractSentiment to analyze emotional state",
          "2. Use queryGraph to find anxiety management techniques",
          "3. Use createPlan to generate coping strategies",
          "4. Provide personalized recommendations"
        ]
      },
      {
        scenario: "Personal Productivity Coach",
        prompt: "User: 'I'm struggling to focus during long work sessions.'",
        steps: [
          "1. Use extractPreferences to understand work style",
          "2. Use queryGraph to find focus enhancement techniques",
          "3. Use createPlan to design productivity workflow",
          "4. Monitor progress and adapt recommendations"
        ]
      },
      {
        scenario: "Educational Assistant",
        prompt: "Student: 'I get overwhelmed studying for multiple exams.'",
        steps: [
          "1. Use extractSentiment to assess stress levels",
          "2. Use extractPreferences to identify learning preferences",
          "3. Use createPlan to organize study schedule",
          "4. Provide stress management techniques"
        ]
      }
    ];

    usageExamples.forEach((example, index) => {
      console.log(`${index + 1}. ${example.scenario}`);
      console.log(`   Scenario: ${example.prompt}`);
      console.log(`   Workflow:`);
      example.steps.forEach(step => console.log(`     ${step}`));
      console.log('');
    });

    // Start the MCP server
    console.log('🎯 Starting MCP Server');
    console.log('─'.repeat(25));
    console.log('Server will start on stdio transport...');
    console.log('Use Ctrl+C to stop the server\n');

    // Add signal handling for graceful shutdown
    process.on('SIGINT', async () => {
      console.log('\n🛑 Shutting down MCP server...');
      await server.stop();
      console.log('✅ Server stopped gracefully');
      process.exit(0);
    });

    // Start server (this will block)
    if (!process.argv.includes('--demo')) {
      await server.start({ transportType: "stdio" });
    } else {
      console.log('🚧 Demo mode - server not actually started');
      console.log('✅ MCP integration example completed!');
    }

  } catch (error) {
    console.error('❌ Error:', error.message);
    console.error('Stack:', error.stack);
    process.exit(1);
  }
}

// Simulated MCP tools for demonstration
function createSimulatedMCPTools() {
  return [
    {
      name: 'extractSentiment',
      description: 'Analyze sentiment and emotional context from text',
      parameters: {
        type: 'object',
        properties: {
          text: { type: 'string' },
          includeEmotions: { type: 'boolean' }
        },
        required: ['text']
      },
      execute: async ({ text, includeEmotions }) => {
        return {
          score: Math.random() * 2 - 1,
          primaryEmotion: ['joy', 'sadness', 'anger', 'fear'][Math.floor(Math.random() * 4)],
          confidence: 0.8,
          emotions: includeEmotions ? [
            { emotion: 'neutral', score: 0.1 }
          ] : undefined
        };
      }
    },
    {
      name: 'createPlan',
      description: 'Generate goal-oriented action plans',
      parameters: {
        type: 'object',
        properties: {
          goal: { type: 'string' },
          currentState: { type: 'object' },
          preferences: { type: 'array' }
        },
        required: ['goal']
      },
      execute: async ({ goal, currentState, preferences }) => {
        return {
          plan: [
            { name: 'Take a break', duration: 10 },
            { name: 'Practice mindfulness', duration: 15 },
            { name: 'Review priorities', duration: 20 }
          ],
          confidence: 0.85
        };
      }
    }
  ];
}

// Use simulated tools if in demo mode
if (process.argv.includes('--demo')) {
  console.log('🚧 Running in demo mode with simulated MCP tools\n');
  global.createPsychoSymbolicTools = async () => createSimulatedMCPTools();
  global.FastMCP = class {
    constructor(config) {
      this.config = config;
      this.tools = [];
    }
    addTool(tool) {
      this.tools.push(tool);
    }
    async callTool(name, params) {
      const tool = this.tools.find(t => t.name === name);
      return tool ? await tool.execute(params) : { error: 'Tool not found' };
    }
    async start() {
      console.log('Demo server started');
    }
    async stop() {
      console.log('Demo server stopped');
    }
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export { main as mcpIntegrationExample };