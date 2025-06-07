// Test Runner for WiFi DensePose UI

import { API_CONFIG, buildApiUrl, buildWsUrl } from '../config/api.config.js';
import { apiService } from '../services/api.service.js';
import { wsService } from '../services/websocket.service.js';
import { poseService } from '../services/pose.service.js';
import { healthService } from '../services/health.service.js';
import { TabManager } from '../components/TabManager.js';

class TestRunner {
  constructor() {
    this.tests = [];
    this.results = {
      total: 0,
      passed: 0,
      failed: 0,
      pending: 0
    };
    this.output = [];
  }

  // Add a test
  test(name, category, testFn) {
    this.tests.push({
      name,
      category,
      fn: testFn,
      status: 'pending'
    });
  }

  // Run all tests
  async runAllTests() {
    this.clearResults();
    this.log('Starting test suite...\n');
    
    for (const test of this.tests) {
      await this.runSingleTest(test);
    }
    
    this.updateSummary();
    this.log(`\nTest suite completed. ${this.results.passed}/${this.results.total} tests passed.`);
  }

  // Run tests by category
  async runTestsByCategory(category) {
    this.clearResults();
    const categoryTests = this.tests.filter(test => test.category === category);
    
    this.log(`Starting ${category} tests...\n`);
    
    for (const test of categoryTests) {
      await this.runSingleTest(test);
    }
    
    this.updateSummary();
    this.log(`\n${category} tests completed. ${this.results.passed}/${this.results.total} tests passed.`);
  }

  // Run a single test
  async runSingleTest(test) {
    this.log(`Running: ${test.name}...`);
    
    try {
      const startTime = Date.now();
      await test.fn();
      const duration = Date.now() - startTime;
      
      test.status = 'pass';
      this.results.passed++;
      this.log(`  ✓ PASS (${duration}ms)`);
      
    } catch (error) {
      test.status = 'fail';
      test.error = error.message;
      this.results.failed++;
      this.log(`  ✗ FAIL: ${error.message}`);
      
    } finally {
      this.results.total++;
      this.updateTestDisplay(test);
    }
  }

  // Assertion helpers
  assert(condition, message) {
    if (!condition) {
      throw new Error(message || 'Assertion failed');
    }
  }

  assertEqual(actual, expected, message) {
    if (actual !== expected) {
      throw new Error(message || `Expected ${expected}, got ${actual}`);
    }
  }

  assertNotEqual(actual, unexpected, message) {
    if (actual === unexpected) {
      throw new Error(message || `Expected not to equal ${unexpected}`);
    }
  }

  assertThrows(fn, message) {
    try {
      fn();
      throw new Error(message || 'Expected function to throw');
    } catch (error) {
      // Expected
    }
  }

  async assertRejects(promise, message) {
    try {
      await promise;
      throw new Error(message || 'Expected promise to reject');
    } catch (error) {
      // Expected
    }
  }

  // Logging
  log(message) {
    this.output.push(message);
    const outputElement = document.getElementById('testOutput');
    if (outputElement) {
      outputElement.style.display = 'block';
      outputElement.textContent = this.output.join('\n');
      outputElement.scrollTop = outputElement.scrollHeight;
    }
  }

  // Clear results
  clearResults() {
    this.results = { total: 0, passed: 0, failed: 0, pending: 0 };
    this.output = [];
    
    // Reset test statuses
    this.tests.forEach(test => {
      test.status = 'pending';
      delete test.error;
    });
    
    // Clear UI
    this.updateSummary();
    this.tests.forEach(test => this.updateTestDisplay(test));
    
    const outputElement = document.getElementById('testOutput');
    if (outputElement) {
      outputElement.style.display = 'none';
      outputElement.textContent = '';
    }
  }

  // Update test display
  updateTestDisplay(test) {
    const container = document.getElementById(`${test.category}Tests`);
    if (!container) return;

    let testElement = container.querySelector(`[data-test="${test.name}"]`);
    if (!testElement) {
      testElement = document.createElement('div');
      testElement.className = 'test-case';
      testElement.setAttribute('data-test', test.name);
      testElement.innerHTML = `
        <div class="test-name">${test.name}</div>
        <div class="test-status pending">pending</div>
      `;
      container.appendChild(testElement);
    }

    const statusElement = testElement.querySelector('.test-status');
    statusElement.className = `test-status ${test.status}`;
    statusElement.textContent = test.status;
    
    if (test.error) {
      statusElement.title = test.error;
    }
  }

  // Update summary
  updateSummary() {
    document.getElementById('totalTests').textContent = this.results.total;
    document.getElementById('passedTests').textContent = this.results.passed;
    document.getElementById('failedTests').textContent = this.results.failed;
    document.getElementById('pendingTests').textContent = this.tests.length - this.results.total;
  }
}

// Create test runner instance
const testRunner = new TestRunner();

// Mock DOM elements for testing
function createMockContainer() {
  const container = document.createElement('div');
  container.innerHTML = `
    <nav class="nav-tabs">
      <button class="nav-tab active" data-tab="dashboard">Dashboard</button>
      <button class="nav-tab" data-tab="hardware">Hardware</button>
    </nav>
    <div id="dashboard" class="tab-content active"></div>
    <div id="hardware" class="tab-content"></div>
  `;
  return container;
}

// API Configuration Tests
testRunner.test('API_CONFIG contains required endpoints', 'apiConfig', () => {
  testRunner.assert(API_CONFIG.ENDPOINTS, 'ENDPOINTS should exist');
  testRunner.assert(API_CONFIG.ENDPOINTS.POSE, 'POSE endpoints should exist');
  testRunner.assert(API_CONFIG.ENDPOINTS.HEALTH, 'HEALTH endpoints should exist');
  testRunner.assert(API_CONFIG.ENDPOINTS.STREAM, 'STREAM endpoints should exist');
});

testRunner.test('buildApiUrl constructs correct URLs', 'apiConfig', () => {
  const url = buildApiUrl('/api/v1/pose/current', { zone_id: 'zone1', limit: 10 });
  testRunner.assert(url.includes('/api/v1/pose/current'), 'URL should contain endpoint');
  testRunner.assert(url.includes('zone_id=zone1'), 'URL should contain zone_id parameter');
  testRunner.assert(url.includes('limit=10'), 'URL should contain limit parameter');
});

testRunner.test('buildApiUrl handles path parameters', 'apiConfig', () => {
  const url = buildApiUrl('/api/v1/pose/zones/{zone_id}/occupancy', { zone_id: 'zone1' });
  testRunner.assert(url.includes('/api/v1/pose/zones/zone1/occupancy'), 'URL should replace path parameter');
  testRunner.assert(!url.includes('{zone_id}'), 'URL should not contain placeholder');
});

testRunner.test('buildWsUrl constructs WebSocket URLs', 'apiConfig', () => {
  const url = buildWsUrl('/api/v1/stream/pose', { token: 'test-token' });
  testRunner.assert(url.startsWith('ws://') || url.startsWith('wss://'), 'URL should be WebSocket protocol');
  testRunner.assert(url.includes('/api/v1/stream/pose'), 'URL should contain endpoint');
  testRunner.assert(url.includes('token=test-token'), 'URL should contain token parameter');
});

// API Service Tests
testRunner.test('apiService has required methods', 'apiService', () => {
  testRunner.assert(typeof apiService.get === 'function', 'get method should exist');
  testRunner.assert(typeof apiService.post === 'function', 'post method should exist');
  testRunner.assert(typeof apiService.put === 'function', 'put method should exist');
  testRunner.assert(typeof apiService.delete === 'function', 'delete method should exist');
});

testRunner.test('apiService can set auth token', 'apiService', () => {
  const token = 'test-token-123';
  apiService.setAuthToken(token);
  testRunner.assertEqual(apiService.authToken, token, 'Auth token should be set');
});

testRunner.test('apiService builds correct headers', 'apiService', () => {
  apiService.setAuthToken('test-token');
  const headers = apiService.getHeaders();
  testRunner.assert(headers['Content-Type'], 'Content-Type header should exist');
  testRunner.assert(headers['Authorization'], 'Authorization header should exist');
  testRunner.assertEqual(headers['Authorization'], 'Bearer test-token', 'Authorization header should be correct');
});

testRunner.test('apiService handles interceptors', 'apiService', () => {
  let requestIntercepted = false;
  let responseIntercepted = false;
  
  apiService.addRequestInterceptor(() => {
    requestIntercepted = true;
    return { url: 'test', options: {} };
  });
  
  apiService.addResponseInterceptor(() => {
    responseIntercepted = true;
    return new Response('{}');
  });
  
  testRunner.assert(apiService.requestInterceptors.length > 0, 'Request interceptor should be added');
  testRunner.assert(apiService.responseInterceptors.length > 0, 'Response interceptor should be added');
});

// WebSocket Service Tests
testRunner.test('wsService has required methods', 'websocketService', () => {
  testRunner.assert(typeof wsService.connect === 'function', 'connect method should exist');
  testRunner.assert(typeof wsService.disconnect === 'function', 'disconnect method should exist');
  testRunner.assert(typeof wsService.send === 'function', 'send method should exist');
  testRunner.assert(typeof wsService.onMessage === 'function', 'onMessage method should exist');
});

testRunner.test('wsService generates unique connection IDs', 'websocketService', () => {
  const id1 = wsService.generateId();
  const id2 = wsService.generateId();
  testRunner.assertNotEqual(id1, id2, 'Connection IDs should be unique');
  testRunner.assert(id1.startsWith('ws_'), 'Connection ID should have correct prefix');
});

testRunner.test('wsService manages connection state', 'websocketService', () => {
  const initialConnections = wsService.getActiveConnections();
  testRunner.assert(Array.isArray(initialConnections), 'Active connections should be an array');
});

// Pose Service Tests
testRunner.test('poseService has required methods', 'poseService', () => {
  testRunner.assert(typeof poseService.getCurrentPose === 'function', 'getCurrentPose method should exist');
  testRunner.assert(typeof poseService.getZoneOccupancy === 'function', 'getZoneOccupancy method should exist');
  testRunner.assert(typeof poseService.startPoseStream === 'function', 'startPoseStream method should exist');
  testRunner.assert(typeof poseService.subscribeToPoseUpdates === 'function', 'subscribeToPoseUpdates method should exist');
});

testRunner.test('poseService subscription management', 'poseService', () => {
  let callbackCalled = false;
  const unsubscribe = poseService.subscribeToPoseUpdates(() => {
    callbackCalled = true;
  });
  
  testRunner.assert(typeof unsubscribe === 'function', 'Subscribe should return unsubscribe function');
  testRunner.assert(poseService.poseSubscribers.length > 0, 'Subscriber should be added');
  
  unsubscribe();
  testRunner.assertEqual(poseService.poseSubscribers.length, 0, 'Subscriber should be removed');
});

testRunner.test('poseService handles pose updates', 'poseService', () => {
  let receivedUpdate = null;
  
  poseService.subscribeToPoseUpdates(update => {
    receivedUpdate = update;
  });
  
  const testUpdate = { type: 'pose_update', data: { persons: [] } };
  poseService.notifyPoseSubscribers(testUpdate);
  
  testRunner.assertEqual(receivedUpdate, testUpdate, 'Update should be received by subscriber');
});

// Health Service Tests
testRunner.test('healthService has required methods', 'healthService', () => {
  testRunner.assert(typeof healthService.getSystemHealth === 'function', 'getSystemHealth method should exist');
  testRunner.assert(typeof healthService.checkReadiness === 'function', 'checkReadiness method should exist');
  testRunner.assert(typeof healthService.startHealthMonitoring === 'function', 'startHealthMonitoring method should exist');
  testRunner.assert(typeof healthService.subscribeToHealth === 'function', 'subscribeToHealth method should exist');
});

testRunner.test('healthService subscription management', 'healthService', () => {
  let callbackCalled = false;
  const unsubscribe = healthService.subscribeToHealth(() => {
    callbackCalled = true;
  });
  
  testRunner.assert(typeof unsubscribe === 'function', 'Subscribe should return unsubscribe function');
  testRunner.assert(healthService.healthSubscribers.length > 0, 'Subscriber should be added');
  
  unsubscribe();
  testRunner.assertEqual(healthService.healthSubscribers.length, 0, 'Subscriber should be removed');
});

testRunner.test('healthService status checking', 'healthService', () => {
  // Set mock health status
  healthService.lastHealthStatus = { status: 'healthy' };
  testRunner.assert(healthService.isSystemHealthy(), 'System should be healthy');
  
  healthService.lastHealthStatus = { status: 'unhealthy' };
  testRunner.assert(!healthService.isSystemHealthy(), 'System should not be healthy');
  
  healthService.lastHealthStatus = null;
  testRunner.assertEqual(healthService.isSystemHealthy(), null, 'System health should be null when no status');
});

// UI Component Tests
testRunner.test('TabManager can be instantiated', 'uiComponent', () => {
  const container = createMockContainer();
  const tabManager = new TabManager(container);
  testRunner.assert(tabManager instanceof TabManager, 'TabManager should be instantiated');
});

testRunner.test('TabManager initializes tabs', 'uiComponent', () => {
  const container = createMockContainer();
  const tabManager = new TabManager(container);
  tabManager.init();
  
  testRunner.assert(tabManager.tabs.length > 0, 'Tabs should be found');
  testRunner.assert(tabManager.tabContents.length > 0, 'Tab contents should be found');
});

testRunner.test('TabManager handles tab switching', 'uiComponent', () => {
  const container = createMockContainer();
  const tabManager = new TabManager(container);
  tabManager.init();
  
  let tabChangeEvent = null;
  tabManager.onTabChange((newTab, oldTab) => {
    tabChangeEvent = { newTab, oldTab };
  });
  
  // Switch to hardware tab
  const hardwareTab = container.querySelector('[data-tab="hardware"]');
  tabManager.switchTab(hardwareTab);
  
  testRunner.assertEqual(tabManager.getActiveTab(), 'hardware', 'Active tab should be updated');
  testRunner.assert(tabChangeEvent, 'Tab change event should be fired');
  testRunner.assertEqual(tabChangeEvent.newTab, 'hardware', 'New tab should be correct');
});

testRunner.test('TabManager can enable/disable tabs', 'uiComponent', () => {
  const container = createMockContainer();
  const tabManager = new TabManager(container);
  tabManager.init();
  
  tabManager.setTabEnabled('hardware', false);
  const hardwareTab = container.querySelector('[data-tab="hardware"]');
  testRunner.assert(hardwareTab.disabled, 'Tab should be disabled');
  testRunner.assert(hardwareTab.classList.contains('disabled'), 'Tab should have disabled class');
});

testRunner.test('TabManager can show/hide tabs', 'uiComponent', () => {
  const container = createMockContainer();
  const tabManager = new TabManager(container);
  tabManager.init();
  
  tabManager.setTabVisible('hardware', false);
  const hardwareTab = container.querySelector('[data-tab="hardware"]');
  testRunner.assertEqual(hardwareTab.style.display, 'none', 'Tab should be hidden');
});

// Integration Tests
testRunner.test('Services can be imported together', 'integration', () => {
  testRunner.assert(apiService, 'API service should be available');
  testRunner.assert(wsService, 'WebSocket service should be available');
  testRunner.assert(poseService, 'Pose service should be available');
  testRunner.assert(healthService, 'Health service should be available');
});

testRunner.test('Services maintain separate state', 'integration', () => {
  // Set different states
  apiService.setAuthToken('api-token');
  poseService.subscribeToPoseUpdates(() => {});
  healthService.subscribeToHealth(() => {});
  
  // Verify independence
  testRunner.assertEqual(apiService.authToken, 'api-token', 'API service should maintain its token');
  testRunner.assert(poseService.poseSubscribers.length > 0, 'Pose service should have subscribers');
  testRunner.assert(healthService.healthSubscribers.length > 0, 'Health service should have subscribers');
});

testRunner.test('Configuration is consistent across services', 'integration', () => {
  // All services should use the same configuration
  testRunner.assert(API_CONFIG.BASE_URL, 'Base URL should be configured');
  testRunner.assert(API_CONFIG.ENDPOINTS, 'Endpoints should be configured');
  testRunner.assert(API_CONFIG.WS_CONFIG, 'WebSocket config should be available');
});

// Event listeners for UI
document.addEventListener('DOMContentLoaded', () => {
  document.getElementById('runAllTests').addEventListener('click', () => {
    testRunner.runAllTests();
  });
  
  document.getElementById('runUnitTests').addEventListener('click', () => {
    const unitCategories = ['apiConfig', 'apiService', 'websocketService', 'poseService', 'healthService', 'uiComponent'];
    testRunner.clearResults();
    
    (async () => {
      for (const category of unitCategories) {
        await testRunner.runTestsByCategory(category);
      }
      testRunner.updateSummary();
    })();
  });
  
  document.getElementById('runIntegrationTests').addEventListener('click', () => {
    testRunner.runTestsByCategory('integration');
  });
  
  document.getElementById('clearResults').addEventListener('click', () => {
    testRunner.clearResults();
  });
  
  // Initialize test display
  testRunner.tests.forEach(test => testRunner.updateTestDisplay(test));
  testRunner.updateSummary();
});

export { testRunner };