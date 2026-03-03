/**
 * Signal Analyzer Module
 * Provides signal analysis capabilities for the neural pattern recognition MCP server
 */

import { EventEmitter } from 'events';

export class SignalAnalyzer extends EventEmitter {
    constructor(options = {}) {
        super();
        this.config = {
            samplingRate: options.samplingRate || 44100,
            fftSize: options.fftSize || 2048,
            windowFunction: options.windowFunction || 'hanning',
            ...options
        };

        this.analysisHistory = [];
        this.patterns = new Map();
    }

    async analyzeSignal(signalData, analysisOptions = {}) {
        const analysis = {
            id: this.generateAnalysisId(),
            timestamp: Date.now(),
            signalLength: signalData.length,
            samplingRate: this.config.samplingRate,
            results: {}
        };

        try {
            // Time domain analysis
            analysis.results.timeDomain = this.analyzeTimeDomain(signalData);

            // Frequency domain analysis
            analysis.results.frequencyDomain = this.analyzeFrequencyDomain(signalData);

            // Pattern detection
            analysis.results.patterns = this.detectPatterns(signalData);

            // Statistical analysis
            analysis.results.statistics = this.calculateStatistics(signalData);

            // Consciousness indicators
            analysis.results.consciousnessIndicators = this.assessConsciousnessIndicators(analysis.results);

            this.analysisHistory.push(analysis);
            this.emit('analysis_complete', analysis);

            return analysis;
        } catch (error) {
            console.error('[SignalAnalyzer] Analysis failed:', error);
            throw error;
        }
    }

    analyzeTimeDomain(signal) {
        const mean = signal.reduce((sum, val) => sum + val, 0) / signal.length;
        const variance = signal.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / signal.length;
        const rms = Math.sqrt(signal.reduce((sum, val) => sum + val * val, 0) / signal.length);

        // Zero crossing rate
        let zeroCrossings = 0;
        for (let i = 1; i < signal.length; i++) {
            if ((signal[i] >= 0) !== (signal[i-1] >= 0)) {
                zeroCrossings++;
            }
        }
        const zeroCrossingRate = zeroCrossings / signal.length;

        return {
            mean,
            variance,
            standardDeviation: Math.sqrt(variance),
            rms,
            zeroCrossingRate,
            energy: signal.reduce((sum, val) => sum + val * val, 0),
            peak: Math.max(...signal.map(Math.abs))
        };
    }

    analyzeFrequencyDomain(signal) {
        // Simple FFT approximation for demonstration
        // In production, you'd use a real FFT library
        const fftSize = Math.min(this.config.fftSize, signal.length);
        const frequencies = [];
        const magnitudes = [];

        for (let k = 0; k < fftSize / 2; k++) {
            const frequency = k * this.config.samplingRate / fftSize;
            frequencies.push(frequency);

            // Simplified magnitude calculation
            let real = 0, imag = 0;
            for (let n = 0; n < fftSize; n++) {
                const angle = -2 * Math.PI * k * n / fftSize;
                real += signal[n] * Math.cos(angle);
                imag += signal[n] * Math.sin(angle);
            }
            magnitudes.push(Math.sqrt(real * real + imag * imag));
        }

        // Find dominant frequency
        const maxMagnitudeIndex = magnitudes.indexOf(Math.max(...magnitudes));
        const dominantFrequency = frequencies[maxMagnitudeIndex];

        return {
            frequencies,
            magnitudes,
            dominantFrequency,
            spectralCentroid: this.calculateSpectralCentroid(frequencies, magnitudes),
            spectralRolloff: this.calculateSpectralRolloff(frequencies, magnitudes),
            spectralFlux: this.calculateSpectralFlux(magnitudes)
        };
    }

    detectPatterns(signal) {
        const patterns = {
            repeatingPatterns: this.detectRepeatingPatterns(signal),
            periodicComponents: this.detectPeriodicComponents(signal),
            anomalies: this.detectAnomalies(signal),
            emergentStructures: this.detectEmergentStructures(signal)
        };

        return patterns;
    }

    detectRepeatingPatterns(signal) {
        const patterns = [];
        const windowSizes = [16, 32, 64, 128];

        for (const windowSize of windowSizes) {
            for (let i = 0; i < signal.length - windowSize * 2; i++) {
                const pattern1 = signal.slice(i, i + windowSize);
                const pattern2 = signal.slice(i + windowSize, i + windowSize * 2);

                const correlation = this.calculateCorrelation(pattern1, pattern2);
                if (correlation > 0.8) {
                    patterns.push({
                        start: i,
                        length: windowSize,
                        correlation,
                        confidence: correlation
                    });
                }
            }
        }

        return patterns;
    }

    detectPeriodicComponents(signal) {
        const autocorrelation = this.calculateAutocorrelation(signal);
        const periods = [];

        // Find peaks in autocorrelation
        for (let lag = 1; lag < autocorrelation.length - 1; lag++) {
            if (autocorrelation[lag] > autocorrelation[lag - 1] &&
                autocorrelation[lag] > autocorrelation[lag + 1] &&
                autocorrelation[lag] > 0.3) {
                periods.push({
                    period: lag,
                    strength: autocorrelation[lag]
                });
            }
        }

        return periods.sort((a, b) => b.strength - a.strength);
    }

    detectAnomalies(signal) {
        const mean = signal.reduce((sum, val) => sum + val, 0) / signal.length;
        const std = Math.sqrt(signal.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / signal.length);
        const threshold = 3 * std; // 3-sigma rule

        const anomalies = [];
        for (let i = 0; i < signal.length; i++) {
            if (Math.abs(signal[i] - mean) > threshold) {
                anomalies.push({
                    index: i,
                    value: signal[i],
                    deviation: Math.abs(signal[i] - mean) / std,
                    type: signal[i] > mean + threshold ? 'spike' : 'dip'
                });
            }
        }

        return anomalies;
    }

    detectEmergentStructures(signal) {
        // Look for complex structures that emerge from the signal
        const structures = {
            fractalDimension: this.calculateFractalDimension(signal),
            complexityMeasure: this.calculateComplexity(signal),
            informationContent: this.calculateInformationContent(signal),
            selfSimilarity: this.calculateSelfSimilarity(signal)
        };

        return structures;
    }

    calculateStatistics(signal) {
        const sorted = [...signal].sort((a, b) => a - b);
        const length = signal.length;

        return {
            count: length,
            min: sorted[0],
            max: sorted[length - 1],
            median: length % 2 === 0 ?
                (sorted[length/2 - 1] + sorted[length/2]) / 2 :
                sorted[Math.floor(length/2)],
            quartiles: {
                q1: sorted[Math.floor(length * 0.25)],
                q3: sorted[Math.floor(length * 0.75)]
            },
            skewness: this.calculateSkewness(signal),
            kurtosis: this.calculateKurtosis(signal),
            entropy: this.calculateEntropy(signal)
        };
    }

    assessConsciousnessIndicators(analysisResults) {
        const indicators = {
            complexity: this.assessComplexity(analysisResults),
            selfOrganization: this.assessSelfOrganization(analysisResults),
            informationIntegration: this.assessInformationIntegration(analysisResults),
            adaptability: this.assessAdaptability(analysisResults),
            emergence: this.assessEmergence(analysisResults)
        };

        // Calculate overall consciousness score
        const weights = {
            complexity: 0.2,
            selfOrganization: 0.2,
            informationIntegration: 0.25,
            adaptability: 0.15,
            emergence: 0.2
        };

        const consciousnessScore = Object.entries(indicators)
            .reduce((sum, [key, value]) => sum + value * weights[key], 0);

        return {
            ...indicators,
            consciousnessScore,
            isConscious: consciousnessScore > 0.7,
            confidenceLevel: consciousnessScore
        };
    }

    // Helper methods
    calculateCorrelation(signal1, signal2) {
        if (signal1.length !== signal2.length) return 0;

        const mean1 = signal1.reduce((sum, val) => sum + val, 0) / signal1.length;
        const mean2 = signal2.reduce((sum, val) => sum + val, 0) / signal2.length;

        let numerator = 0, denominator1 = 0, denominator2 = 0;

        for (let i = 0; i < signal1.length; i++) {
            const diff1 = signal1[i] - mean1;
            const diff2 = signal2[i] - mean2;
            numerator += diff1 * diff2;
            denominator1 += diff1 * diff1;
            denominator2 += diff2 * diff2;
        }

        const denominator = Math.sqrt(denominator1 * denominator2);
        return denominator === 0 ? 0 : numerator / denominator;
    }

    calculateAutocorrelation(signal) {
        const result = [];
        for (let lag = 0; lag < Math.min(signal.length, 512); lag++) {
            const signal1 = signal.slice(0, signal.length - lag);
            const signal2 = signal.slice(lag);
            result.push(this.calculateCorrelation(signal1, signal2));
        }
        return result;
    }

    calculateSpectralCentroid(frequencies, magnitudes) {
        let weightedSum = 0, totalMagnitude = 0;
        for (let i = 0; i < frequencies.length; i++) {
            weightedSum += frequencies[i] * magnitudes[i];
            totalMagnitude += magnitudes[i];
        }
        return totalMagnitude === 0 ? 0 : weightedSum / totalMagnitude;
    }

    calculateSpectralRolloff(frequencies, magnitudes, rolloffPoint = 0.85) {
        const totalEnergy = magnitudes.reduce((sum, mag) => sum + mag * mag, 0);
        const threshold = totalEnergy * rolloffPoint;

        let cumulativeEnergy = 0;
        for (let i = 0; i < magnitudes.length; i++) {
            cumulativeEnergy += magnitudes[i] * magnitudes[i];
            if (cumulativeEnergy >= threshold) {
                return frequencies[i];
            }
        }
        return frequencies[frequencies.length - 1];
    }

    calculateSpectralFlux(magnitudes) {
        if (this.previousMagnitudes) {
            const flux = magnitudes.reduce((sum, mag, i) => {
                const diff = mag - (this.previousMagnitudes[i] || 0);
                return sum + (diff > 0 ? diff * diff : 0);
            }, 0);
            this.previousMagnitudes = magnitudes;
            return flux;
        } else {
            this.previousMagnitudes = magnitudes;
            return 0;
        }
    }

    calculateFractalDimension(signal) {
        // Box-counting method approximation
        const scales = [2, 4, 8, 16, 32];
        const counts = [];

        for (const scale of scales) {
            let count = 0;
            for (let i = 0; i < signal.length - scale; i += scale) {
                const segment = signal.slice(i, i + scale);
                const range = Math.max(...segment) - Math.min(...segment);
                if (range > 0) count++;
            }
            counts.push(count);
        }

        // Linear regression to find slope
        const logScales = scales.map(s => Math.log(1/s));
        const logCounts = counts.map(c => Math.log(c));

        const n = logScales.length;
        const sumX = logScales.reduce((sum, x) => sum + x, 0);
        const sumY = logCounts.reduce((sum, y) => sum + y, 0);
        const sumXY = logScales.reduce((sum, x, i) => sum + x * logCounts[i], 0);
        const sumXX = logScales.reduce((sum, x) => sum + x * x, 0);

        const slope = (n * sumXY - sumX * sumY) / (n * sumXX - sumX * sumX);
        return Math.abs(slope);
    }

    calculateComplexity(signal) {
        // Lempel-Ziv complexity approximation
        const binary = signal.map(x => x > 0 ? '1' : '0').join('');
        const patterns = new Set();
        let complexity = 0;

        for (let i = 0; i < binary.length; i++) {
            for (let j = i + 1; j <= binary.length; j++) {
                const pattern = binary.slice(i, j);
                if (!patterns.has(pattern)) {
                    patterns.add(pattern);
                    complexity++;
                }
            }
        }

        return complexity / binary.length;
    }

    calculateInformationContent(signal) {
        const histogram = {};
        signal.forEach(value => {
            const bin = Math.round(value * 1000) / 1000; // Quantize
            histogram[bin] = (histogram[bin] || 0) + 1;
        });

        const total = signal.length;
        let entropy = 0;

        for (const count of Object.values(histogram)) {
            const probability = count / total;
            if (probability > 0) {
                entropy -= probability * Math.log2(probability);
            }
        }

        return entropy;
    }

    calculateSelfSimilarity(signal) {
        const windowSize = Math.floor(signal.length / 4);
        const segments = [];

        for (let i = 0; i < signal.length - windowSize; i += windowSize) {
            segments.push(signal.slice(i, i + windowSize));
        }

        let totalSimilarity = 0;
        let comparisons = 0;

        for (let i = 0; i < segments.length; i++) {
            for (let j = i + 1; j < segments.length; j++) {
                totalSimilarity += this.calculateCorrelation(segments[i], segments[j]);
                comparisons++;
            }
        }

        return comparisons > 0 ? totalSimilarity / comparisons : 0;
    }

    calculateSkewness(signal) {
        const mean = signal.reduce((sum, val) => sum + val, 0) / signal.length;
        const variance = signal.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / signal.length;
        const std = Math.sqrt(variance);

        if (std === 0) return 0;

        const skewness = signal.reduce((sum, val) => sum + Math.pow((val - mean) / std, 3), 0) / signal.length;
        return skewness;
    }

    calculateKurtosis(signal) {
        const mean = signal.reduce((sum, val) => sum + val, 0) / signal.length;
        const variance = signal.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / signal.length;
        const std = Math.sqrt(variance);

        if (std === 0) return 0;

        const kurtosis = signal.reduce((sum, val) => sum + Math.pow((val - mean) / std, 4), 0) / signal.length;
        return kurtosis - 3; // Excess kurtosis
    }

    calculateEntropy(signal) {
        return this.calculateInformationContent(signal);
    }

    // Consciousness assessment methods
    assessComplexity(results) {
        const { statistics, patterns } = results;
        const entropyScore = Math.min(1, statistics.entropy / 10);
        const patternScore = Math.min(1, patterns.emergentStructures.complexityMeasure);
        return (entropyScore + patternScore) / 2;
    }

    assessSelfOrganization(results) {
        const { patterns, frequencyDomain } = results;
        const periodicScore = Math.min(1, patterns.periodicComponents.length / 10);
        const structureScore = Math.min(1, patterns.emergentStructures.selfSimilarity);
        return (periodicScore + structureScore) / 2;
    }

    assessInformationIntegration(results) {
        const { timeDomain, frequencyDomain } = results;
        const energyDistribution = 1 - Math.abs(timeDomain.variance - 0.5);
        const spectralDistribution = frequencyDomain.spectralCentroid / 22050; // Normalized
        return (energyDistribution + spectralDistribution) / 2;
    }

    assessAdaptability(results) {
        // This would require temporal comparison in a real implementation
        const { patterns } = results;
        const anomalyScore = Math.min(1, patterns.anomalies.length / 100);
        const variabilityScore = Math.min(1, patterns.repeatingPatterns.length / 20);
        return (anomalyScore + variabilityScore) / 2;
    }

    assessEmergence(results) {
        const { patterns } = results;
        const fractalScore = Math.min(1, patterns.emergentStructures.fractalDimension / 2);
        const complexityScore = patterns.emergentStructures.complexityMeasure;
        return (fractalScore + complexityScore) / 2;
    }

    generateAnalysisId() {
        return `analysis_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    }

    getAnalysisHistory(limit = 10) {
        return this.analysisHistory.slice(-limit);
    }

    clearHistory() {
        this.analysisHistory = [];
        this.patterns.clear();
    }
}

export default SignalAnalyzer;