const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const CopyWebpackPlugin = require('copy-webpack-plugin');

module.exports = (env, argv) => {
  const isProduction = argv.mode === 'production';

  return {
    entry: './index.js',
    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: isProduction ? '[name].[contenthash].js' : '[name].js',
      clean: true,
      library: {
        name: 'MidstreamWasm',
        type: 'umd',
        export: 'default'
      },
      globalObject: 'this'
    },
    experiments: {
      asyncWebAssembly: true,
      syncWebAssembly: true
    },
    module: {
      rules: [
        {
          test: /\.wasm$/,
          type: 'webassembly/async'
        }
      ]
    },
    plugins: [
      new CopyWebpackPlugin({
        patterns: [
          {
            from: 'pkg-bundler/*.wasm',
            to: '[name][ext]',
            noErrorOnMissing: true
          },
          {
            from: 'pkg-bundler/*.js',
            to: '[name][ext]',
            noErrorOnMissing: true
          }
        ]
      }),
      new HtmlWebpackPlugin({
        template: './examples/demo.html',
        filename: 'demo.html',
        inject: 'head',
        scriptLoading: 'defer'
      })
    ],
    devServer: {
      static: {
        directory: path.join(__dirname, 'dist')
      },
      compress: true,
      port: 8080,
      hot: true,
      open: true,
      headers: {
        'Cross-Origin-Opener-Policy': 'same-origin',
        'Cross-Origin-Embedder-Policy': 'require-corp'
      }
    },
    optimization: {
      minimize: isProduction,
      splitChunks: {
        chunks: 'all',
        cacheGroups: {
          wasm: {
            test: /\.wasm$/,
            name: 'wasm',
            priority: 10
          }
        }
      }
    },
    resolve: {
      extensions: ['.js', '.wasm'],
      fallback: {
        'crypto': false,
        'path': false,
        'fs': false
      }
    },
    performance: {
      maxAssetSize: 512000,
      maxEntrypointSize: 512000
    }
  };
};
