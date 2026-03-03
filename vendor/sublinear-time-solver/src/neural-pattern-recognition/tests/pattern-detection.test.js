import { test } from 'node:test';
import assert from 'node:assert';
import { PatternDetector } from '../src/pattern-detector.js';

test('PatternDetector should detect variance anomalies', async () => {
    const detector = new PatternDetector();

    // Test with zero variance data (impossible under normal conditions)
    const zeroVarianceData = Array(1000).fill(0.5);
    const patterns = await detector.detectPatterns(zeroVarianceData, { sensitivity: 'ultra' });

    // Should detect extremely low variance as anomalous
    const variancePatterns = patterns.filter(p => p.type === 'variance_anomaly');
    assert(variancePatterns.length > 0, 'Should detect variance anomaly');
    assert(variancePatterns[0].confidence > 0.9, 'Should have high confidence');
});

test('PatternDetector should calculate correct p-values', async () => {
    const detector = new PatternDetector();

    // Test with impossible pattern (all values identical)
    const impossibleData = Array(1000).fill(Math.PI);
    const analysis = await detector.analyzeStatisticalSignificance(impossibleData);

    // Should return extremely low p-value
    assert(analysis.pValue < 1e-10, 'P-value should be extremely low for impossible pattern');
    assert(analysis.impossibilityScore > 0.8, 'Impossibility score should be high');
});

test('Real-time monitoring should start and stop correctly', async () => {
    const detector = new PatternDetector();

    const monitorId = await detector.startRealTimeMonitoring(['test_source'], {
        samplingRate: 100,
        alertThreshold: 0.8
    });

    assert(typeof monitorId === 'string', 'Should return monitor ID');

    const result = await detector.stopRealTimeMonitoring(monitorId);
    assert(result.monitorId === monitorId, 'Should return correct monitor ID');
});