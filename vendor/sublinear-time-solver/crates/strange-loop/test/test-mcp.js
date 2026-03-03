#!/usr/bin/env node

/**
 * Test script for Strange Loops MCP Server
 */

const { spawn } = require('child_process');
const path = require('path');

async function testMCPServer() {
  console.log('🧪 Testing Strange Loops MCP Server...\n');

  const serverPath = path.join(__dirname, '..', 'mcp', 'server.js');

  // Start MCP server process
  const server = spawn('node', [serverPath], {
    stdio: ['pipe', 'pipe', 'inherit']
  });

  let responseBuffer = '';
  let requestId = 1;

  server.stdout.on('data', (data) => {
    responseBuffer += data.toString();

    // Try to parse complete JSON-RPC responses
    const lines = responseBuffer.split('\n');
    responseBuffer = lines.pop() || ''; // Keep incomplete line

    for (const line of lines) {
      if (line.trim()) {
        try {
          const response = JSON.parse(line);
          console.log('📥 Response:', JSON.stringify(response, null, 2));
        } catch (e) {
          console.log('📥 Raw output:', line);
        }
      }
    }
  });

  // Helper function to send JSON-RPC requests
  function sendRequest(method, params = {}) {
    const request = {
      jsonrpc: '2.0',
      id: requestId++,
      method,
      params
    };

    console.log('📤 Request:', JSON.stringify(request, null, 2));
    server.stdin.write(JSON.stringify(request) + '\n');
  }

  // Wait for server to start
  await new Promise(resolve => setTimeout(resolve, 1000));

  try {
    // Test 1: List available tools
    console.log('🔧 Test 1: Listing available tools');
    sendRequest('tools/list');
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Test 2: Get system info
    console.log('\n📊 Test 2: Getting system information');
    sendRequest('tools/call', {
      name: 'system_info',
      arguments: {}
    });
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Test 3: Create nano-agent swarm
    console.log('\n🤖 Test 3: Creating nano-agent swarm');
    sendRequest('tools/call', {
      name: 'nano_swarm_create',
      arguments: {
        agentCount: 100,
        topology: 'mesh'
      }
    });
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Test 4: Run benchmark
    console.log('\n🏃 Test 4: Running benchmark');
    sendRequest('tools/call', {
      name: 'benchmark_run',
      arguments: {
        agentCount: 500,
        durationMs: 1000
      }
    });
    await new Promise(resolve => setTimeout(resolve, 2000));

    // Test 5: Quantum operations
    console.log('\n⚛️ Test 5: Quantum operations');
    sendRequest('tools/call', {
      name: 'quantum_superposition',
      arguments: {
        qubits: 3
      }
    });
    await new Promise(resolve => setTimeout(resolve, 1000));

    sendRequest('tools/call', {
      name: 'quantum_measure',
      arguments: {
        qubits: 3
      }
    });
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Test 6: Temporal prediction
    console.log('\n🔮 Test 6: Temporal prediction');
    sendRequest('tools/call', {
      name: 'temporal_predict',
      arguments: {
        currentValues: [1.0, 2.0, 3.0, 4.0]
      }
    });
    await new Promise(resolve => setTimeout(resolve, 1000));

    console.log('\n✅ MCP Server tests completed successfully!');

  } catch (error) {
    console.error('❌ Test failed:', error);
  } finally {
    // Clean shutdown
    server.kill('SIGTERM');
  }
}

// Run tests
testMCPServer().catch(console.error);