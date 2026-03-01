/**
 * @ruvector/edge-net Monitoring and Metrics System
 *
 * Real-time monitoring for distributed compute network:
 * - System metrics collection
 * - Network health monitoring
 * - Performance tracking
 * - Alert system
 * - Metrics aggregation
 *
 * @module @ruvector/edge-net/monitor
 */

import { EventEmitter } from 'events';
import { randomBytes } from 'crypto';
import { cpus, totalmem, freemem, loadavg } from 'os';

// ============================================
// METRICS COLLECTOR
// ============================================

/**
 * Time-series metrics storage
 */
class MetricsSeries {
    constructor(options = {}) {
        this.name = options.name;
        this.maxPoints = options.maxPoints || 1000;
        this.points = [];
    }

    add(value, timestamp = Date.now()) {
        this.points.push({ value, timestamp });

        // Prune old points
        if (this.points.length > this.maxPoints) {
            this.points = this.points.slice(-this.maxPoints);
        }
    }

    latest() {
        return this.points.length > 0 ? this.points[this.points.length - 1] : null;
    }

    avg(duration = 60000) {
        const cutoff = Date.now() - duration;
        const recent = this.points.filter(p => p.timestamp >= cutoff);
        if (recent.length === 0) return 0;
        return recent.reduce((sum, p) => sum + p.value, 0) / recent.length;
    }

    min(duration = 60000) {
        const cutoff = Date.now() - duration;
        const recent = this.points.filter(p => p.timestamp >= cutoff);
        if (recent.length === 0) return 0;
        return Math.min(...recent.map(p => p.value));
    }

    max(duration = 60000) {
        const cutoff = Date.now() - duration;
        const recent = this.points.filter(p => p.timestamp >= cutoff);
        if (recent.length === 0) return 0;
        return Math.max(...recent.map(p => p.value));
    }

    rate(duration = 60000) {
        const cutoff = Date.now() - duration;
        const recent = this.points.filter(p => p.timestamp >= cutoff);
        if (recent.length < 2) return 0;

        const first = recent[0];
        const last = recent[recent.length - 1];
        const timeDiff = (last.timestamp - first.timestamp) / 1000;

        return timeDiff > 0 ? (last.value - first.value) / timeDiff : 0;
    }

    percentile(p, duration = 60000) {
        const cutoff = Date.now() - duration;
        const recent = this.points.filter(pt => pt.timestamp >= cutoff);
        if (recent.length === 0) return 0;

        const sorted = recent.map(pt => pt.value).sort((a, b) => a - b);
        const index = Math.ceil((p / 100) * sorted.length) - 1;
        return sorted[Math.max(0, index)];
    }

    toJSON() {
        return {
            name: this.name,
            count: this.points.length,
            latest: this.latest(),
            avg: this.avg(),
            min: this.min(),
            max: this.max(),
        };
    }
}

/**
 * Counter metric (monotonically increasing)
 */
class Counter {
    constructor(name) {
        this.name = name;
        this.value = 0;
        this.lastReset = Date.now();
    }

    inc(amount = 1) {
        this.value += amount;
    }

    get() {
        return this.value;
    }

    reset() {
        this.value = 0;
        this.lastReset = Date.now();
    }

    toJSON() {
        return {
            name: this.name,
            value: this.value,
            lastReset: this.lastReset,
        };
    }
}

/**
 * Gauge metric (can go up and down)
 */
class Gauge {
    constructor(name) {
        this.name = name;
        this.value = 0;
    }

    set(value) {
        this.value = value;
    }

    inc(amount = 1) {
        this.value += amount;
    }

    dec(amount = 1) {
        this.value -= amount;
    }

    get() {
        return this.value;
    }

    toJSON() {
        return {
            name: this.name,
            value: this.value,
        };
    }
}

/**
 * Histogram metric
 */
class Histogram {
    constructor(name, buckets = [5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000]) {
        this.name = name;
        this.buckets = buckets.sort((a, b) => a - b);
        this.counts = new Map(buckets.map(b => [b, 0]));
        this.counts.set(Infinity, 0);
        this.sum = 0;
        this.count = 0;
    }

    observe(value) {
        this.sum += value;
        this.count++;

        for (const bucket of this.buckets) {
            if (value <= bucket) {
                this.counts.set(bucket, this.counts.get(bucket) + 1);
            }
        }
        this.counts.set(Infinity, this.counts.get(Infinity) + 1);
    }

    avg() {
        return this.count > 0 ? this.sum / this.count : 0;
    }

    toJSON() {
        return {
            name: this.name,
            count: this.count,
            sum: this.sum,
            avg: this.avg(),
            buckets: Object.fromEntries(this.counts),
        };
    }
}

// ============================================
// SYSTEM MONITOR
// ============================================

/**
 * System resource monitor
 */
export class SystemMonitor extends EventEmitter {
    constructor(options = {}) {
        super();
        this.interval = options.interval || 5000;
        this.timer = null;

        // Metrics
        this.cpu = new MetricsSeries({ name: 'cpu_usage' });
        this.memory = new MetricsSeries({ name: 'memory_usage' });
        this.load = new MetricsSeries({ name: 'load_avg' });
    }

    start() {
        this.collect();
        this.timer = setInterval(() => this.collect(), this.interval);
    }

    stop() {
        if (this.timer) {
            clearInterval(this.timer);
            this.timer = null;
        }
    }

    collect() {
        // CPU usage (simplified - percentage of load vs cores)
        const load = loadavg()[0];
        const cores = cpus().length;
        const cpuUsage = Math.min(100, (load / cores) * 100);
        this.cpu.add(cpuUsage);

        // Memory usage
        const total = totalmem();
        const free = freemem();
        const memUsage = ((total - free) / total) * 100;
        this.memory.add(memUsage);

        // Load average
        this.load.add(load);

        this.emit('metrics', this.getMetrics());
    }

    getMetrics() {
        return {
            timestamp: Date.now(),
            cpu: {
                usage: this.cpu.latest()?.value || 0,
                avg1m: this.cpu.avg(60000),
                avg5m: this.cpu.avg(300000),
            },
            memory: {
                usage: this.memory.latest()?.value || 0,
                total: totalmem(),
                free: freemem(),
            },
            load: {
                current: this.load.latest()?.value || 0,
                avg: loadavg(),
            },
            cores: cpus().length,
        };
    }
}

// ============================================
// NETWORK MONITOR
// ============================================

/**
 * Network health and performance monitor
 */
export class NetworkMonitor extends EventEmitter {
    constructor(options = {}) {
        super();
        this.nodeId = options.nodeId;
        this.checkInterval = options.checkInterval || 30000;
        this.timer = null;

        // Metrics
        this.peers = new Gauge('connected_peers');
        this.messages = new Counter('messages_total');
        this.errors = new Counter('errors_total');
        this.latency = new Histogram('peer_latency_ms');

        // Series
        this.bandwidth = new MetricsSeries({ name: 'bandwidth_bps' });
        this.peerCount = new MetricsSeries({ name: 'peer_count' });

        // Peer tracking
        this.peerLatencies = new Map(); // peerId -> latency ms
        this.peerStatus = new Map();    // peerId -> { status, lastSeen }
    }

    start() {
        this.timer = setInterval(() => this.check(), this.checkInterval);
    }

    stop() {
        if (this.timer) {
            clearInterval(this.timer);
            this.timer = null;
        }
    }

    /**
     * Record peer connection
     */
    peerConnected(peerId) {
        this.peers.inc();
        this.peerStatus.set(peerId, { status: 'connected', lastSeen: Date.now() });
        this.peerCount.add(this.peers.get());
        this.emit('peer-connected', { peerId });
    }

    /**
     * Record peer disconnection
     */
    peerDisconnected(peerId) {
        this.peers.dec();
        this.peerStatus.set(peerId, { status: 'disconnected', lastSeen: Date.now() });
        this.peerCount.add(this.peers.get());
        this.emit('peer-disconnected', { peerId });
    }

    /**
     * Record message
     */
    recordMessage(peerId, bytes) {
        this.messages.inc();
        this.bandwidth.add(bytes);

        if (peerId && this.peerStatus.has(peerId)) {
            this.peerStatus.get(peerId).lastSeen = Date.now();
        }
    }

    /**
     * Record latency measurement
     */
    recordLatency(peerId, latencyMs) {
        this.latency.observe(latencyMs);
        this.peerLatencies.set(peerId, latencyMs);
    }

    /**
     * Record error
     */
    recordError(type) {
        this.errors.inc();
        this.emit('error', { type });
    }

    /**
     * Periodic health check
     */
    check() {
        const metrics = this.getMetrics();

        // Check for issues
        if (metrics.peers.current === 0) {
            this.emit('alert', { type: 'no_peers', message: 'No connected peers' });
        }

        if (metrics.latency.avg > 1000) {
            this.emit('alert', { type: 'high_latency', message: 'High network latency', value: metrics.latency.avg });
        }

        this.emit('health-check', metrics);
    }

    getMetrics() {
        return {
            timestamp: Date.now(),
            peers: {
                current: this.peers.get(),
                avg1h: this.peerCount.avg(3600000),
            },
            messages: this.messages.get(),
            errors: this.errors.get(),
            latency: {
                avg: this.latency.avg(),
                p50: this.latency.toJSON().buckets[50] || 0,
                p99: this.latency.toJSON().buckets[1000] || 0,
            },
            bandwidth: {
                current: this.bandwidth.rate(),
                avg1m: this.bandwidth.avg(60000),
            },
        };
    }
}

// ============================================
// TASK MONITOR
// ============================================

/**
 * Task execution monitor
 */
export class TaskMonitor extends EventEmitter {
    constructor(options = {}) {
        super();

        // Counters
        this.submitted = new Counter('tasks_submitted');
        this.completed = new Counter('tasks_completed');
        this.failed = new Counter('tasks_failed');
        this.retried = new Counter('tasks_retried');

        // Gauges
        this.pending = new Gauge('tasks_pending');
        this.running = new Gauge('tasks_running');

        // Histograms
        this.waitTime = new Histogram('task_wait_time_ms');
        this.execTime = new Histogram('task_exec_time_ms');

        // Series
        this.throughput = new MetricsSeries({ name: 'tasks_per_second' });
    }

    taskSubmitted() {
        this.submitted.inc();
        this.pending.inc();
    }

    taskStarted() {
        this.pending.dec();
        this.running.inc();
    }

    taskCompleted(waitTimeMs, execTimeMs) {
        this.running.dec();
        this.completed.inc();
        this.waitTime.observe(waitTimeMs);
        this.execTime.observe(execTimeMs);
        this.throughput.add(1);
    }

    taskFailed() {
        this.running.dec();
        this.failed.inc();
    }

    taskRetried() {
        this.retried.inc();
    }

    getMetrics() {
        const total = this.completed.get() + this.failed.get();
        const successRate = total > 0 ? this.completed.get() / total : 1;

        return {
            timestamp: Date.now(),
            submitted: this.submitted.get(),
            completed: this.completed.get(),
            failed: this.failed.get(),
            retried: this.retried.get(),
            pending: this.pending.get(),
            running: this.running.get(),
            successRate,
            waitTime: {
                avg: this.waitTime.avg(),
                p50: this.waitTime.toJSON().buckets[100] || 0,
                p99: this.waitTime.toJSON().buckets[5000] || 0,
            },
            execTime: {
                avg: this.execTime.avg(),
                p50: this.execTime.toJSON().buckets[500] || 0,
                p99: this.execTime.toJSON().buckets[10000] || 0,
            },
            throughput: this.throughput.rate(60000),
        };
    }
}

// ============================================
// MONITORING DASHBOARD
// ============================================

/**
 * Unified monitoring dashboard
 */
export class Monitor extends EventEmitter {
    constructor(options = {}) {
        super();
        this.nodeId = options.nodeId || `monitor-${randomBytes(8).toString('hex')}`;

        // Sub-monitors
        this.system = new SystemMonitor(options.system);
        this.network = new NetworkMonitor({ ...options.network, nodeId: this.nodeId });
        this.tasks = new TaskMonitor(options.tasks);

        // Alert thresholds
        this.thresholds = {
            cpuHigh: options.cpuHigh || 90,
            memoryHigh: options.memoryHigh || 90,
            latencyHigh: options.latencyHigh || 1000,
            errorRateHigh: options.errorRateHigh || 0.1,
            ...options.thresholds,
        };

        // Alert state
        this.alerts = new Map();
        this.alertHistory = [];

        // Reporting
        this.reportInterval = options.reportInterval || 60000;
        this.reportTimer = null;

        // Forward events
        this.system.on('metrics', m => this.emit('system-metrics', m));
        this.network.on('health-check', m => this.emit('network-metrics', m));
        this.network.on('alert', a => this.handleAlert(a));
    }

    /**
     * Start all monitors
     */
    start() {
        this.system.start();
        this.network.start();

        this.reportTimer = setInterval(() => {
            this.generateReport();
        }, this.reportInterval);

        this.emit('started');
    }

    /**
     * Stop all monitors
     */
    stop() {
        this.system.stop();
        this.network.stop();

        if (this.reportTimer) {
            clearInterval(this.reportTimer);
            this.reportTimer = null;
        }

        this.emit('stopped');
    }

    /**
     * Handle alert
     */
    handleAlert(alert) {
        const key = `${alert.type}`;
        const existing = this.alerts.get(key);

        if (existing) {
            existing.count++;
            existing.lastSeen = Date.now();
        } else {
            const newAlert = {
                ...alert,
                id: `alert-${randomBytes(4).toString('hex')}`,
                count: 1,
                firstSeen: Date.now(),
                lastSeen: Date.now(),
            };
            this.alerts.set(key, newAlert);
            this.alertHistory.push(newAlert);
        }

        this.emit('alert', alert);
    }

    /**
     * Clear alert
     */
    clearAlert(type) {
        this.alerts.delete(type);
        this.emit('alert-cleared', { type });
    }

    /**
     * Generate comprehensive report
     */
    generateReport() {
        const report = {
            timestamp: Date.now(),
            nodeId: this.nodeId,
            system: this.system.getMetrics(),
            network: this.network.getMetrics(),
            tasks: this.tasks.getMetrics(),
            alerts: Array.from(this.alerts.values()),
            health: this.calculateHealth(),
        };

        this.emit('report', report);
        return report;
    }

    /**
     * Calculate overall health score (0-100)
     */
    calculateHealth() {
        let score = 100;
        const issues = [];

        // System health
        const sysMetrics = this.system.getMetrics();
        if (sysMetrics.cpu.usage > this.thresholds.cpuHigh) {
            score -= 20;
            issues.push('high_cpu');
        }
        if (sysMetrics.memory.usage > this.thresholds.memoryHigh) {
            score -= 20;
            issues.push('high_memory');
        }

        // Network health
        const netMetrics = this.network.getMetrics();
        if (netMetrics.peers.current === 0) {
            score -= 30;
            issues.push('no_peers');
        }
        if (netMetrics.latency.avg > this.thresholds.latencyHigh) {
            score -= 15;
            issues.push('high_latency');
        }

        // Task health
        const taskMetrics = this.tasks.getMetrics();
        if (taskMetrics.successRate < (1 - this.thresholds.errorRateHigh)) {
            score -= 15;
            issues.push('high_error_rate');
        }

        return {
            score: Math.max(0, score),
            status: score >= 80 ? 'healthy' : score >= 50 ? 'degraded' : 'unhealthy',
            issues,
        };
    }

    /**
     * Get current metrics summary
     */
    getMetrics() {
        return {
            system: this.system.getMetrics(),
            network: this.network.getMetrics(),
            tasks: this.tasks.getMetrics(),
        };
    }

    /**
     * Get active alerts
     */
    getAlerts() {
        return Array.from(this.alerts.values());
    }
}

// ============================================
// EXPORTS
// ============================================

export default Monitor;
