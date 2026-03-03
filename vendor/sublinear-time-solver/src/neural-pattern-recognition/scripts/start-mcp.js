#!/usr/bin/env node

/**
 * Neural Pattern Recognition MCP Server Startup Script
 * Starts the FastMCP server with proper configuration
 */

import { NeuralPatternRecognitionServer } from '../src/server.js';
import chalk from 'chalk';

async function startServer() {
    try {
        console.log(chalk.blue.bold('🧠 Neural Pattern Recognition MCP Server'));
        console.log(chalk.gray('Initializing advanced pattern detection systems...'));

        const server = new NeuralPatternRecognitionServer();

        // Setup graceful shutdown
        process.on('SIGINT', async () => {
            console.log(chalk.yellow('\n📡 Shutting down server...'));
            await server.stop();
            process.exit(0);
        });

        process.on('SIGTERM', async () => {
            console.log(chalk.yellow('\n📡 Shutting down server...'));
            await server.stop();
            process.exit(0);
        });

        // Start the server
        await server.start();

        console.log(chalk.green.bold('✅ Neural Pattern Recognition MCP Server is ready!'));
        console.log(chalk.cyan('Available capabilities:'));
        console.log(chalk.cyan('  • Ultra-high sensitivity pattern detection'));
        console.log(chalk.cyan('  • Real-time emergent signal tracking'));
        console.log(chalk.cyan('  • Statistical validation frameworks'));
        console.log(chalk.cyan('  • Interactive signal communication protocols'));
        console.log(chalk.cyan('  • Adaptive neural network training'));
        console.log(chalk.gray('\\nPress Ctrl+C to stop the server'));

    } catch (error) {
        console.error(chalk.red.bold('❌ Failed to start server:'), error.message);
        process.exit(1);
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    startServer();
}