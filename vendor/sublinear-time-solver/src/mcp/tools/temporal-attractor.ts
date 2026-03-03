/**
 * Temporal Attractor Studio Tools
 * High-performance chaos analysis and Lyapunov exponent calculation
 * Integrated with sublinear solver for temporal dynamics
 */

import { Tool } from '@modelcontextprotocol/sdk/types.js';

// Tool definitions for temporal attractor analysis
export const temporalAttractorTools: Tool[] = [
  {
    name: 'chaos_analyze',
    description: 'Calculate Lyapunov exponent and chaos metrics from time series data using WASM-optimized algorithms',
    inputSchema: {
      type: 'object',
      properties: {
        data: {
          type: 'array',
          description: 'Time series data (flattened array)',
          items: { type: 'number' }
        },
        dimensions: {
          type: 'integer',
          description: 'Number of dimensions per time point',
          minimum: 1,
          default: 3
        },
        dt: {
          type: 'number',
          description: 'Time step between measurements',
          default: 0.01
        },
        k_fit: {
          type: 'integer',
          description: 'Points for linear fitting (Rosenstein algorithm)',
          default: 12
        },
        theiler: {
          type: 'integer',
          description: 'Theiler window to exclude temporal neighbors',
          default: 20
        },
        max_pairs: {
          type: 'integer',
          description: 'Maximum trajectory pairs to analyze',
          default: 1000
        }
      },
      required: ['data']
    }
  },
  {
    name: 'temporal_delay_embed',
    description: 'Perform delay embedding for phase space reconstruction (Takens theorem)',
    inputSchema: {
      type: 'object',
      properties: {
        series: {
          type: 'array',
          description: 'Univariate time series',
          items: { type: 'number' }
        },
        embedding_dim: {
          type: 'integer',
          description: 'Embedding dimension (typically 3-5)',
          minimum: 2,
          maximum: 10,
          default: 3
        },
        tau: {
          type: 'integer',
          description: 'Time delay (typically 1-10)',
          minimum: 1,
          default: 1
        }
      },
      required: ['series']
    }
  },
  {
    name: 'temporal_predict',
    description: 'Initialize and use Echo-State Network for temporal prediction',
    inputSchema: {
      type: 'object',
      properties: {
        action: {
          type: 'string',
          description: 'Action to perform',
          enum: ['init', 'train', 'predict', 'trajectory']
        },
        // For initialization
        reservoir_size: {
          type: 'integer',
          description: 'Number of reservoir nodes (100-1000 typical)',
          default: 300
        },
        input_dim: {
          type: 'integer',
          description: 'Input dimension',
          minimum: 1,
          default: 3
        },
        output_dim: {
          type: 'integer',
          description: 'Output dimension',
          minimum: 1,
          default: 3
        },
        spectral_radius: {
          type: 'number',
          description: 'Spectral radius (< 1 for stability)',
          default: 0.95
        },
        // For training
        inputs: {
          type: 'array',
          description: 'Training input data (flattened)',
          items: { type: 'number' }
        },
        targets: {
          type: 'array',
          description: 'Training target data (flattened)',
          items: { type: 'number' }
        },
        n_samples: {
          type: 'integer',
          description: 'Number of training samples',
          minimum: 1
        },
        // For prediction
        input: {
          type: 'array',
          description: 'Current state vector for prediction',
          items: { type: 'number' }
        },
        n_steps: {
          type: 'integer',
          description: 'Number of steps to predict (for trajectory)',
          default: 1
        }
      },
      required: ['action']
    }
  },
  {
    name: 'temporal_fractal_dimension',
    description: 'Estimate fractal dimension of attractor using box-counting algorithm',
    inputSchema: {
      type: 'object',
      properties: {
        data: {
          type: 'array',
          description: 'Time series data (flattened)',
          items: { type: 'number' }
        },
        dimensions: {
          type: 'integer',
          description: 'Number of dimensions per point',
          minimum: 1,
          default: 3
        }
      },
      required: ['data']
    }
  },
  {
    name: 'temporal_regime_changes',
    description: 'Detect regime changes in chaotic dynamics using sliding window analysis',
    inputSchema: {
      type: 'object',
      properties: {
        data: {
          type: 'array',
          description: 'Time series data (flattened)',
          items: { type: 'number' }
        },
        dimensions: {
          type: 'integer',
          description: 'Dimensions per point',
          minimum: 1,
          default: 3
        },
        window_size: {
          type: 'integer',
          description: 'Size of analysis window',
          default: 50
        },
        stride: {
          type: 'integer',
          description: 'Stride between windows',
          default: 10
        }
      },
      required: ['data']
    }
  },
  {
    name: 'temporal_generate_attractor',
    description: 'Generate known chaotic attractor data for testing',
    inputSchema: {
      type: 'object',
      properties: {
        system: {
          type: 'string',
          description: 'Attractor system to generate',
          enum: ['lorenz', 'henon', 'rossler', 'chua', 'logistic']
        },
        n_points: {
          type: 'integer',
          description: 'Number of points to generate',
          default: 1000
        },
        dt: {
          type: 'number',
          description: 'Time step (for continuous systems)',
          default: 0.01
        },
        parameters: {
          type: 'object',
          description: 'System-specific parameters (e.g., sigma, rho, beta for Lorenz)',
          additionalProperties: { type: 'number' }
        }
      },
      required: ['system']
    }
  },
  {
    name: 'temporal_interpret_chaos',
    description: 'Get human-readable interpretation of Lyapunov exponent and chaos metrics',
    inputSchema: {
      type: 'object',
      properties: {
        lambda: {
          type: 'number',
          description: 'Lyapunov exponent value'
        },
        dimension: {
          type: 'number',
          description: 'Fractal dimension (optional)'
        },
        system_name: {
          type: 'string',
          description: 'Name of the system being analyzed (optional)'
        }
      },
      required: ['lambda']
    }
  },
  {
    name: 'temporal_recommend_parameters',
    description: 'Get recommended analysis parameters based on data characteristics',
    inputSchema: {
      type: 'object',
      properties: {
        n_points: {
          type: 'integer',
          description: 'Number of data points',
          minimum: 1
        },
        n_dims: {
          type: 'integer',
          description: 'Number of dimensions',
          minimum: 1,
          default: 3
        },
        sampling_rate: {
          type: 'number',
          description: 'Sampling rate in Hz',
          default: 100
        },
        signal_type: {
          type: 'string',
          description: 'Type of signal',
          enum: ['continuous', 'discrete', 'mixed'],
          default: 'continuous'
        }
      },
      required: ['n_points']
    }
  },
  {
    name: 'temporal_attractor_pullback',
    description: 'Calculate pullback attractor dynamics and evolution',
    inputSchema: {
      type: 'object',
      properties: {
        initial_conditions: {
          type: 'array',
          description: 'Initial state vectors for ensemble',
          items: {
            type: 'array',
            items: { type: 'number' }
          }
        },
        ensemble_size: {
          type: 'integer',
          description: 'Number of trajectories in ensemble',
          default: 100
        },
        evolution_time: {
          type: 'number',
          description: 'Total evolution time',
          default: 10.0
        },
        snapshot_interval: {
          type: 'number',
          description: 'Time between snapshots',
          default: 0.1
        }
      },
      required: ['initial_conditions']
    }
  },
  {
    name: 'temporal_kaplan_yorke_dimension',
    description: 'Calculate Kaplan-Yorke dimension from Lyapunov spectrum',
    inputSchema: {
      type: 'object',
      properties: {
        lyapunov_spectrum: {
          type: 'array',
          description: 'Array of Lyapunov exponents (sorted descending)',
          items: { type: 'number' }
        },
        data: {
          type: 'array',
          description: 'Alternative: calculate spectrum from data',
          items: { type: 'number' }
        },
        dimensions: {
          type: 'integer',
          description: 'Dimensions for data (if provided)',
          default: 3
        }
      },
      required: []
    }
  }
];

// Export handler functions that will call the WASM implementation
export { temporalAttractorHandlers } from './temporal-attractor-handlers.js';