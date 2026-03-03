#!/usr/bin/env node

const { spawn } = require('child_process');
const chalk = require('chalk');

// Test MCP server with extended tools
async function testMCPServer() {
  console.log(chalk.cyan.bold('\n🧪 Testing Extended Strange Loops MCP Server\n'));

  // Start the MCP server
  const server = spawn('node', ['mcp/server-extended.js'], {
    cwd: '/workspaces/sublinear-time-solver/npx-strange-loop'
  });

  // Capture server output
  let serverReady = false;
  server.stderr.on('data', (data) => {
    const msg = data.toString();
    if (msg.includes('Strange Loops Extended MCP Server started')) {
      serverReady = true;
      console.log(chalk.green('✅ MCP Server started successfully'));
      runTests();
    }
  });

  server.stdout.on('data', (data) => {
    try {
      const response = JSON.parse(data.toString());
      if (response.result) {
        console.log(chalk.green('\n📊 Response received:'));
        if (response.result.tools) {
          console.log(`  Found ${response.result.tools.length} tools`);
        } else if (response.result.content) {
          const content = JSON.parse(response.result.content[0].text);
          console.log(chalk.white(JSON.stringify(content, null, 2).substring(0, 500)));
        }
      }
    } catch (e) {
      // Not JSON, ignore
    }
  });

  async function runTests() {
    console.log(chalk.yellow('\n🔧 Running test suite...\n'));

    const tests = [
      // Test 1: List tools
      {
        name: 'List Extended Tools',
        request: {
          jsonrpc: '2.0',
          id: 1,
          method: 'tools/list',
          params: {}
        }
      },

      // Test 2: Create agent task
      {
        name: 'Create Search Task',
        request: {
          jsonrpc: '2.0',
          id: 2,
          method: 'tools/call',
          params: {
            name: 'agent_task_create',
            arguments: {
              taskType: 'search',
              description: 'Find optimal solutions in 100-dimensional space',
              agentCount: 500,
              parameters: {
                searchSpace: 'continuous',
                targetValue: 42
              }
            }
          }
        }
      },

      // Test 3: Perform agent search
      {
        name: 'Agent Search',
        request: {
          jsonrpc: '2.0',
          id: 3,
          method: 'tools/call',
          params: {
            name: 'agent_search',
            arguments: {
              query: 'Find patterns in quantum states',
              searchSpace: {
                type: 'pattern',
                dimensions: 16
              },
              agentCount: 1000,
              strategy: 'quantum_enhanced'
            }
          }
        }
      },

      // Test 4: Analyze data
      {
        name: 'Agent Analysis',
        request: {
          jsonrpc: '2.0',
          id: 4,
          method: 'tools/call',
          params: {
            name: 'agent_analyze',
            arguments: {
              data: [1.2, 3.4, 2.1, 5.6, 4.3, 6.7, 5.4, 7.8, 6.5, 8.9],
              analysisType: 'pattern',
              agentCount: 300
            }
          }
        }
      },

      // Test 5: Optimize function
      {
        name: 'Agent Optimization',
        request: {
          jsonrpc: '2.0',
          id: 5,
          method: 'tools/call',
          params: {
            name: 'agent_optimize',
            arguments: {
              objective: 'Minimize cost function f(x) = x^2 + sin(x)',
              constraints: ['x >= -10', 'x <= 10'],
              dimensions: 20,
              agentCount: 1500,
              iterations: 50
            }
          }
        }
      },

      // Test 6: Temporal prediction
      {
        name: 'Agent Prediction',
        request: {
          jsonrpc: '2.0',
          id: 6,
          method: 'tools/call',
          params: {
            name: 'agent_predict',
            arguments: {
              historicalData: [10, 12, 11, 14, 13, 16, 15, 18, 17, 20],
              horizonSteps: 5,
              agentCount: 400,
              useQuantum: true
            }
          }
        }
      },

      // Test 7: Monitor metrics
      {
        name: 'Agent Monitoring',
        request: {
          jsonrpc: '2.0',
          id: 7,
          method: 'tools/call',
          params: {
            name: 'agent_monitor',
            arguments: {
              metrics: ['cpu', 'memory', 'latency', 'errors'],
              thresholds: {
                cpu: 0.8,
                memory: 0.9,
                errors: 5
              },
              agentCount: 200,
              intervalMs: 100
            }
          }
        }
      },

      // Test 8: Classification
      {
        name: 'Agent Classification',
        request: {
          jsonrpc: '2.0',
          id: 8,
          method: 'tools/call',
          params: {
            name: 'agent_classify',
            arguments: {
              data: ['apple', 'car', 'banana', 'truck', 'orange'],
              categories: ['fruit', 'vehicle', 'animal'],
              agentCount: 250,
              consensusThreshold: 0.75
            }
          }
        }
      },

      // Test 9: Generate solutions
      {
        name: 'Agent Generation',
        request: {
          jsonrpc: '2.0',
          id: 9,
          method: 'tools/call',
          params: {
            name: 'agent_generate',
            arguments: {
              prompt: 'Generate novel sorting algorithm',
              generationType: 'solution',
              agentCount: 800,
              diversityFactor: 0.7
            }
          }
        }
      },

      // Test 10: Validate hypothesis
      {
        name: 'Agent Validation',
        request: {
          jsonrpc: '2.0',
          id: 10,
          method: 'tools/call',
          params: {
            name: 'agent_validate',
            arguments: {
              hypothesis: 'Quantum superposition improves search efficiency',
              testCases: [
                { input: 'classical', expected: 100 },
                { input: 'quantum', expected: 50 }
              ],
              agentCount: 150,
              confidenceThreshold: 0.9
            }
          }
        }
      },

      // Test 11: Coordinate agent groups
      {
        name: 'Agent Coordination',
        request: {
          jsonrpc: '2.0',
          id: 11,
          method: 'tools/call',
          params: {
            name: 'agent_coordinate',
            arguments: {
              groups: [
                { name: 'scouts', agentCount: 100, role: 'exploration' },
                { name: 'analyzers', agentCount: 200, role: 'analysis' },
                { name: 'validators', agentCount: 100, role: 'verification' }
              ],
              coordinationStrategy: 'hierarchical'
            }
          }
        }
      },

      // Test 12: Build consensus
      {
        name: 'Agent Consensus',
        request: {
          jsonrpc: '2.0',
          id: 12,
          method: 'tools/call',
          params: {
            name: 'agent_consensus',
            arguments: {
              proposals: ['Option A', 'Option B', 'Option C'],
              agentCount: 300,
              votingMethod: 'weighted'
            }
          }
        }
      },

      // Test 13: Distribute work
      {
        name: 'Agent Distribution',
        request: {
          jsonrpc: '2.0',
          id: 13,
          method: 'tools/call',
          params: {
            name: 'agent_distribute',
            arguments: {
              workItems: ['Task 1', 'Task 2', 'Task 3', 'Task 4', 'Task 5'],
              agentCount: 500,
              distributionStrategy: 'adaptive'
            }
          }
        }
      }
    ];

    let testIndex = 0;

    function sendNextTest() {
      if (testIndex < tests.length) {
        const test = tests[testIndex];
        console.log(chalk.blue(`\n🔹 Test ${testIndex + 1}: ${test.name}`));
        server.stdin.write(JSON.stringify(test.request) + '\n');
        testIndex++;
        setTimeout(sendNextTest, 1500); // Wait between tests
      } else {
        console.log(chalk.green.bold('\n✅ All tests completed!\n'));
        setTimeout(() => {
          server.kill();
          process.exit(0);
        }, 1000);
      }
    }

    // Start sending tests
    sendNextTest();
  }

  // Error handling
  server.on('error', (err) => {
    console.error(chalk.red('❌ Server error:', err));
  });

  server.on('close', (code) => {
    if (code !== 0 && code !== null) {
      console.error(chalk.red(`❌ Server exited with code ${code}`));
    }
  });
}

// Run the test
testMCPServer().catch(console.error);