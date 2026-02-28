/**
 * Hyperbolic Geometry Commands
 * CLI commands for hyperbolic embedding operations (Poincare ball, Lorentz model)
 *
 * NOTE: These functions require the hyperbolic geometry module to be enabled
 * in the RuVector PostgreSQL extension. Currently in development.
 */
import type { RuVectorClient } from '../client.js';
export interface PoincareDistanceOptions {
    a: string;
    b: string;
    curvature?: string;
}
export interface LorentzDistanceOptions {
    a: string;
    b: string;
    curvature?: string;
}
export interface MobiusAddOptions {
    a: string;
    b: string;
    curvature?: string;
}
export interface ExpMapOptions {
    base: string;
    tangent: string;
    curvature?: string;
}
export interface LogMapOptions {
    base: string;
    target: string;
    curvature?: string;
}
export interface ConvertOptions {
    vector: string;
    curvature?: string;
}
export declare class HyperbolicCommands {
    static poincareDistance(client: RuVectorClient, options: PoincareDistanceOptions): Promise<void>;
    static lorentzDistance(client: RuVectorClient, options: LorentzDistanceOptions): Promise<void>;
    static mobiusAdd(client: RuVectorClient, options: MobiusAddOptions): Promise<void>;
    static expMap(client: RuVectorClient, options: ExpMapOptions): Promise<void>;
    static logMap(client: RuVectorClient, options: LogMapOptions): Promise<void>;
    static poincareToLorentz(client: RuVectorClient, options: ConvertOptions): Promise<void>;
    static lorentzToPoincare(client: RuVectorClient, options: ConvertOptions): Promise<void>;
    static minkowskiDot(client: RuVectorClient, a: string, b: string): Promise<void>;
    static showHelp(): void;
}
export default HyperbolicCommands;
//# sourceMappingURL=hyperbolic.d.ts.map