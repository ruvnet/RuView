import * as wasm from 'lean-agentic-wasm';

// Global state
let wsClient = null;
let sseClient = null;
let httpClient = null;
let agenticClient = null;
let messageCount = 0;
let startTime = Date.now();

// Initialize WASM and agentic client
async function init() {
    try {
        agenticClient = new wasm.LeanAgenticClient('demo-session', null);
        log('ws', 'WASM module loaded successfully', 'success');
        updateStats();
    } catch (err) {
        log('ws', `Failed to initialize: ${err}`, 'error');
    }
}

// Tab switching
window.switchTab = function(tab) {
    const tabs = document.querySelectorAll('.tab');
    const contents = document.querySelectorAll('.tab-content');

    tabs.forEach(t => t.classList.remove('active'));
    contents.forEach(c => c.classList.remove('active'));

    document.querySelector(`.tab:nth-child(${getTabIndex(tab)})`).classList.add('active');
    document.getElementById(`${tab}-tab`).classList.add('active');
};

function getTabIndex(tab) {
    const tabs = ['websocket', 'sse', 'http', 'benchmark'];
    return tabs.indexOf(tab) + 1;
}

// WebSocket functions
window.connectWebSocket = async function() {
    const url = document.getElementById('ws-url').value;

    try {
        wsClient = new wasm.WebSocketClient(url);

        wsClient.set_on_message((data) => {
            const start = performance.now();
            const result = agenticClient.process_message(data);
            const latency = performance.now() - start;

            log('ws', `Received: ${data} | Latency: ${latency.toFixed(2)}ms`, 'success');
            messageCount++;
            updateStats();
        });

        wsClient.set_on_error((error) => {
            log('ws', `Error: ${error}`, 'error');
        });

        wsClient.set_on_close((code) => {
            log('ws', `Connection closed: ${code}`, 'error');
            document.getElementById('status').textContent = 'Disconnected';
        });

        // Wait for connection
        await new Promise(resolve => setTimeout(resolve, 100));

        if (wsClient.ready_state() === 1) {
            log('ws', `Connected to ${url}`, 'success');
            document.getElementById('status').textContent = 'Connected (WS)';
        }
    } catch (err) {
        log('ws', `Connection failed: ${err}`, 'error');
    }
};

window.disconnectWebSocket = function() {
    if (wsClient) {
        wsClient.close();
        wsClient = null;
        log('ws', 'Disconnected', 'success');
        document.getElementById('status').textContent = 'Disconnected';
    }
};

window.sendWebSocketMessage = function() {
    const message = document.getElementById('ws-message').value;

    if (wsClient && wsClient.ready_state() === 1) {
        const start = performance.now();
        wsClient.send(message);
        const latency = performance.now() - start;

        log('ws', `Sent: ${message} | Send latency: ${latency.toFixed(3)}ms`, 'success');
        document.getElementById('ws-message').value = '';
    } else {
        log('ws', 'Not connected', 'error');
    }
};

window.startWebSocketBurst = async function() {
    if (!wsClient || wsClient.ready_state() !== 1) {
        log('ws', 'Not connected', 'error');
        return;
    }

    const count = 1000;
    const start = performance.now();

    for (let i = 0; i < count; i++) {
        wsClient.send(`Message ${i}`);
    }

    const duration = performance.now() - start;
    const throughput = (count / duration) * 1000;

    log('ws', `Burst test: ${count} messages in ${duration.toFixed(2)}ms (${throughput.toFixed(0)} msg/s)`, 'success');
};

// SSE functions
window.connectSSE = function() {
    const url = document.getElementById('sse-url').value;

    try {
        sseClient = new wasm.SSEClient(url);

        sseClient.set_on_message((data) => {
            const start = performance.now();
            const result = agenticClient.process_message(data);
            const latency = performance.now() - start;

            log('sse', `Received: ${data} | Latency: ${latency.toFixed(2)}ms`, 'success');
            messageCount++;
            updateStats();
        });

        log('sse', `Connected to ${url}`, 'success');
        document.getElementById('status').textContent = 'Connected (SSE)';
    } catch (err) {
        log('sse', `Connection failed: ${err}`, 'error');
    }
};

window.disconnectSSE = function() {
    if (sseClient) {
        sseClient.close();
        sseClient = null;
        log('sse', 'Disconnected', 'success');
        document.getElementById('status').textContent = 'Disconnected';
    }
};

// HTTP Streaming
window.startHTTPStream = async function() {
    const url = document.getElementById('http-url').value;

    try {
        httpClient = new wasm.StreamingHTTPClient(url);

        log('http', `Starting stream from ${url}...`, 'success');

        await httpClient.stream((data) => {
            const start = performance.now();
            const result = agenticClient.process_message(data);
            const latency = performance.now() - start;

            log('http', `Chunk: ${data.substring(0, 50)}... | Latency: ${latency.toFixed(2)}ms`, 'success');
            messageCount++;
            updateStats();
        });

        log('http', 'Stream completed', 'success');
    } catch (err) {
        log('http', `Stream error: ${err}`, 'error');
    }
};

// Benchmark functions
window.runLatencyBenchmark = function() {
    log('benchmark', 'Running latency benchmark...', 'success');

    const iterations = 10000;
    const latencies = [];

    for (let i = 0; i < iterations; i++) {
        const start = performance.now();
        agenticClient.process_message(`Test message ${i}`);
        const latency = performance.now() - start;
        latencies.push(latency);
    }

    const avg = latencies.reduce((a, b) => a + b, 0) / iterations;
    const sorted = latencies.sort((a, b) => a - b);
    const p50 = sorted[Math.floor(iterations * 0.5)];
    const p95 = sorted[Math.floor(iterations * 0.95)];
    const p99 = sorted[Math.floor(iterations * 0.99)];
    const min = sorted[0];
    const max = sorted[iterations - 1];

    log('benchmark', `Latency Statistics (${iterations} iterations):`, 'success');
    log('benchmark', `  Min:    ${min.toFixed(3)}ms`, 'success');
    log('benchmark', `  P50:    ${p50.toFixed(3)}ms`, 'success');
    log('benchmark', `  P95:    ${p95.toFixed(3)}ms`, 'success');
    log('benchmark', `  P99:    ${p99.toFixed(3)}ms`, 'success');
    log('benchmark', `  Max:    ${max.toFixed(3)}ms`, 'success');
    log('benchmark', `  Avg:    ${avg.toFixed(3)}ms`, 'success');

    updateLatencyMeter(avg, p99);
};

window.runThroughputBenchmark = function() {
    log('benchmark', 'Running throughput benchmark...', 'success');

    const duration = 5000; // 5 seconds
    const start = performance.now();
    let count = 0;

    while (performance.now() - start < duration) {
        agenticClient.process_message(`Throughput test ${count}`);
        count++;
    }

    const actualDuration = performance.now() - start;
    const throughput = (count / actualDuration) * 1000;

    log('benchmark', `Throughput: ${throughput.toFixed(0)} messages/second`, 'success');
    log('benchmark', `Total processed: ${count} messages in ${actualDuration.toFixed(0)}ms`, 'success');

    document.getElementById('throughput').textContent = `${throughput.toFixed(0)}/s`;
};

window.runConcurrentBenchmark = async function() {
    log('benchmark', 'Running concurrent sessions benchmark...', 'success');

    const sessions = 100;
    const messagesPerSession = 10;
    const clients = [];

    for (let i = 0; i < sessions; i++) {
        clients.push(new wasm.LeanAgenticClient(`session-${i}`, null));
    }

    const start = performance.now();

    // Process messages concurrently
    const promises = clients.map((client, i) => {
        return new Promise(resolve => {
            for (let j = 0; j < messagesPerSession; j++) {
                client.process_message(`Session ${i} message ${j}`);
            }
            resolve();
        });
    });

    await Promise.all(promises);

    const duration = performance.now() - start;
    const totalMessages = sessions * messagesPerSession;
    const throughput = (totalMessages / duration) * 1000;

    log('benchmark', `Concurrent test: ${sessions} sessions`, 'success');
    log('benchmark', `Total messages: ${totalMessages}`, 'success');
    log('benchmark', `Duration: ${duration.toFixed(2)}ms`, 'success');
    log('benchmark', `Throughput: ${throughput.toFixed(0)} msg/s`, 'success');
    log('benchmark', `Avg per session: ${(duration / sessions).toFixed(2)}ms`, 'success');
};

// Utility functions
function log(tab, message, type = 'info') {
    const logDiv = document.getElementById(`${tab}-log`);
    const timestamp = new Date().toISOString().split('T')[1].split('.')[0];
    const entry = document.createElement('div');
    entry.className = `log-entry ${type}`;
    entry.textContent = `[${timestamp}] ${message}`;
    logDiv.appendChild(entry);
    logDiv.scrollTop = logDiv.scrollHeight;
}

function updateStats() {
    if (agenticClient) {
        const avgLatency = agenticClient.get_avg_latency_ms();
        const msgCount = agenticClient.get_message_count();
        const elapsed = (Date.now() - startTime) / 1000;
        const throughput = msgCount / elapsed;

        document.getElementById('avg-latency').textContent = `${avgLatency.toFixed(2)}ms`;
        document.getElementById('msg-count').textContent = msgCount;
        document.getElementById('throughput').textContent = `${throughput.toFixed(0)}/s`;
    }
}

function updateLatencyMeter(avg, p99) {
    const meter = document.getElementById('latency-bar');
    const maxLatency = 10; // 10ms max for visualization
    const percentage = Math.min((avg / maxLatency) * 100, 100);
    meter.style.height = `${percentage}%`;

    if (avg < 1) {
        meter.style.background = 'linear-gradient(180deg, #2ecc71 0%, #27ae60 100%)';
    } else if (avg < 5) {
        meter.style.background = 'linear-gradient(180deg, #f39c12 0%, #e67e22 100%)';
    } else {
        meter.style.background = 'linear-gradient(180deg, #e74c3c 0%, #c0392b 100%)';
    }
}

// Auto-update stats
setInterval(updateStats, 1000);

// Initialize on load
init();
