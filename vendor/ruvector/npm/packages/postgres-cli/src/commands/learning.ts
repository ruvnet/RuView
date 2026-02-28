/**
 * Learning Commands
 * CLI commands for self-learning and ReasoningBank operations
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import { readFileSync } from 'fs';
import type { RuVectorClient } from '../client.js';

export interface TrainOptions {
  file: string;
  epochs: string;
}

export interface PredictOptions {
  input: string;
}

export class LearningCommands {
  static async train(
    client: RuVectorClient,
    options: TrainOptions
  ): Promise<void> {
    const spinner = ora('Training from trajectories...').start();

    try {
      await client.connect();

      // Load trajectory data from file
      const content = readFileSync(options.file, 'utf-8');
      const data = JSON.parse(content) as Record<string, unknown>[];

      const epochs = parseInt(options.epochs);

      spinner.text = `Training for ${epochs} epochs...`;

      const result = await client.trainFromTrajectories(data, epochs);

      spinner.succeed(chalk.green('Training completed successfully'));

      console.log(chalk.bold.blue('\nTraining Results:'));
      console.log(chalk.gray('─'.repeat(40)));
      console.log(`  ${chalk.green('Epochs:')} ${epochs}`);
      console.log(`  ${chalk.green('Trajectories:')} ${data.length}`);
      console.log(`  ${chalk.green('Final Loss:')} ${result.loss.toFixed(6)}`);
      console.log(`  ${chalk.green('Accuracy:')} ${(result.accuracy * 100).toFixed(2)}%`);

      // Show training progress visualization
      console.log(chalk.bold.blue('\nLearning Progress:'));
      const progressBar = '█'.repeat(Math.floor(result.accuracy * 20)) +
                         '░'.repeat(20 - Math.floor(result.accuracy * 20));
      console.log(`  [${chalk.green(progressBar)}] ${(result.accuracy * 100).toFixed(1)}%`);
    } catch (err) {
      spinner.fail(chalk.red('Training failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async predict(
    client: RuVectorClient,
    options: PredictOptions
  ): Promise<void> {
    const spinner = ora('Making prediction...').start();

    try {
      await client.connect();

      const input = JSON.parse(options.input) as number[];

      const prediction = await client.predict(input);

      spinner.succeed(chalk.green('Prediction completed'));

      console.log(chalk.bold.blue('\nPrediction Result:'));
      console.log(chalk.gray('─'.repeat(40)));
      console.log(`  ${chalk.green('Input Dimensions:')} ${input.length}`);
      console.log(`  ${chalk.green('Output Dimensions:')} ${prediction.length}`);
      console.log(`  ${chalk.green('Output Vector:')}`);

      // Format output nicely
      const formatted = prediction.slice(0, 10).map(v => v.toFixed(4)).join(', ');
      console.log(`  [${formatted}${prediction.length > 10 ? ', ...' : ''}]`);

      // Show stats
      const sum = prediction.reduce((a, b) => a + b, 0);
      const max = Math.max(...prediction);
      const maxIdx = prediction.indexOf(max);

      console.log(chalk.bold.blue('\nStatistics:'));
      console.log(`  ${chalk.gray('Sum:')} ${sum.toFixed(4)}`);
      console.log(`  ${chalk.gray('Max:')} ${max.toFixed(4)} (index ${maxIdx})`);
      console.log(`  ${chalk.gray('Mean:')} ${(sum / prediction.length).toFixed(4)}`);
    } catch (err) {
      spinner.fail(chalk.red('Prediction failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async status(client: RuVectorClient): Promise<void> {
    const spinner = ora('Fetching learning status...').start();

    try {
      await client.connect();

      // Get learning system status
      const result = await client.query<{
        model_count: number;
        trajectory_count: number;
        last_training: string;
        accuracy: number;
      }>(
        'SELECT * FROM learning_status()'
      );

      spinner.stop();

      const status = result[0];

      console.log(chalk.bold.blue('\nLearning System Status:'));
      console.log(chalk.gray('─'.repeat(40)));

      if (status) {
        console.log(`  ${chalk.green('Models:')} ${status.model_count}`);
        console.log(`  ${chalk.green('Trajectories:')} ${status.trajectory_count}`);
        console.log(`  ${chalk.green('Last Training:')} ${status.last_training}`);
        console.log(`  ${chalk.green('Current Accuracy:')} ${(status.accuracy * 100).toFixed(2)}%`);
      } else {
        console.log(chalk.yellow('  No learning models found'));
        console.log(chalk.gray('  Train with: ruvector-pg learning train -f <trajectories.json>'));
      }
    } catch (err) {
      spinner.fail(chalk.red('Failed to get status'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static showInfo(): void {
    console.log(chalk.bold.blue('\nSelf-Learning / ReasoningBank System:'));
    console.log(chalk.gray('─'.repeat(50)));

    console.log(`
${chalk.yellow('Overview:')}
  The self-learning system enables the database to learn from
  past query trajectories and improve over time. Based on the
  ReasoningBank architecture.

${chalk.yellow('Trajectory Format:')}
  A trajectory is a sequence of (state, action, outcome) tuples
  that represent decision points during query execution.

  Example trajectory file (trajectories.json):
  ${chalk.gray(`[
    {
      "state": [0.1, 0.2, ...],    // Current context vector
      "action": "expand_hnsw",     // Action taken
      "outcome": "success",        // Result
      "reward": 0.95               // Performance score
    },
    ...
  ]`)}

${chalk.yellow('Commands:')}
  ${chalk.green('ruvector-pg learning train')} - Train from trajectory data
  ${chalk.green('ruvector-pg learning predict')} - Make predictions
  ${chalk.green('ruvector-pg learning status')} - Check system status

${chalk.yellow('Algorithm:')}
  Uses Decision Transformer architecture to learn optimal
  action sequences from reward-conditioned trajectory data.
`);
  }
}

export default LearningCommands;
