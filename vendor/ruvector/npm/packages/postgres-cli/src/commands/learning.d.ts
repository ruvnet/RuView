/**
 * Learning Commands
 * CLI commands for self-learning and ReasoningBank operations
 */
import type { RuVectorClient } from '../client.js';
export interface TrainOptions {
    file: string;
    epochs: string;
}
export interface PredictOptions {
    input: string;
}
export declare class LearningCommands {
    static train(client: RuVectorClient, options: TrainOptions): Promise<void>;
    static predict(client: RuVectorClient, options: PredictOptions): Promise<void>;
    static status(client: RuVectorClient): Promise<void>;
    static showInfo(): void;
}
export default LearningCommands;
//# sourceMappingURL=learning.d.ts.map