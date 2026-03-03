/**
 * Temporal Attractor Studio Handlers
 * WASM-based implementation for chaos analysis tools
 */
export declare const temporalAttractorHandlers: {
    chaos_analyze: (args: any) => Promise<{
        lambda: any;
        is_chaotic: any;
        chaos_level: any;
        lyapunov_time: any;
        doubling_time: any;
        safe_prediction_steps: any;
        pairs_found: any;
        interpretation: string;
    }>;
    temporal_delay_embed: (args: any) => Promise<{
        original_length: any;
        embedded_vectors: number;
        embedding_dim: any;
        tau: any;
        data: any;
    }>;
    temporal_predict: (args: any) => Promise<{
        initialized: boolean;
        reservoir_size: any;
        training_complete?: undefined;
        mse?: undefined;
        n_samples?: undefined;
        input?: undefined;
        prediction?: undefined;
        trajectory?: undefined;
        n_steps?: undefined;
    } | {
        training_complete: boolean;
        mse: any;
        n_samples: any;
        initialized?: undefined;
        reservoir_size?: undefined;
        input?: undefined;
        prediction?: undefined;
        trajectory?: undefined;
        n_steps?: undefined;
    } | {
        input: any;
        prediction: any;
        initialized?: undefined;
        reservoir_size?: undefined;
        training_complete?: undefined;
        mse?: undefined;
        n_samples?: undefined;
        trajectory?: undefined;
        n_steps?: undefined;
    } | {
        input: any;
        trajectory: any;
        n_steps: any;
        initialized?: undefined;
        reservoir_size?: undefined;
        training_complete?: undefined;
        mse?: undefined;
        n_samples?: undefined;
        prediction?: undefined;
    }>;
    temporal_fractal_dimension: (args: any) => Promise<{
        fractal_dimension: any;
        interpretation: string;
    }>;
    temporal_regime_changes: (args: any) => Promise<{
        n_windows: any;
        lyapunov_values: any;
        changes_detected: boolean;
        max_lambda: number;
        min_lambda: number;
        variance: number;
    }>;
    temporal_generate_attractor: (args: any) => Promise<{
        system: any;
        n_points: any;
        dimensions: any;
        dt: any;
        data: any;
    }>;
    temporal_interpret_chaos: (args: any) => Promise<any>;
    temporal_recommend_parameters: (args: any) => Promise<any>;
    temporal_attractor_pullback: (args: any) => Promise<{
        ensemble_size: any;
        evolution_time: any;
        snapshots: any[];
        drift: any[];
        convergence_rate: number;
    }>;
    temporal_kaplan_yorke_dimension: (args: any) => Promise<{
        kaplan_yorke_dimension: number;
        lyapunov_spectrum: any;
        interpretation: string;
    }>;
};
