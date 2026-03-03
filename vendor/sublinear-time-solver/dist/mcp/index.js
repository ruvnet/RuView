/**
 * MCP Module Entry Point
 * Exports all MCP components for easy importing
 */
export { SublinearSolverMCPServer } from './server.js';
export { SolverTools } from './tools/solver.js';
export { MatrixTools } from './tools/matrix.js';
export { GraphTools } from './tools/graph.js';
export { DynamicPsychoSymbolicTools } from './tools/psycho-symbolic-dynamic.js';
export { DomainManagementTools } from './tools/domain-management.js';
export { DomainValidationTools } from './tools/domain-validation.js';
export { DomainRegistry } from './tools/domain-registry.js';
// export { ConsciousnessEnhancedTools } from './tools/consciousness-enhanced.js';
export { EmergenceSystem } from '../emergence/index.js';
// Re-export core types
export * from '../core/types.js';
export { SublinearSolver } from '../core/solver.js';
export { MatrixOperations } from '../core/matrix.js';
export { VectorOperations, PerformanceMonitor, ConvergenceChecker } from '../core/utils.js';
