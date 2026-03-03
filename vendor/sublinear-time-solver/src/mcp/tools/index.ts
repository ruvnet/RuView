/**
 * MCP Tools Export
 *
 * This module exports all MCP tool classes and provides
 * a consolidated tool list for the MCP server
 */

// Import all tool classes
import { SolverTools } from './solver.js';
import { MatrixTools } from './matrix.js';
import { EmergenceTools } from './emergence-tools.js';
import { ConsciousnessTools } from './consciousness.js';
import { SchedulerTools } from './scheduler.js';
import { PsychoSymbolicTools } from './psycho-symbolic.js';
import { WasmSublinearSolverTools } from './wasm-sublinear-solver.js';
import { temporalAttractorTools } from './temporal-attractor.js';
import { temporalAttractorHandlers } from './temporal-attractor-handlers.js';

// Export classes for direct usage
export { SolverTools } from './solver.js';
export { MatrixTools } from './matrix.js';
export { EmergenceTools } from './emergence-tools.js';
export { ConsciousnessTools } from './consciousness.js';
export { SchedulerTools } from './scheduler.js';
export { PsychoSymbolicTools } from './psycho-symbolic.js';
export { WasmSublinearSolverTools } from './wasm-sublinear-solver.js';
export { temporalAttractorHandlers } from './temporal-attractor-handlers.js';

// Create instances for getting tool definitions
const solverToolsInstance = new SolverTools();
const matrixToolsInstance = new MatrixTools();
const emergenceToolsInstance = new EmergenceTools();
const consciousnessToolsInstance = new ConsciousnessTools();
const schedulerToolsInstance = new SchedulerTools();
const psychoSymbolicToolsInstance = new PsychoSymbolicTools();
const wasmSolverToolsInstance = new WasmSublinearSolverTools();

// Export tool arrays (if classes have getTools method, otherwise empty)
export const solverTools = (solverToolsInstance as any).getTools?.() || [];
export const matrixTools = (matrixToolsInstance as any).getTools?.() || [];
export const emergenceTools = (emergenceToolsInstance as any).getTools?.() || [];
export const consciousnessTools = (consciousnessToolsInstance as any).getTools?.() || [];
export const schedulerTools = (schedulerToolsInstance as any).getTools?.() || [];
export const psychoSymbolicTools = (psychoSymbolicToolsInstance as any).getTools?.() || [];

// Temporal attractor tools are exported directly from the file
export { temporalAttractorTools } from './temporal-attractor.js';

// For backward compatibility - if getTools doesn't exist,
// we'll assume the tools are defined in the MCP server itself
export const allTools = [
  ...solverTools,
  ...matrixTools,
  ...emergenceTools,
  ...consciousnessTools,
  ...schedulerTools,
  ...psychoSymbolicTools,
  ...temporalAttractorTools
];

// Default export with both instances and classes
export default {
  // Instances (for calling methods)
  solver: solverToolsInstance,
  matrix: matrixToolsInstance,
  emergence: emergenceToolsInstance,
  consciousness: consciousnessToolsInstance,
  scheduler: schedulerToolsInstance,
  psychoSymbolic: psychoSymbolicToolsInstance,
  temporalAttractor: temporalAttractorHandlers,

  // Classes (for creating new instances)
  SolverTools,
  MatrixTools,
  EmergenceTools,
  ConsciousnessTools,
  SchedulerTools,
  PsychoSymbolicTools,

  // Tool arrays (may be empty if getTools doesn't exist)
  solverTools,
  matrixTools,
  emergenceTools,
  consciousnessTools,
  schedulerTools,
  psychoSymbolicTools,
  allTools
};