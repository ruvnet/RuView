/**
 * Webpack configuration for Temporal Neural Solver
 * Optimizes WASM and JavaScript bundling for different deployment scenarios
 */

const path = require('path');
const CopyWebpackPlugin = require('copy-webpack-plugin');
const { CleanWebpackPlugin } = require('clean-webpack-plugin');

module.exports = (env, argv) => {
  const isProduction = argv.mode === 'production';
  const isDevelopment = !isProduction;

  return {
    entry: {
      'temporal-neural-solver': './pkg/temporal_neural_solver.js',
      'temporal-neural-solver.node': './pkg-node/temporal_neural_solver.js',
      'temporal-neural-solver.web': './pkg-web/temporal_neural_solver.js',
    },

    mode: isProduction ? 'production' : 'development',

    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: '[name].js',
      library: 'TemporalNeuralSolver',
      libraryTarget: 'umd',
      globalObject: 'this'
    },

    resolve: {
      extensions: ['.js', '.wasm', '.ts'],
    },

    module: {
      rules: [
        {
          test: /\.wasm$/,
          type: 'webassembly/async',
        },
        {
          test: /\.ts$/,
          use: 'ts-loader',
          exclude: /node_modules/,
        },
      ],
    },

    plugins: [
      new CleanWebpackPlugin(),

      new CopyWebpackPlugin({
        patterns: [
          // Copy WASM files
          { from: 'pkg/*.wasm', to: '[name][ext]' },
          { from: 'pkg-node/*.wasm', to: 'node/[name][ext]' },
          { from: 'pkg-web/*.wasm', to: 'web/[name][ext]' },

          // Copy TypeScript definitions
          { from: 'pkg/*.d.ts', to: '[name][ext]' },

          // Copy CLI tools
          { from: 'pkg/bin/**/*', to: 'bin/[name][ext]' },

          // Copy examples
          { from: 'examples/**/*', to: 'examples/[path][name][ext]' },

          // Copy documentation
          { from: 'README.md', to: 'README.md' },
          { from: 'pkg/package.json', to: 'package.json' },
        ],
      }),
    ],

    experiments: {
      asyncWebAssembly: true,
    },

    optimization: {
      minimize: isProduction,
      splitChunks: {
        chunks: 'all',
        cacheGroups: {
          wasm: {
            test: /\.wasm$/,
            name: 'wasm',
            chunks: 'all',
            enforce: true,
          },
        },
      },
    },

    performance: {
      hints: isDevelopment ? false : 'warning',
      maxAssetSize: 2000000, // 2MB - WASM can be large
      maxEntrypointSize: 2000000,
    },

    devtool: isDevelopment ? 'eval-source-map' : 'source-map',

    stats: {
      errorDetails: true,
      warnings: true,
    },

    // Development server for testing
    devServer: {
      static: {
        directory: path.join(__dirname, 'dist'),
      },
      port: 8080,
      hot: true,
      headers: {
        'Cross-Origin-Embedder-Policy': 'require-corp',
        'Cross-Origin-Opener-Policy': 'same-origin',
      },
    },

    // Node.js specific configuration
    target: env && env.target === 'node' ? 'node' : 'web',

    externals: env && env.target === 'node' ? {
      // Don't bundle Node.js built-ins
      fs: 'commonjs fs',
      path: 'commonjs path',
      util: 'commonjs util',
    } : {},
  };
};

// Export additional configurations for different targets
module.exports.node = {
  ...module.exports,
  target: 'node',
  entry: './pkg-node/temporal_neural_solver.js',
  output: {
    path: path.resolve(__dirname, 'dist/node'),
    filename: 'temporal-neural-solver.js',
    library: 'TemporalNeuralSolver',
    libraryTarget: 'commonjs2',
  },
  externals: {
    fs: 'commonjs fs',
    path: 'commonjs path',
    util: 'commonjs util',
    perf_hooks: 'commonjs perf_hooks',
  },
};

module.exports.web = {
  ...module.exports,
  target: 'web',
  entry: './pkg-web/temporal_neural_solver.js',
  output: {
    path: path.resolve(__dirname, 'dist/web'),
    filename: 'temporal-neural-solver.js',
    library: 'TemporalNeuralSolver',
    libraryTarget: 'window',
  },
  resolve: {
    fallback: {
      fs: false,
      path: false,
      util: false,
    },
  },
};