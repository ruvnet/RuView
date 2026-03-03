
// TypeScript integration test
import type { GraphReasoner } from './graph_reasoner/pkg/graph_reasoner';
import type { TextExtractor } from './extractors/pkg/extractors';
import type { PlannerSystem } from './planner/pkg/planner';

// Test interface compatibility
interface WasmModules {
    graphReasoner: GraphReasoner;
    textExtractor: TextExtractor;
    plannerSystem: PlannerSystem;
}

// Example usage function
export function exampleUsage(): void {
    console.log('WASM modules can be used in TypeScript!');
}
