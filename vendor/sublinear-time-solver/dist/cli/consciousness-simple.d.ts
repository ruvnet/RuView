#!/usr/bin/env node
import { Command } from 'commander';
export declare function createConsciousnessCommand(): Command;
export declare const consciousnessTools: {
    processInput: (input: number[]) => Promise<number>;
    measurePhi: () => Promise<number>;
    getAttention: () => Promise<number[]>;
    temporalBinding: () => Promise<number>;
    benchmark: (iterations: number) => Promise<any>;
};
