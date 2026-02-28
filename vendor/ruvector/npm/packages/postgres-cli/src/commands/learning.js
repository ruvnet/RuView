"use strict";
/**
 * Learning Commands
 * CLI commands for self-learning and ReasoningBank operations
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.LearningCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const fs_1 = require("fs");
class LearningCommands {
    static async train(client, options) {
        const spinner = (0, ora_1.default)('Training from trajectories...').start();
        try {
            await client.connect();
            // Load trajectory data from file
            const content = (0, fs_1.readFileSync)(options.file, 'utf-8');
            const data = JSON.parse(content);
            const epochs = parseInt(options.epochs);
            spinner.text = `Training for ${epochs} epochs...`;
            const result = await client.trainFromTrajectories(data, epochs);
            spinner.succeed(chalk_1.default.green('Training completed successfully'));
            console.log(chalk_1.default.bold.blue('\nTraining Results:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Epochs:')} ${epochs}`);
            console.log(`  ${chalk_1.default.green('Trajectories:')} ${data.length}`);
            console.log(`  ${chalk_1.default.green('Final Loss:')} ${result.loss.toFixed(6)}`);
            console.log(`  ${chalk_1.default.green('Accuracy:')} ${(result.accuracy * 100).toFixed(2)}%`);
            // Show training progress visualization
            console.log(chalk_1.default.bold.blue('\nLearning Progress:'));
            const progressBar = '█'.repeat(Math.floor(result.accuracy * 20)) +
                '░'.repeat(20 - Math.floor(result.accuracy * 20));
            console.log(`  [${chalk_1.default.green(progressBar)}] ${(result.accuracy * 100).toFixed(1)}%`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Training failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async predict(client, options) {
        const spinner = (0, ora_1.default)('Making prediction...').start();
        try {
            await client.connect();
            const input = JSON.parse(options.input);
            const prediction = await client.predict(input);
            spinner.succeed(chalk_1.default.green('Prediction completed'));
            console.log(chalk_1.default.bold.blue('\nPrediction Result:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Input Dimensions:')} ${input.length}`);
            console.log(`  ${chalk_1.default.green('Output Dimensions:')} ${prediction.length}`);
            console.log(`  ${chalk_1.default.green('Output Vector:')}`);
            // Format output nicely
            const formatted = prediction.slice(0, 10).map(v => v.toFixed(4)).join(', ');
            console.log(`  [${formatted}${prediction.length > 10 ? ', ...' : ''}]`);
            // Show stats
            const sum = prediction.reduce((a, b) => a + b, 0);
            const max = Math.max(...prediction);
            const maxIdx = prediction.indexOf(max);
            console.log(chalk_1.default.bold.blue('\nStatistics:'));
            console.log(`  ${chalk_1.default.gray('Sum:')} ${sum.toFixed(4)}`);
            console.log(`  ${chalk_1.default.gray('Max:')} ${max.toFixed(4)} (index ${maxIdx})`);
            console.log(`  ${chalk_1.default.gray('Mean:')} ${(sum / prediction.length).toFixed(4)}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Prediction failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async status(client) {
        const spinner = (0, ora_1.default)('Fetching learning status...').start();
        try {
            await client.connect();
            // Get learning system status
            const result = await client.query('SELECT * FROM learning_status()');
            spinner.stop();
            const status = result[0];
            console.log(chalk_1.default.bold.blue('\nLearning System Status:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            if (status) {
                console.log(`  ${chalk_1.default.green('Models:')} ${status.model_count}`);
                console.log(`  ${chalk_1.default.green('Trajectories:')} ${status.trajectory_count}`);
                console.log(`  ${chalk_1.default.green('Last Training:')} ${status.last_training}`);
                console.log(`  ${chalk_1.default.green('Current Accuracy:')} ${(status.accuracy * 100).toFixed(2)}%`);
            }
            else {
                console.log(chalk_1.default.yellow('  No learning models found'));
                console.log(chalk_1.default.gray('  Train with: ruvector-pg learning train -f <trajectories.json>'));
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to get status'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static showInfo() {
        console.log(chalk_1.default.bold.blue('\nSelf-Learning / ReasoningBank System:'));
        console.log(chalk_1.default.gray('─'.repeat(50)));
        console.log(`
${chalk_1.default.yellow('Overview:')}
  The self-learning system enables the database to learn from
  past query trajectories and improve over time. Based on the
  ReasoningBank architecture.

${chalk_1.default.yellow('Trajectory Format:')}
  A trajectory is a sequence of (state, action, outcome) tuples
  that represent decision points during query execution.

  Example trajectory file (trajectories.json):
  ${chalk_1.default.gray(`[
    {
      "state": [0.1, 0.2, ...],    // Current context vector
      "action": "expand_hnsw",     // Action taken
      "outcome": "success",        // Result
      "reward": 0.95               // Performance score
    },
    ...
  ]`)}

${chalk_1.default.yellow('Commands:')}
  ${chalk_1.default.green('ruvector-pg learning train')} - Train from trajectory data
  ${chalk_1.default.green('ruvector-pg learning predict')} - Make predictions
  ${chalk_1.default.green('ruvector-pg learning status')} - Check system status

${chalk_1.default.yellow('Algorithm:')}
  Uses Decision Transformer architecture to learn optimal
  action sequences from reward-conditioned trajectory data.
`);
    }
}
exports.LearningCommands = LearningCommands;
exports.default = LearningCommands;
//# sourceMappingURL=learning.js.map