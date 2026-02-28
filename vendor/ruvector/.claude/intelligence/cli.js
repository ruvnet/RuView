#!/usr/bin/env node
/**
 * RuVector Intelligence CLI
 *
 * Commands:
 *   remember <type> <content>     - Store in vector memory
 *   recall <query>                - Search memory semantically
 *   learn <state> <action> <reward> - Record learning trajectory
 *   suggest <state> <actions...>  - Get best action suggestion
 *   route <task> [--file <f>] [--crate <c>] - Route to best agent
 *   stats                         - Show intelligence stats
 *   pre-edit <file>               - Pre-edit intelligence hook
 *   post-edit <file> <success>    - Post-edit learning hook
 */

import RuVectorIntelligence from './index.js';
import SwarmOptimizer from './swarm.js';
import { basename, extname } from 'path';

const intel = new RuVectorIntelligence();
const swarm = new SwarmOptimizer();

async function main() {
  const args = process.argv.slice(2);
  const command = args[0];

  if (!command) {
    console.log(`
🧠 RuVector Intelligence CLI

Commands:
  remember <type> <content>       Store in semantic memory
  recall <query>                  Search memory
  learn <state> <action> <reward> Record trajectory
  suggest <state> <action1,action2,...>  Get best action
  route <task> --file <f> --crate <c>    Route to agent
  stats                           Show system stats

Hooks:
  pre-edit <file>                 Pre-edit intelligence
  post-edit <file> <success>      Post-edit learning
  pre-command <cmd>               Pre-command intelligence
  post-command <cmd> <success> [stderr]  Post-command learning

v3 Features:
  record-error <cmd> <stderr>     Record error for pattern learning
  suggest-fix <error-code>        Get suggested fixes for error
  suggest-next <file>             Suggest next files to edit
  should-test <file>              Check if tests should run

Swarm (Hive-Mind):
  swarm-register <id> <type>      Register agent in swarm
  swarm-coordinate <src> <dst>    Record agent coordination
  swarm-optimize <tasks...>       Optimize task distribution
  swarm-recommend <type>          Get best agent for task
  swarm-heal <agent-id>           Handle agent failure
  swarm-stats                     Show swarm statistics
`);
    return;
  }

  try {
    switch (command) {
      case 'remember': {
        const [, type, ...contentParts] = args;
        const content = contentParts.join(' ');
        const id = await intel.remember(type, content);
        console.log(JSON.stringify({ success: true, id }));
        break;
      }

      case 'recall': {
        const query = args.slice(1).join(' ');
        const results = await intel.recall(query, 5);
        console.log(JSON.stringify({
          query,
          results: results.map(r => ({
            type: r.type,
            content: r.content?.slice(0, 200),
            score: r.score?.toFixed(3),
            timestamp: r.metadata?.timestamp
          }))
        }, null, 2));
        break;
      }

      case 'learn': {
        const [, state, action, rewardStr] = args;
        const reward = parseFloat(rewardStr) || 0;
        const id = intel.learn(state, action, 'recorded', reward);
        console.log(JSON.stringify({ success: true, id, state, action, reward }));
        break;
      }

      case 'suggest': {
        const [, state, actionsStr] = args;
        const actions = actionsStr?.split(',') || ['coder', 'reviewer', 'tester'];
        const suggestion = intel.suggest(state, actions);
        console.log(JSON.stringify(suggestion, null, 2));
        break;
      }

      case 'route': {
        const task = [];
        let file = null, crate = null, operation = 'edit';

        for (let i = 1; i < args.length; i++) {
          if (args[i] === '--file' && args[i + 1]) {
            file = args[++i];
          } else if (args[i] === '--crate' && args[i + 1]) {
            crate = args[++i];
          } else if (args[i] === '--op' && args[i + 1]) {
            operation = args[++i];
          } else {
            task.push(args[i]);
          }
        }

        const fileType = file ? extname(file).slice(1) : 'unknown';
        const routing = await intel.route(task.join(' '), { file, fileType, crate, operation });
        console.log(JSON.stringify(routing, null, 2));
        break;
      }

      case 'stats': {
        const stats = intel.stats();
        console.log(JSON.stringify(stats, null, 2));
        break;
      }

      // === HOOK INTEGRATIONS ===

      case 'pre-edit': {
        const file = args[1];
        if (!file) {
          console.log('{}');
          break;
        }

        const fileType = extname(file).slice(1);
        const fileName = basename(file);
        const crateMatch = file.match(/crates\/([^/]+)/);
        const crate = crateMatch ? crateMatch[1] : null;

        // Build context for routing - use underscore format to match pretrained Q-table
        const state = `edit_${fileType}_in_${crate || 'project'}`;

        // Get routing suggestion
        const routing = await intel.route(
          `edit ${fileName}`,
          { file, fileType, crate, operation: 'edit' }
        );

        // Recall similar past edits
        const similar = await intel.recall(`edit ${fileType} ${crate || ''} ${fileName}`, 3);

        // Get learned suggestion
        const actions = ['check-first', 'edit-directly', 'test-first', 'review-first'];
        const suggestion = intel.suggest(state, actions);

        const output = {
          file,
          fileType,
          crate,
          routing: {
            agent: routing.recommended,
            confidence: routing.confidence,
            reason: routing.reasoning
          },
          suggestion: {
            approach: suggestion.action,
            confidence: suggestion.confidence
          },
          context: similar.length > 0 ? {
            similarEdits: similar.slice(0, 2).map(s => ({
              what: s.content?.slice(0, 80),
              when: s.metadata?.timestamp
            }))
          } : null
        };

        // Pretty output for hook
        console.log('🧠 Intelligence Analysis:');
        console.log(`   📁 ${crate || 'project'}/${fileName}`);
        console.log(`   🤖 Recommended: ${routing.recommended} (${(routing.confidence * 100).toFixed(0)}% confidence)`);
        if (suggestion.confidence > 0) {
          console.log(`   💡 Approach: ${suggestion.action}`);
        }
        if (similar.length > 0) {
          console.log(`   📚 ${similar.length} similar past edits found`);
        }

        break;
      }

      case 'post-edit': {
        const [, file, successStr] = args;
        const success = successStr === 'true' || successStr === '1';
        const reward = success ? 1.0 : -0.5;

        const fileType = extname(file || '').slice(1);
        const crateMatch = (file || '').match(/crates\/([^/]+)/);
        const crate = crateMatch ? crateMatch[1] : null;

        const state = `edit_${fileType}_in_${crate || 'project'}`;
        const action = success ? 'successful-edit' : 'failed-edit';

        // Record trajectory for learning
        intel.learn(state, action, success ? 'completed' : 'failed', reward);

        // v3: Record file edit for sequence learning
        intel.recordFileEdit(file);

        // Store in memory
        await intel.remember(
          'edit',
          `${success ? 'successful' : 'failed'} edit of ${fileType} in ${crate || 'project'}`,
          { file, success, crate }
        );

        // v3: Check if tests should be suggested
        const testSuggestion = intel.shouldSuggestTests(file);

        console.log(`📊 Learning recorded: ${success ? '✅' : '❌'} ${basename(file || 'unknown')}`);

        // v3: Suggest next files
        const nextFiles = intel.suggestNextFiles(file, 2);
        if (nextFiles.length > 0) {
          console.log(`   📁 Often edit next: ${nextFiles.map(f => f.file.split('/').pop()).join(', ')}`);
        }

        // v3: Suggest running tests
        if (testSuggestion.suggest) {
          console.log(`   🧪 Consider: ${testSuggestion.command}`);
        }
        break;
      }

      case 'pre-command': {
        const cmd = args.slice(1).join(' ');

        // Classify command type
        let cmdType = 'other';
        if (cmd.startsWith('cargo')) cmdType = 'cargo';
        else if (cmd.startsWith('npm')) cmdType = 'npm';
        else if (cmd.startsWith('git')) cmdType = 'git';
        else if (cmd.startsWith('wasm-pack')) cmdType = 'wasm';

        const state = `${cmdType}_in_general`;
        const actions = ['command-succeeded', 'command-failed'];
        const suggestion = intel.suggest(state, actions);

        // Recall similar commands
        const similar = await intel.recall(`command ${cmdType} ${cmd.slice(0, 50)}`, 2);

        console.log(`🧠 Command: ${cmdType}`);
        if (suggestion.confidence > 0.3) {
          console.log(`   💡 Suggestion: ${suggestion.action}`);
        }
        if (similar.length > 0 && similar[0].score > 0.6) {
          const lastOutcome = similar[0].metadata?.success ? '✅' : '❌';
          console.log(`   📚 Similar command ran before: ${lastOutcome}`);
        }
        break;
      }

      case 'post-command': {
        // Parse: post-command <cmd> <success> [stderr]
        // Find success flag (true/false/1/0) in args
        let successIdx = args.findIndex((a, i) => i > 0 && (a === 'true' || a === 'false' || a === '1' || a === '0'));
        if (successIdx === -1) successIdx = args.length - 1;

        const success = args[successIdx] === 'true' || args[successIdx] === '1';
        const cmd = args.slice(1, successIdx).join(' ');
        const stderr = args.slice(successIdx + 1).join(' ');

        let cmdType = 'other';
        if (cmd.startsWith('cargo')) cmdType = 'cargo';
        else if (cmd.startsWith('npm')) cmdType = 'npm';
        else if (cmd.startsWith('git')) cmdType = 'git';
        else if (cmd.startsWith('wasm-pack')) cmdType = 'wasm';

        const state = `${cmdType}_in_general`;
        const reward = success ? 1.0 : -0.5;

        intel.learn(state, success ? 'command-succeeded' : 'command-failed', cmd.slice(0, 100), reward);

        // v3: Record error patterns if command failed
        if (!success && stderr) {
          const crateMatch = cmd.match(/-p\s+(\S+)/) || cmd.match(/crates\/([^/\s]+)/);
          const crate = crateMatch ? crateMatch[1] : null;
          const errors = intel.recordError(cmd, stderr, null, crate);
          if (errors.length > 0) {
            console.log(`📊 Command ❌ recorded (${errors.length} error patterns learned)`);
            for (const e of errors.slice(0, 2)) {
              const fix = intel.suggestFix(`${e.type}:${e.code}`);
              if (fix.recentFixes.length > 0) {
                console.log(`   💡 ${e.code}: ${fix.recentFixes[0]}`);
              }
            }
            break;
          }
        }

        await intel.remember(
          'command',
          `${cmdType}: ${cmd.slice(0, 100)}`,
          { success, cmdType }
        );

        console.log(`📊 Command ${success ? '✅' : '❌'} recorded`);
        break;
      }

      // === SWARM / HIVE-MIND COMMANDS ===

      case 'swarm-register': {
        const [, id, type, ...caps] = args;
        const result = swarm.registerAgent(id, type, caps);
        console.log(JSON.stringify(result, null, 2));
        break;
      }

      case 'swarm-coordinate': {
        const [, src, dst, weight] = args;
        const result = swarm.recordCoordination(src, dst, parseFloat(weight) || 1);
        console.log(JSON.stringify(result, null, 2));
        break;
      }

      case 'swarm-optimize': {
        const tasks = args.slice(1);
        const result = swarm.optimizeTaskDistribution(tasks);
        console.log(JSON.stringify(result, null, 2));
        break;
      }

      case 'swarm-recommend': {
        const [, taskType, ...caps] = args;
        const result = swarm.recommendForTask(taskType, caps);
        console.log(JSON.stringify(result, null, 2));
        break;
      }

      case 'swarm-heal': {
        const [, agentId] = args;
        const result = swarm.handleFailure(agentId);
        console.log(JSON.stringify(result, null, 2));
        break;
      }

      case 'swarm-stats': {
        const stats = swarm.getStats();
        console.log(JSON.stringify(stats, null, 2));
        break;
      }

      // === V3 FEATURES ===

      case 'record-error': {
        const cmd = args[1] || '';
        const stderr = args.slice(2).join(' ');
        const errors = intel.recordError(cmd, stderr);
        console.log(JSON.stringify({ recorded: errors.length, errors }, null, 2));
        break;
      }

      case 'suggest-fix': {
        const errorCode = args[1];
        const suggestion = intel.suggestFix(errorCode);
        console.log(JSON.stringify(suggestion, null, 2));
        break;
      }

      case 'suggest-next': {
        const file = args[1];
        const suggestions = intel.suggestNextFiles(file);
        console.log(JSON.stringify(suggestions, null, 2));
        break;
      }

      case 'should-test': {
        const file = args[1];
        const suggestion = intel.shouldSuggestTests(file);
        console.log(JSON.stringify(suggestion, null, 2));
        break;
      }

      default:
        console.error(`Unknown command: ${command}`);
        process.exit(1);
    }
  } catch (error) {
    console.error('Error:', error.message);
    process.exit(1);
  }
}

main();
