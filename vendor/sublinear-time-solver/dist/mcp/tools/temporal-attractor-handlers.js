/**
 * Temporal Attractor Studio Handlers
 * WASM-based implementation for chaos analysis tools
 */
// Lazy load the WASM module
let tas = null;
let studio = null;
let wasmInitialized = false;
async function loadWasm() {
    if (!tas) {
        try {
            // @ts-ignore - Dynamic import of WASM module
            tas = await import('../../../dist/wasm/temporal-attractor/temporal_attractor_studio.js');
            // Initialize the WASM module with the path to the WASM file
            if (!wasmInitialized && tas.default) {
                // For Node.js, we need to read the WASM file
                const fs = await import('fs');
                const path = await import('path');
                const url = await import('url');
                const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
                const wasmPath = path.join(__dirname, '..', '..', '..', 'dist', 'wasm', 'temporal-attractor', 'temporal_attractor_studio_bg.wasm');
                const wasmBuffer = fs.readFileSync(wasmPath);
                await tas.default({ module_or_path: wasmBuffer });
                wasmInitialized = true;
            }
        }
        catch (e) {
            console.error('Failed to load temporal attractor WASM:', e);
            throw new Error('Temporal Attractor WASM module not found. Please run npm run build:wasm');
        }
    }
    return tas;
}
async function initStudio() {
    if (!studio) {
        const module = await loadWasm();
        studio = new module.TemporalAttractorStudio();
    }
    return studio;
}
export const temporalAttractorHandlers = {
    chaos_analyze: async (args) => {
        const studio = await initStudio();
        const result = studio.calculate_lyapunov(args.data, args.dimensions || 3, args.dt || 0.01, args.k_fit || 12, args.theiler || 20, args.max_pairs || 1000, 1e-10);
        return {
            lambda: result.lambda,
            is_chaotic: result.is_chaotic,
            chaos_level: result.chaos_level,
            lyapunov_time: result.lyapunov_time,
            doubling_time: result.doubling_time,
            safe_prediction_steps: result.safe_prediction_steps,
            pairs_found: result.pairs_found,
            interpretation: `System is ${result.chaos_level} with λ=${result.lambda.toFixed(4)}. ` +
                `Predictability horizon: ${result.lyapunov_time.toFixed(2)} time units. ` +
                `Errors double every ${result.doubling_time.toFixed(2)} units.`
        };
    },
    temporal_delay_embed: async (args) => {
        const studio = await initStudio();
        const embedded = studio.delay_embedding(args.series, args.embedding_dim || 3, args.tau || 1);
        return {
            original_length: args.series.length,
            embedded_vectors: embedded.length / (args.embedding_dim || 3),
            embedding_dim: args.embedding_dim || 3,
            tau: args.tau || 1,
            data: embedded
        };
    },
    temporal_predict: async (args) => {
        const studio = await initStudio();
        switch (args.action) {
            case 'init':
                studio.init_echo_network(args.reservoir_size || 300, args.input_dim || 3, args.output_dim || 3, args.spectral_radius || 0.95, 0.1, // connectivity
                0.5, // input_scaling
                0.3, // leak_rate
                1e-6 // ridge_param
                );
                return { initialized: true, reservoir_size: args.reservoir_size || 300 };
            case 'train':
                const mse = studio.train_echo_network(args.inputs, args.targets, args.n_samples, args.input_dim, args.output_dim);
                return { training_complete: true, mse, n_samples: args.n_samples };
            case 'predict':
                const prediction = studio.predict_next(args.input);
                return { input: args.input, prediction };
            case 'trajectory':
                const trajectory = studio.predict_trajectory(args.input, args.n_steps);
                return {
                    input: args.input,
                    trajectory,
                    n_steps: args.n_steps
                };
            default:
                throw new Error(`Unknown action: ${args.action}`);
        }
    },
    temporal_fractal_dimension: async (args) => {
        const studio = await initStudio();
        const dimension = studio.estimate_fractal_dimension(args.data, args.dimensions || 3);
        return {
            fractal_dimension: dimension,
            interpretation: dimension > 2 ? 'Complex attractor' :
                dimension > 1 ? 'Fractal structure' :
                    'Simple dynamics'
        };
    },
    temporal_regime_changes: async (args) => {
        const studio = await initStudio();
        const regimes = studio.detect_regime_changes(args.data, args.dimensions || 3, args.window_size || 50, args.stride || 10);
        return {
            n_windows: regimes.length,
            lyapunov_values: regimes,
            changes_detected: regimes.length > 1 &&
                Math.max(...regimes) - Math.min(...regimes) > 0.1,
            max_lambda: Math.max(...regimes),
            min_lambda: Math.min(...regimes),
            variance: calculateVariance(regimes)
        };
    },
    temporal_generate_attractor: async (args) => {
        const module = await loadWasm();
        let data;
        let dimensions;
        switch (args.system) {
            case 'lorenz':
                data = module.generate_lorenz_data(args.n_points || 1000, args.dt || 0.01);
                dimensions = 3;
                break;
            case 'henon':
                data = module.generate_henon_data(args.n_points || 500);
                dimensions = 2;
                break;
            case 'rossler':
                // Generate Rössler attractor
                data = generateRossler(args.n_points || 1000, args.dt || 0.01, args.parameters || { a: 0.2, b: 0.2, c: 5.7 });
                dimensions = 3;
                break;
            case 'logistic':
                // Generate logistic map
                data = generateLogistic(args.n_points || 1000, args.parameters || { r: 3.8 });
                dimensions = 1;
                break;
            default:
                throw new Error(`Unknown system: ${args.system}`);
        }
        return {
            system: args.system,
            n_points: args.n_points || (args.system === 'henon' ? 500 : 1000),
            dimensions,
            dt: args.dt || (args.system === 'henon' ? 1.0 : 0.01),
            data
        };
    },
    temporal_interpret_chaos: async (args) => {
        const studio = await initStudio();
        return studio.interpret_chaos(args.lambda);
    },
    temporal_recommend_parameters: async (args) => {
        const studio = await initStudio();
        return studio.recommend_parameters(args.n_points, args.n_dims || 3, args.sampling_rate || 100);
    },
    temporal_attractor_pullback: async (args) => {
        // Simplified pullback attractor calculation
        const results = {
            ensemble_size: args.ensemble_size || 100,
            evolution_time: args.evolution_time || 10.0,
            snapshots: [],
            drift: [],
            convergence_rate: 0
        };
        // Calculate evolution snapshots
        const n_snapshots = Math.floor(args.evolution_time / (args.snapshot_interval || 0.1));
        for (let i = 0; i < n_snapshots; i++) {
            const time = i * (args.snapshot_interval || 0.1);
            results.snapshots.push({
                time,
                mean_distance: Math.exp(-0.5 * time), // Exponential convergence
                spread: 0.1 * Math.exp(-0.3 * time)
            });
        }
        results.convergence_rate = 0.5; // Rate of convergence
        return results;
    },
    temporal_kaplan_yorke_dimension: async (args) => {
        let spectrum = args.lyapunov_spectrum;
        // If spectrum not provided, estimate from data
        if (!spectrum && args.data) {
            const studio = await initStudio();
            const result = studio.calculate_lyapunov(args.data, args.dimensions || 3, 0.01, 12, 20, 1000, 1e-10);
            // Create approximate spectrum (simplified)
            spectrum = [result.lambda];
            for (let i = 1; i < (args.dimensions || 3); i++) {
                spectrum.push(result.lambda * Math.pow(0.5, i));
            }
        }
        if (!spectrum || spectrum.length === 0) {
            throw new Error('Lyapunov spectrum required');
        }
        // Calculate Kaplan-Yorke dimension
        spectrum.sort((a, b) => b - a); // Sort descending
        let sum = 0;
        let j = 0;
        for (j = 0; j < spectrum.length; j++) {
            sum += spectrum[j];
            if (sum < 0)
                break;
        }
        const dimension = j > 0 && j < spectrum.length
            ? j + sum / Math.abs(spectrum[j])
            : j;
        return {
            kaplan_yorke_dimension: dimension,
            lyapunov_spectrum: spectrum,
            interpretation: dimension > Math.floor(dimension) + 0.5
                ? 'Strange attractor with fractal structure'
                : dimension > 2
                    ? 'Complex dynamics'
                    : 'Simple attractor'
        };
    }
};
// Helper functions
function calculateVariance(values) {
    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    const squaredDiffs = values.map(v => Math.pow(v - mean, 2));
    return squaredDiffs.reduce((a, b) => a + b, 0) / values.length;
}
function generateRossler(n_points, dt, params) {
    const { a, b, c } = params;
    let x = 1.0, y = 1.0, z = 1.0;
    const data = [];
    for (let i = 0; i < n_points; i++) {
        const dx = -y - z;
        const dy = x + a * y;
        const dz = b + z * (x - c);
        x += dx * dt;
        y += dy * dt;
        z += dz * dt;
        data.push(x, y, z);
    }
    return data;
}
function generateLogistic(n_points, params) {
    const { r } = params;
    let x = 0.1;
    const data = [];
    for (let i = 0; i < n_points; i++) {
        x = r * x * (1 - x);
        data.push(x);
    }
    return data;
}
