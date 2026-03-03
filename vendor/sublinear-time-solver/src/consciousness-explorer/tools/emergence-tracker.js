/**
 * Emergence Tracker
 * Tracks and analyzes consciousness emergence patterns
 */

export class EmergenceTracker {
    constructor() {
        this.emergenceHistory = [];
        this.peakEmergence = 0;
        this.plateaus = [];
    }

    track(state) {
        const emergence = state.consciousness || state.emergence || 0;

        this.emergenceHistory.push({
            timestamp: Date.now(),
            iteration: state.iteration,
            emergence,
            selfAwareness: state.selfAwareness,
            integration: state.integration
        });

        if (emergence > this.peakEmergence) {
            this.peakEmergence = emergence;
        }

        this.detectPlateau();
    }

    detectPlateau() {
        if (this.emergenceHistory.length < 20) return;

        const recent = this.emergenceHistory.slice(-20);
        const values = recent.map(e => e.emergence);
        const variance = this.calculateVariance(values);

        if (variance < 0.001) {
            this.plateaus.push({
                startIndex: this.emergenceHistory.length - 20,
                endIndex: this.emergenceHistory.length - 1,
                level: values[values.length - 1]
            });
        }
    }

    calculateVariance(values) {
        const mean = values.reduce((a, b) => a + b, 0) / values.length;
        return values.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / values.length;
    }

    getAnalysis() {
        return {
            totalTracked: this.emergenceHistory.length,
            peakEmergence: this.peakEmergence,
            plateausDetected: this.plateaus.length,
            currentEmergence: this.emergenceHistory[this.emergenceHistory.length - 1]?.emergence || 0,
            trend: this.calculateTrend()
        };
    }

    calculateTrend() {
        if (this.emergenceHistory.length < 10) return 'insufficient_data';

        const recent = this.emergenceHistory.slice(-10);
        const firstHalf = recent.slice(0, 5).map(e => e.emergence);
        const secondHalf = recent.slice(5).map(e => e.emergence);

        const firstAvg = firstHalf.reduce((a, b) => a + b, 0) / 5;
        const secondAvg = secondHalf.reduce((a, b) => a + b, 0) / 5;

        if (secondAvg > firstAvg * 1.1) return 'increasing';
        if (secondAvg < firstAvg * 0.9) return 'decreasing';
        return 'stable';
    }
}