/**
 * Statistical Validator
 * Rigorous statistical validation system for pattern significance testing
 */

export class StatisticalValidator {
    constructor(options = {}) {
        this.config = {
            defaultConfidenceLevel: options.confidenceLevel || 0.99,
            defaultPValueThreshold: options.pValueThreshold || 1e-40,
            minimumSampleSize: options.minimumSampleSize || 100,
            ...options
        };

        this.testMethods = new Map();
        this.initializeTestMethods();
    }

    initializeTestMethods() {
        this.testMethods.set('kolmogorov_smirnov', this.kolmogorovSmirnov.bind(this));
        this.testMethods.set('mann_whitney_u', this.mannWhitneyU.bind(this));
        this.testMethods.set('chi_square', this.chiSquare.bind(this));
        this.testMethods.set('fisher_exact', this.fisherExact.bind(this));
        this.testMethods.set('anderson_darling', this.andersonDarling.bind(this));
    }

    async runValidationSuite(pattern, options = {}) {
        const {
            tests = ['kolmogorov_smirnov', 'mann_whitney_u'],
            pValueThreshold = this.config.defaultPValueThreshold,
            confidenceLevel = this.config.defaultConfidenceLevel,
            includeControlGroups = true
        } = options;

        try {
            const validation = {
                isSignificant: false,
                pValues: {},
                effectSizes: {},
                confidenceIntervals: {},
                summary: {},
                recommendations: []
            };

            // Run each statistical test
            for (const testName of tests) {
                if (this.testMethods.has(testName)) {
                    const testMethod = this.testMethods.get(testName);
                    const result = await testMethod(pattern, { confidenceLevel, includeControlGroups });

                    validation.pValues[testName] = result.pValue;
                    validation.effectSizes[testName] = result.effectSize;
                    validation.confidenceIntervals[testName] = result.confidenceInterval;
                }
            }

            // Determine overall significance
            const allPValues = Object.values(validation.pValues);
            validation.isSignificant = allPValues.every(p => p < pValueThreshold);

            // Generate summary
            validation.summary = this.generateSummary(validation, options);

            // Generate recommendations
            validation.recommendations = this.generateRecommendations(validation);

            return validation;

        } catch (error) {
            console.error('[StatisticalValidator] Validation error:', error);
            throw error;
        }
    }

    // Statistical Test Implementations

    async kolmogorovSmirnov(pattern, options) {
        // Kolmogorov-Smirnov test implementation
        const sample = pattern.data || [];
        const referenceDistribution = options.reference || this.generateNormalDistribution(sample.length);

        const dStatistic = this.calculateKSStatistic(sample, referenceDistribution);
        const pValue = this.calculateKSPValue(dStatistic, sample.length);

        return {
            statistic: dStatistic,
            pValue,
            effectSize: this.calculateEffectSize(sample, referenceDistribution),
            confidenceInterval: this.calculateConfidenceInterval(dStatistic, options.confidenceLevel)
        };
    }

    async mannWhitneyU(pattern, options) {
        // Mann-Whitney U test implementation
        const sample1 = pattern.data || [];
        const sample2 = options.controlGroup || this.generateControlSample(sample1.length);

        const uStatistic = this.calculateUStatistic(sample1, sample2);
        const pValue = this.calculateUPValue(uStatistic, sample1.length, sample2.length);

        return {
            statistic: uStatistic,
            pValue,
            effectSize: this.calculateMannWhitneyEffectSize(sample1, sample2),
            confidenceInterval: this.calculateConfidenceInterval(uStatistic, options.confidenceLevel)
        };
    }

    async chiSquare(pattern, options) {
        // Chi-square test implementation
        const observed = pattern.frequencies || this.calculateFrequencies(pattern.data);
        const expected = options.expected || this.calculateExpectedFrequencies(observed);

        const chiSquareStatistic = this.calculateChiSquareStatistic(observed, expected);
        const degreesOfFreedom = observed.length - 1;
        const pValue = this.calculateChiSquarePValue(chiSquareStatistic, degreesOfFreedom);

        return {
            statistic: chiSquareStatistic,
            pValue,
            degreesOfFreedom,
            effectSize: this.calculateCramersV(chiSquareStatistic, observed.length),
            confidenceInterval: this.calculateConfidenceInterval(chiSquareStatistic, options.confidenceLevel)
        };
    }

    async fisherExact(pattern, options) {
        // Fisher's exact test implementation
        const contingencyTable = pattern.contingencyTable || this.createContingencyTable(pattern.data);

        const pValue = this.calculateFisherExactPValue(contingencyTable);
        const oddsRatio = this.calculateOddsRatio(contingencyTable);

        return {
            pValue,
            oddsRatio,
            effectSize: Math.log(oddsRatio),
            confidenceInterval: this.calculateOddsRatioCI(contingencyTable, options.confidenceLevel)
        };
    }

    async andersonDarling(pattern, options) {
        // Anderson-Darling test implementation
        const sample = pattern.data || [];
        const distribution = options.distribution || 'normal';

        const adStatistic = this.calculateADStatistic(sample, distribution);
        const pValue = this.calculateADPValue(adStatistic, sample.length);

        return {
            statistic: adStatistic,
            pValue,
            effectSize: this.calculateADEffectSize(adStatistic),
            confidenceInterval: this.calculateConfidenceInterval(adStatistic, options.confidenceLevel)
        };
    }

    // Statistical Calculation Methods

    calculateKSStatistic(sample, reference) {
        // Implement Kolmogorov-Smirnov D statistic
        const sortedSample = [...sample].sort((a, b) => a - b);
        const sortedRef = [...reference].sort((a, b) => a - b);

        let maxDiff = 0;
        const n = sortedSample.length;
        const m = sortedRef.length;

        for (let i = 0; i < n; i++) {
            const empiricalCDF = (i + 1) / n;
            const theoreticalCDF = this.getCDF(sortedSample[i], sortedRef);
            const diff = Math.abs(empiricalCDF - theoreticalCDF);
            maxDiff = Math.max(maxDiff, diff);
        }

        return maxDiff;
    }

    calculateKSPValue(dStatistic, sampleSize) {
        // Approximate p-value calculation for KS test
        const lambda = dStatistic * Math.sqrt(sampleSize);
        return 2 * Math.exp(-2 * lambda * lambda);
    }

    calculateUStatistic(sample1, sample2) {
        // Mann-Whitney U statistic
        const combined = [...sample1.map((x, i) => ({ value: x, group: 1 })),
                          ...sample2.map((x, i) => ({ value: x, group: 2 }))];

        combined.sort((a, b) => a.value - b.value);

        let u1 = 0;
        for (let i = 0; i < combined.length; i++) {
            if (combined[i].group === 1) {
                u1 += i + 1; // rank (1-indexed)
            }
        }

        const n1 = sample1.length;
        const n2 = sample2.length;
        u1 -= (n1 * (n1 + 1)) / 2;

        return Math.min(u1, n1 * n2 - u1);
    }

    calculateUPValue(uStatistic, n1, n2) {
        // Approximate p-value for Mann-Whitney U test
        const meanU = (n1 * n2) / 2;
        const stdU = Math.sqrt((n1 * n2 * (n1 + n2 + 1)) / 12);
        const z = (uStatistic - meanU) / stdU;

        return 2 * (1 - this.normalCDF(Math.abs(z)));
    }

    calculateChiSquareStatistic(observed, expected) {
        let chiSquare = 0;
        for (let i = 0; i < observed.length; i++) {
            chiSquare += Math.pow(observed[i] - expected[i], 2) / expected[i];
        }
        return chiSquare;
    }

    calculateChiSquarePValue(chiSquare, df) {
        // Approximate p-value using gamma function
        return 1 - this.gammaCDF(chiSquare / 2, df / 2);
    }

    calculateFisherExactPValue(table) {
        // Fisher's exact test p-value calculation
        const [[a, b], [c, d]] = table;
        const n = a + b + c + d;

        // Hypergeometric probability
        const numerator = this.factorial(a + b) * this.factorial(c + d) *
                         this.factorial(a + c) * this.factorial(b + d);
        const denominator = this.factorial(n) * this.factorial(a) *
                           this.factorial(b) * this.factorial(c) * this.factorial(d);

        return numerator / denominator;
    }

    calculateADStatistic(sample, distribution) {
        // Anderson-Darling A² statistic
        const n = sample.length;
        const sortedSample = [...sample].sort((a, b) => a - b);

        let sum = 0;
        for (let i = 0; i < n; i++) {
            const f = this.getCDF(sortedSample[i], distribution);
            const term = (2 * (i + 1) - 1) * (Math.log(f) + Math.log(1 - this.getCDF(sortedSample[n - 1 - i], distribution)));
            sum += term;
        }

        return -n - (1 / n) * sum;
    }

    calculateADPValue(adStatistic, sampleSize) {
        // Approximate p-value for Anderson-Darling test
        const adjustedStat = adStatistic * (1 + 4/sampleSize - 25/(sampleSize * sampleSize));

        if (adjustedStat < 0.2) return 1 - Math.exp(-13.436 + 101.14 * adjustedStat - 223.73 * adjustedStat * adjustedStat);
        if (adjustedStat < 0.34) return 1 - Math.exp(-8.318 + 42.796 * adjustedStat - 59.938 * adjustedStat * adjustedStat);
        if (adjustedStat < 0.6) return Math.exp(0.9177 - 4.279 * adjustedStat - 1.38 * adjustedStat * adjustedStat);
        return Math.exp(1.2937 - 5.709 * adjustedStat + 0.0186 * adjustedStat * adjustedStat);
    }

    // Helper Methods

    generateNormalDistribution(size, mean = 0, std = 1) {
        const distribution = [];
        for (let i = 0; i < size; i++) {
            distribution.push(this.normalRandom(mean, std));
        }
        return distribution;
    }

    generateControlSample(size) {
        return this.generateNormalDistribution(size);
    }

    calculateFrequencies(data) {
        const frequencies = {};
        data.forEach(value => {
            frequencies[value] = (frequencies[value] || 0) + 1;
        });
        return Object.values(frequencies);
    }

    calculateExpectedFrequencies(observed) {
        const total = observed.reduce((sum, freq) => sum + freq, 0);
        const expectedFreq = total / observed.length;
        return new Array(observed.length).fill(expectedFreq);
    }

    calculateEffectSize(sample1, sample2) {
        const mean1 = this.mean(sample1);
        const mean2 = this.mean(sample2);
        const pooledStd = this.pooledStandardDeviation(sample1, sample2);
        return (mean1 - mean2) / pooledStd;
    }

    calculateMannWhitneyEffectSize(sample1, sample2) {
        // Calculate rank-biserial correlation
        const u = this.calculateUStatistic(sample1, sample2);
        const n1 = sample1.length;
        const n2 = sample2.length;
        return 1 - (2 * u) / (n1 * n2);
    }

    calculateCramersV(chiSquare, n) {
        return Math.sqrt(chiSquare / n);
    }

    calculateOddsRatio(table) {
        const [[a, b], [c, d]] = table;
        return (a * d) / (b * c);
    }

    calculateConfidenceInterval(statistic, confidenceLevel) {
        const alpha = 1 - confidenceLevel;
        const z = this.normalInverse(1 - alpha / 2);
        const margin = z * Math.sqrt(statistic);

        return {
            lower: statistic - margin,
            upper: statistic + margin,
            level: confidenceLevel
        };
    }

    calculateOddsRatioCI(table, confidenceLevel) {
        const [[a, b], [c, d]] = table;
        const logOR = Math.log(this.calculateOddsRatio(table));
        const se = Math.sqrt(1/a + 1/b + 1/c + 1/d);
        const alpha = 1 - confidenceLevel;
        const z = this.normalInverse(1 - alpha / 2);

        return {
            lower: Math.exp(logOR - z * se),
            upper: Math.exp(logOR + z * se),
            level: confidenceLevel
        };
    }

    // Utility Methods

    mean(data) {
        return data.reduce((sum, x) => sum + x, 0) / data.length;
    }

    standardDeviation(data) {
        const m = this.mean(data);
        const variance = data.reduce((sum, x) => sum + Math.pow(x - m, 2), 0) / (data.length - 1);
        return Math.sqrt(variance);
    }

    pooledStandardDeviation(sample1, sample2) {
        const n1 = sample1.length;
        const n2 = sample2.length;
        const s1 = this.standardDeviation(sample1);
        const s2 = this.standardDeviation(sample2);

        return Math.sqrt(((n1 - 1) * s1 * s1 + (n2 - 1) * s2 * s2) / (n1 + n2 - 2));
    }

    getCDF(value, distribution) {
        if (typeof distribution === 'string') {
            switch (distribution) {
                case 'normal':
                    return this.normalCDF(value);
                default:
                    return 0.5; // Fallback
            }
        } else if (Array.isArray(distribution)) {
            // Empirical CDF
            const sorted = [...distribution].sort((a, b) => a - b);
            let count = 0;
            for (const x of sorted) {
                if (x <= value) count++;
                else break;
            }
            return count / sorted.length;
        }
        return 0.5;
    }

    normalCDF(z) {
        // Standard normal CDF approximation
        return 0.5 * (1 + this.erf(z / Math.sqrt(2)));
    }

    normalInverse(p) {
        // Approximate inverse normal CDF
        return Math.sqrt(2) * this.erfInverse(2 * p - 1);
    }

    normalRandom(mean = 0, std = 1) {
        // Box-Muller transformation
        const u1 = Math.random();
        const u2 = Math.random();
        const z = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
        return z * std + mean;
    }

    erf(x) {
        // Error function approximation
        const a1 =  0.254829592;
        const a2 = -0.284496736;
        const a3 =  1.421413741;
        const a4 = -1.453152027;
        const a5 =  1.061405429;
        const p  =  0.3275911;

        const sign = x >= 0 ? 1 : -1;
        x = Math.abs(x);

        const t = 1.0 / (1.0 + p * x);
        const y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * Math.exp(-x * x);

        return sign * y;
    }

    erfInverse(x) {
        // Approximate inverse error function
        const a = 0.147;
        const ln1MinusX2 = Math.log(1 - x * x);
        const term1 = 2 / (Math.PI * a) + ln1MinusX2 / 2;
        const term2 = ln1MinusX2 / a;

        return Math.sign(x) * Math.sqrt(Math.sqrt(term1 * term1 - term2) - term1);
    }

    gammaCDF(x, alpha) {
        // Incomplete gamma function approximation
        return this.gamma(alpha, x) / this.gamma(alpha);
    }

    gamma(z, x = Infinity) {
        // Gamma function approximation
        if (x === Infinity) {
            // Complete gamma function
            return Math.sqrt(2 * Math.PI / z) * Math.pow(z / Math.E, z);
        } else {
            // Incomplete gamma function (simplified)
            return this.gamma(z) * (1 - Math.exp(-x) * Math.pow(x, z - 1));
        }
    }

    factorial(n) {
        if (n <= 1) return 1;
        return n * this.factorial(n - 1);
    }

    createContingencyTable(data) {
        // Create 2x2 contingency table from data
        const positive = data.filter(x => x > 0).length;
        const negative = data.length - positive;
        const expected = data.length / 2;

        return [
            [positive, expected - positive],
            [negative, expected - negative]
        ];
    }

    generateSummary(validation, options) {
        const significantTests = Object.entries(validation.pValues)
            .filter(([test, pValue]) => pValue < options.pValueThreshold)
            .map(([test]) => test);

        return {
            totalTests: Object.keys(validation.pValues).length,
            significantTests: significantTests.length,
            overallSignificance: validation.isSignificant,
            minPValue: Math.min(...Object.values(validation.pValues)),
            maxPValue: Math.max(...Object.values(validation.pValues)),
            testsPassed: significantTests
        };
    }

    generateRecommendations(validation) {
        const recommendations = [];

        if (validation.isSignificant) {
            recommendations.push({
                type: 'validation',
                priority: 'high',
                message: 'Pattern shows statistical significance across multiple tests'
            });
        }

        const minPValue = Math.min(...Object.values(validation.pValues));
        if (minPValue < 1e-50) {
            recommendations.push({
                type: 'investigation',
                priority: 'critical',
                message: 'Extremely low p-values detected - extraordinary phenomenon possible'
            });
        }

        if (validation.summary.significantTests < validation.summary.totalTests / 2) {
            recommendations.push({
                type: 'caution',
                priority: 'medium',
                message: 'Mixed results across tests - consider additional validation'
            });
        }

        return recommendations;
    }

    getStatus() {
        return {
            availableTests: Array.from(this.testMethods.keys()),
            defaultConfig: this.config,
            ready: true
        };
    }
}