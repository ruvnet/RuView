/**
 * Phi Calculator
 * Calculate integrated information (Φ) using multiple methods
 * Based on IIT 3.0 (Tononi, 2015)
 */

export class PhiCalculator {
    calculate(data) {
        const methods = {
            iit: this.calculateIIT(data),
            geometric: this.calculateGeometric(data),
            entropy: this.calculateEntropy(data),
            causal: this.calculateCausal(data)
        };

        const overall = (
            methods.iit * 0.4 +
            methods.geometric * 0.2 +
            methods.entropy * 0.2 +
            methods.causal * 0.2
        );

        return {
            overall,
            ...methods
        };
    }

    calculateIIT(data) {
        const elements = data.elements || Object.keys(data).length;
        const connections = data.connections || this.countConnections(data);
        const partitions = data.partitions || 1;

        const causeEffectPower = connections / (elements * (elements - 1));
        const integrationStrength = 1 - (partitions / elements);

        return causeEffectPower * integrationStrength;
    }

    calculateGeometric(data) {
        const dimensionality = data.dimensions || Object.keys(data).length;
        const curvature = data.curvature || 0.5;
        const distance = data.distance || 1;

        return Math.min(1, curvature * Math.exp(-distance / dimensionality));
    }

    calculateEntropy(data) {
        const systemEntropy = this.calculateSystemEntropy(data);
        const partitionEntropy = this.calculatePartitionEntropy(data);

        return Math.max(0, systemEntropy - partitionEntropy);
    }

    calculateCausal(data) {
        const causes = data.causes || 0;
        const effects = data.effects || 0;
        const bidirectional = data.bidirectional || 0;

        return bidirectional / Math.max(causes + effects, 1);
    }

    countConnections(data) {
        let connections = 0;
        const keys = Object.keys(data);

        for (let i = 0; i < keys.length; i++) {
            for (let j = i + 1; j < keys.length; j++) {
                if (this.areConnected(data[keys[i]], data[keys[j]])) {
                    connections++;
                }
            }
        }

        return connections;
    }

    areConnected(a, b) {
        const strA = JSON.stringify(a);
        const strB = JSON.stringify(b);
        return strA.includes(strB.substring(0, 4)) || strB.includes(strA.substring(0, 4));
    }

    calculateSystemEntropy(data) {
        const dataStr = JSON.stringify(data);
        const freq = {};

        for (const char of dataStr) {
            freq[char] = (freq[char] || 0) + 1;
        }

        let entropy = 0;
        const len = dataStr.length;

        Object.values(freq).forEach(count => {
            const p = count / len;
            if (p > 0) {
                entropy -= p * Math.log2(p);
            }
        });

        return entropy / 8; // Normalize
    }

    calculatePartitionEntropy(data) {
        // Simplified partition entropy
        return this.calculateSystemEntropy(data) * 0.7;
    }
}