#!/usr/bin/env node

/**
 * GOAP MCP CLI
 * Command-line interface for the GOAP MCP server
 */

import { Command } from 'commander';
import { GoapMCPServer } from './mcp/server.js';
import { SearchResult } from './core/types.js';
import dotenv from 'dotenv';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

// Load environment variables
dotenv.config();

// Get package.json version
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const packageJson = JSON.parse(readFileSync(join(__dirname, '..', 'package.json'), 'utf-8'));

const program = new Command();

program
  .name('goalie')
  .description('AI-powered research assistant using Goal-Oriented Action Planning')
  .version(packageJson.version);

// Start MCP Server Command
program
  .command('start')
  .description('Start the MCP server')
  .option('--verbose', 'Enable verbose logging')
  .option('--plugins <paths>', 'Comma-separated paths to external plugins')
  .option('--extensions <paths>', 'Comma-separated paths to external extensions')
  .option('--port <number>', 'Port to run HTTP server on (if not stdio)')
  .action(async (options) => {
    try {
      // Set environment variables from options
      if (options.plugins) {
        process.env.GOAP_PLUGINS = options.plugins;
      }
      if (options.extensions) {
        process.env.GOAP_EXTENSIONS = options.extensions;
      }

      if (options.verbose) {
        console.error('🔧 Verbose logging enabled');
        console.error('🌐 Environment:');
        console.error(`  • Perplexity API Key: ${process.env.PERPLEXITY_API_KEY ? '✅ Set' : '❌ Missing'}`);
        console.error(`  • Plugins: ${process.env.GOAP_PLUGINS || 'None'}`);
        console.error(`  • Extensions: ${process.env.GOAP_EXTENSIONS || 'None'}`);
      }

      // Validate required environment variables
      if (!process.env.PERPLEXITY_API_KEY) {
        console.error('❌ ERROR: PERPLEXITY_API_KEY environment variable is required');
        console.error('💡 Get your API key from: https://www.perplexity.ai/settings/api');
        process.exit(1);
      }

      const server = new GoapMCPServer();
      await server.initialize();
      await server.run();

    } catch (error) {
      console.error('💥 Failed to start GOAP MCP server:', error);
      process.exit(1);
    }
  });

// Main Search Command (goap.search equivalent)
program
  .command('search <query>')
  .description('Execute intelligent search using GOAP planning')
  .option('-d, --domains <domains>', 'Domain restrictions (comma-separated, e.g., edu,gov)')
  .option('-r, --recency <recency>', 'Recency filter (hour|day|week|month|year)')
  .option('-m, --mode <mode>', 'Search mode (web|academic)', 'web')
  .option('--max-results <number>', 'Maximum search results (1-20)', '10')
  .option('--model <model>', 'Perplexity model (sonar|sonar-pro|sonar-deep-research)', 'sonar-pro')
  .option('--no-reasoning', 'Disable Advanced Reasoning Engine')
  .option('--timeout <seconds>', 'Planning timeout in seconds', '30')
  .option('--output <path>', 'Output directory', '.research')
  .option('--format <format>', 'Output format (json|markdown|both)', 'both')
  .option('--no-save', 'Do not save to file')
  .option('--no-subfolder', 'Do not create query-based subfolder')
  .option('--page <number>', 'Page number for pagination', '1')
  .option('--page-size <number>', 'Items per page (5-50)', '10')
  .option('--verify', 'Enable Ed25519 signature verification')
  .option('--strict-verify', 'Require all citations to be signed')
  .option('--sign', 'Sign result with Ed25519')
  .option('--sign-key <key>', 'Base64 encoded Ed25519 private key')
  .option('--key-id <id>', 'Key identifier for signing')
  .option('--cert-id <id>', 'Certificate ID for mandate chain')
  .option('--trusted-issuers <issuers>', 'Trusted certificate issuers (comma-separated)')
  .action(async (query, options) => {
    try {
      const { GoapMCPTools } = await import('./mcp/tools.js');
      const tools = new GoapMCPTools();
      await tools.initialize();

      console.log('🔍 Executing GOAP search...');
      console.log(`📝 Query: ${query}`);

      // Parse comma-separated values
      const domains = options.domains ? options.domains.split(',').map((d: string) => d.trim()) : undefined;
      const trustedIssuers = options.trustedIssuers
        ? options.trustedIssuers.split(',').map((i: string) => i.trim())
        : ['perplexity-ai', 'openai', 'anthropic'];

      // Build Ed25519 verification config if needed
      const ed25519Verification = (options.verify || options.strictVerify || options.sign) ? {
        enabled: true,
        requireSignatures: options.strictVerify || false,
        signResult: options.sign || false,
        privateKey: options.signKey,
        keyId: options.keyId,
        certId: options.certId,
        trustedIssuers
      } : undefined;

      // Add timeout wrapper
      const timeout = parseInt(options.timeout) * 1000 || 30000;
      const resultPromise = tools.executeGoapSearch({
        query,
        domains,
        recency: options.recency,
        mode: options.mode,
        maxResults: parseInt(options.maxResults),
        model: options.model,
        enableReasoning: options.reasoning !== false,
        planningTimeout: parseInt(options.timeout),
        outputToFile: options.save !== false,
        outputFormat: options.format,
        outputPath: options.output,
        useQuerySubfolder: options.subfolder !== false,
        pagination: {
          page: parseInt(options.page),
          pageSize: parseInt(options.pageSize)
        },
        ed25519Verification
      });

      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error(`Search timed out after ${timeout/1000} seconds`)), timeout);
      });

      const result = await Promise.race([resultPromise, timeoutPromise]) as SearchResult;

      // Display results
      console.log('\n✅ Search completed!');
      console.log('━'.repeat(50));

      const answerPreview = result.answer.length > 500
        ? result.answer.substring(0, 500) + '...\n\n[Full answer in files]'
        : result.answer;

      console.log('\n📄 Answer:');
      console.log(answerPreview);

      console.log('\n📊 Metadata:');
      console.log(`  • Citations: ${result.citations.length}`);
      console.log(`  • Execution time: ${result.metadata.executionTime}ms`);
      console.log(`  • Replanned: ${result.metadata.replanned ? 'Yes' : 'No'}`);

      if (result.paginationInfo) {
        console.log(`  • Page: ${result.paginationInfo.currentPage || 1}/${result.paginationInfo.totalPages || 1}`);
        console.log(`  • Total results: ${result.paginationInfo.totalResults || result.citations?.length || 0}`);
      }

      if (result.metadata.ed25519Verification) {
        const v = result.metadata.ed25519Verification;
        console.log(`\n🔐 Verification:`);
        console.log(`  • Verified citations: ${v.verified}/${v.total}`);
        if (v.untrusted.length > 0) {
          console.log(`  • Untrusted sources: ${v.untrusted.join(', ')}`);
        }
      }

      if (result.metadata.savedFiles && options.save !== false) {
        console.log('\n💾 Files saved:');
        for (const file of result.metadata.savedFiles) {
          console.log(`  • ${file}`);
        }
      }

      // Force exit immediately
      process.exit(0);
    } catch (error) {
      console.error('💥 Search failed:', error);
      process.exit(1);
    }
  });

// Plan Explanation Command
program
  .command('explain <query>')
  .description('Explain GOAP planning for a query without executing')
  .option('--no-steps', 'Hide step-by-step breakdown')
  .option('--no-reasoning', 'Hide reasoning analysis')
  .action(async (query, options) => {
    try {
      const { GoapMCPTools } = await import('./mcp/tools.js');
      const tools = new GoapMCPTools();
      await tools.initialize();

      console.log('🧠 Generating plan explanation...');
      console.log(`📝 Query: ${query}`);

      const explanation = await tools.executePlanExplain({
        query,
        showSteps: options.steps !== false,
        showReasoning: options.reasoning !== false
      });

      console.log('\n📋 Plan Explanation:');
      console.log(JSON.stringify(explanation, null, 2));

      process.exit(0);
    } catch (error) {
      console.error('💥 Explanation failed:', error);
      process.exit(1);
    }
  });

// Raw Perplexity Search Command
program
  .command('raw <queries...>')
  .description('Direct Perplexity search without GOAP planning')
  .option('-d, --domains <domains>', 'Domain restrictions (comma-separated)')
  .option('-r, --recency <recency>', 'Recency filter (hour|day|week|month|year)')
  .option('-m, --mode <mode>', 'Search mode (web|academic)', 'web')
  .option('--max-results <number>', 'Maximum results (1-20)', '10')
  .action(async (queries, options) => {
    try {
      const { GoapMCPTools } = await import('./mcp/tools.js');
      const tools = new GoapMCPTools();
      await tools.initialize();

      console.log('🔍 Executing raw Perplexity search...');
      console.log(`📝 Queries: ${queries.join(', ')}`);

      const domains = options.domains ? options.domains.split(',').map((d: string) => d.trim()) : undefined;

      const result = await tools.executeRawSearch({
        query: queries,
        domains,
        recency: options.recency,
        mode: options.mode,
        maxResults: parseInt(options.maxResults)
      });

      console.log('\n✅ Raw search completed!');
      console.log(JSON.stringify(result, null, 2));

      process.exit(0);
    } catch (error) {
      console.error('💥 Raw search failed:', error);
      process.exit(1);
    }
  });

// Plugin Management Commands
const pluginsCmd = program
  .command('plugins')
  .description('Plugin management');

pluginsCmd
  .command('list')
  .description('List all available plugins')
  .action(async () => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          const plugins = await tools.executeToolByName('plugin.list', {});

          console.log('🔌 Available Plugins:');
          console.log(JSON.stringify(plugins, null, 2));

          process.exit(0);
        } catch (error) {
          console.error('💥 Failed to list plugins:', error);
          process.exit(1);
        }
      });

pluginsCmd
  .command('enable <name>')
  .description('Enable a plugin')
  .action(async (name) => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          await tools.executeToolByName('plugin.enable', { name });
          console.log(`✅ Plugin '${name}' enabled`);

          process.exit(0);
        } catch (error) {
          console.error('💥 Failed to enable plugin:', error);
          process.exit(1);
        }
      });

pluginsCmd
  .command('disable <name>')
  .description('Disable a plugin')
  .action(async (name) => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          await tools.executeToolByName('plugin.disable', { name });
          console.log(`✅ Plugin '${name}' disabled`);

          process.exit(0);
        } catch (error) {
          console.error('💥 Failed to disable plugin:', error);
          process.exit(1);
        }
      });

pluginsCmd
  .command('info <name>')
  .description('Get plugin information')
  .action(async (name) => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          const info = await tools.executeToolByName('plugin.info', { name });
          console.log(`🔌 Plugin Information for '${name}':`);
          console.log(JSON.stringify(info, null, 2));

          process.exit(0);
        } catch (error) {
          console.error('💥 Failed to get plugin info:', error);
          process.exit(1);
        }
      });

// Advanced Reasoning Commands
const reasoningCmd = program
  .command('reasoning')
  .description('Advanced reasoning capabilities');

reasoningCmd
  .command('chain-of-thought <query>')
  .description('Apply Chain-of-Thought reasoning with Tree-of-Thoughts')
  .option('--depth <number>', 'Reasoning depth (1-5)', '3')
  .option('--branches <number>', 'Number of branches (2-10)', '3')
  .action(async (query, options) => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          console.log('🧠 Applying Chain-of-Thought reasoning...');

          const result = await tools.executeToolByName('reasoning.chain_of_thought', {
            query,
            depth: parseInt(options.depth),
            branches: parseInt(options.branches)
          });

          console.log('\n✅ Reasoning complete:');
          console.log(JSON.stringify(result, null, 2));

          process.exit(0);
        } catch (error) {
          console.error('💥 Reasoning failed:', error);
          process.exit(1);
        }
      });

reasoningCmd
  .command('consistency <query>')
  .description('Check reasoning consistency with majority voting')
  .option('--samples <number>', 'Number of samples (3-10)', '5')
  .action(async (query, options) => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          console.log('🧠 Checking reasoning consistency...');

          const result = await tools.executeToolByName('reasoning.self_consistency', {
            query,
            samples: parseInt(options.samples)
          });

          console.log('\n✅ Consistency check complete:');
          console.log(JSON.stringify(result, null, 2));

          process.exit(0);
        } catch (error) {
          console.error('💥 Consistency check failed:', error);
          process.exit(1);
        }
      });

reasoningCmd
  .command('verify <claims...>')
  .description('Verify claims with citation grounding')
  .option('--citations <citations>', 'Available citations (comma-separated)')
  .action(async (claims, options) => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          console.log('🧠 Verifying claims...');

          const citations = options.citations
            ? options.citations.split(',').map((c: string) => c.trim())
            : [];

          const result = await tools.executeToolByName('reasoning.anti_hallucination', {
            claims,
            citations
          });

          console.log('\n✅ Verification complete:');
          console.log(JSON.stringify(result, null, 2));

          process.exit(0);
        } catch (error) {
          console.error('💥 Verification failed:', error);
          process.exit(1);
        }
      });

reasoningCmd
  .command('agents <query>')
  .description('Orchestrate multiple research agents')
  .option('--agents <types>', 'Agent types (comma-separated)', 'researcher,fact_checker,synthesizer,critic,summarizer')
  .option('--sequential', 'Execute agents sequentially instead of in parallel')
  .action(async (query, options) => {
        try {
          const { GoapMCPTools } = await import('./mcp/tools.js');
          const tools = new GoapMCPTools();
          await tools.initialize();

          console.log('🤖 Orchestrating research agents...');

          const agents = options.agents.split(',').map((a: string) => a.trim());

          const result = await tools.executeToolByName('reasoning.agentic_research', {
            query,
            agents,
            parallel: !options.sequential
          });

          console.log('\n✅ Agent orchestration complete:');
          console.log(JSON.stringify(result, null, 2));

          process.exit(0);
        } catch (error) {
          console.error('💥 Agent orchestration failed:', error);
          process.exit(1);
        }
      });

// Legacy test command with updated features
program
  .command('test')
  .description('Test the GOAP planner with a sample query')
  .option('--query <string>', 'Test query', 'What are the latest developments in AI?')
  .option('--explain', 'Show plan explanation without executing')
  .action(async (options) => {
    try {
      const { GoapMCPTools } = await import('./mcp/tools.js');
      const tools = new GoapMCPTools();
      await tools.initialize();

      console.log('🧪 Testing GOAP planner...');
      console.log(`📝 Query: ${options.query}`);

      if (options.explain) {
        const explanation = await tools.executePlanExplain({
          query: options.query,
          showSteps: true,
          showReasoning: true
        });

        console.log('📋 Plan Explanation:');
        console.log(JSON.stringify(explanation, null, 2));
      } else {
        // Add timeout wrapper
        const timeout = 30000; // 30 seconds
        const resultPromise = tools.executeGoapSearch({
          query: options.query,
          enableReasoning: true,
          maxResults: 5,
          outputToFile: true,
          outputPath: '.research',
          useQuerySubfolder: false,
          outputFormat: 'both'
        });

        const timeoutPromise = new Promise((_, reject) => {
          setTimeout(() => reject(new Error('Test timed out after 30 seconds')), timeout);
        });

        const result = await Promise.race([resultPromise, timeoutPromise]) as SearchResult;

        console.log('✅ Test Results:');
        console.log(`📝 Answer: ${result.answer.substring(0, 200)}...`);
        console.log(`📚 Citations: ${result.citations.length}`);
        console.log(`⏱️ Execution time: ${result.metadata.executionTime}ms`);
        console.log(`🔄 Replanned: ${result.metadata.replanned}`);

        if (result.metadata.savedFiles) {
          console.log(`💾 Output saved to: ${result.metadata.savedFiles.join(', ')}`);
        }
      }

      // Force exit after a brief delay to ensure all output is flushed
      setTimeout(() => {
        process.exit(0);
      }, 100);

    } catch (error) {
      console.error('💥 Test failed:', error);
      process.exit(1);
    }
  });

// Legacy query command for backward compatibility
program
  .command('query <question>')
  .description('Execute a research query (legacy, use "search" instead)')
  .option('--no-save', 'Do not save output to files')
  .option('--output <path>', 'Output directory path', '.research')
  .option('--format <format>', 'Output format (json, markdown, both)', 'both')
  .option('--max-results <number>', 'Maximum search results', '10')
  .option('--model <model>', 'Perplexity model', 'sonar-pro')
  .option('--explain', 'Show plan explanation without executing')
  .action(async (question, options) => {
    try {
      const { GoapMCPTools } = await import('./mcp/tools.js');
      const tools = new GoapMCPTools();
      await tools.initialize();

      console.log('🔍 Executing research query...');
      console.log(`📝 Query: ${question}`);

      if (options.explain) {
        const explanation = await tools.executePlanExplain({
          query: question,
          showSteps: true,
          showReasoning: true
        });

        console.log('📋 Plan Explanation:');
        console.log(JSON.stringify(explanation, null, 2));
        process.exit(0);
        return;
      }

      // Add timeout wrapper
      const timeout = 30000; // 30 seconds
      const resultPromise = tools.executeGoapSearch({
        query: question,
        enableReasoning: true,
        maxResults: parseInt(options.maxResults),
        model: options.model,
        outputToFile: options.save !== false,
        outputPath: options.output,
        useQuerySubfolder: true,
        outputFormat: options.format
      });

      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error('Query timed out after 30 seconds')), timeout);
      });

      const result = await Promise.race([resultPromise, timeoutPromise]) as SearchResult;

      // Display summary
      console.log('\n✅ Research completed!');
      console.log('━'.repeat(50));

      const answerPreview = result.answer.length > 500
        ? result.answer.substring(0, 500) + '...\n\n[Full answer saved to file]'
        : result.answer;

      console.log('\n📄 Answer:');
      console.log(answerPreview);

      console.log('\n📊 Statistics:');
      console.log(`  • Citations: ${result.citations.length}`);
      console.log(`  • Execution time: ${result.metadata.executionTime}ms`);
      console.log(`  • Replanned: ${result.metadata.replanned ? 'Yes' : 'No'}`);

      if (result.usage) {
        console.log(`  • Tokens used: ${result.usage.tokens || 'N/A'}`);
      }

      if (result.metadata.savedFiles && options.save !== false) {
        console.log('\n💾 Files saved:');
        for (const file of result.metadata.savedFiles) {
          console.log(`  • ${file}`);
        }
      }

      // Force exit immediately
      process.exit(0);

    } catch (error) {
      console.error('💥 Query failed:', error);
      process.exit(1);
    }
  });

// Validation command
program
  .command('validate')
  .description('Validate configuration and dependencies')
  .action(async () => {
    console.log('🔍 Validating GOAP MCP configuration...');

    // Check environment variables
    const checks = [
      {
        name: 'Perplexity API Key',
        check: () => !!process.env.PERPLEXITY_API_KEY,
        fix: 'Set PERPLEXITY_API_KEY environment variable'
      },
      {
        name: 'Node.js version',
        check: () => {
          const version = process.version;
          const major = parseInt(version.slice(1).split('.')[0]);
          return major >= 18;
        },
        fix: 'Update Node.js to version 18 or higher'
      }
    ];

    let allPassed = true;

    for (const check of checks) {
      const passed = check.check();
      const status = passed ? '✅' : '❌';
      console.log(`${status} ${check.name}`);

      if (!passed) {
        console.log(`   💡 ${check.fix}`);
        allPassed = false;
      }
    }

    // Test Advanced Reasoning Engine WASM
    try {
      const { AdvancedReasoningEngine } = await import('./core/advanced-reasoning-engine');
      const engine = new AdvancedReasoningEngine();
      await engine.initialize();
      console.log('✅ Advanced Reasoning Engine integration');
    } catch (error) {
      console.log('⚠️ Advanced Reasoning Engine (will use fallback)');
    }

    // Test MCP SDK
    try {
      await import('@modelcontextprotocol/sdk/server/index.js');
      console.log('✅ MCP SDK');
    } catch (error) {
      console.log('❌ MCP SDK - npm install required');
      allPassed = false;
    }

    if (allPassed) {
      console.log('🎉 All validations passed! Ready to run GOAP MCP server.');
    } else {
      console.log('⚠️ Some validations failed. Please fix the issues above.');
      process.exit(1);
    }
  });

// Info command
program
  .command('info')
  .description('Show system information and capabilities')
  .action(async () => {
    console.log('🎯 GOAP MCP Server Information');
    console.log('==============================');
    console.log('');

    console.log('📋 Core Features:');
    console.log('  • STRIPS-style preconditions and effects');
    console.log('  • A* pathfinding for optimal plans');
    console.log('  • Dynamic re-planning on failure');
    console.log('  • Advanced Reasoning Engine enhanced reasoning');
    console.log('  • Perplexity API integration');
    console.log('  • Extensible plugin system');
    console.log('');

    console.log('🔧 Available Tools:');
    console.log('  • goap.search - Intelligent search with planning');
    console.log('  • goap.plan.explain - Plan explanation');
    console.log('  • search.raw - Direct Perplexity search');
    console.log('  • plugin.* - Plugin management tools');
    console.log('  • reasoning.* - Advanced reasoning tools');
    console.log('');

    console.log('🎪 Plugin System:');
    console.log('  • cost-tracker - Track execution costs');
    console.log('  • performance-monitor - Monitor execution performance');
    console.log('  • logger - Comprehensive logging');
    console.log('  • query-diversifier - Enhance search queries');
    console.log('  • chain-of-thought - CoT reasoning');
    console.log('  • self-consistency - Consistency checking');
    console.log('  • anti-hallucination - Citation grounding');
    console.log('  • agentic-research - Multi-agent coordination');
    console.log('');

    console.log('🧠 Advanced Reasoning Engine:');
    console.log('  • Pattern analysis algorithms');
    console.log('  • Predictive modeling capabilities');
    console.log('  • State-enhanced reasoning');
    console.log('  • Multi-agent coordination');
    console.log('  • Ed25519 cryptographic verification');
    console.log('');

    console.log('🌟 Advantages over standard web search:');
    console.log('  • Multi-step planning with dependencies');
    console.log('  • Automatic query optimization');
    console.log('  • Enhanced reasoning with Advanced Reasoning Engine');
    console.log('  • Dynamic re-planning on failures');
    console.log('  • Comprehensive answer verification');
    console.log('  • Cost optimization with A* pathfinding');
    console.log('  • Extensible plugin architecture');
    console.log('  • Cryptographic citation verification');
    console.log('');

    console.log('💡 Quick Start Examples:');
    console.log('  npx goalie search "latest AI developments"');
    console.log('  npx goalie explain "quantum computing breakthroughs"');
    console.log('  npx goalie reasoning chain-of-thought "solve climate change"');
    console.log('  npx goalie plugins list');
  });

// Default command runs the server
program.parse();

// If no command provided, show help
if (!process.argv.slice(2).length) {
  program.outputHelp();
}