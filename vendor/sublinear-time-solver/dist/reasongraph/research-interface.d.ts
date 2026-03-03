/**
 * ReasonGraph Research Interface
 * Web-based research platform for scientific discovery acceleration
 * Provides intuitive access to advanced reasoning capabilities
 */
import { ReasoningResult } from './advanced-reasoning-engine.js';
export interface ResearchProject {
    id: string;
    name: string;
    domain: string;
    questions: string[];
    results: ReasoningResult[];
    created_at: number;
    updated_at: number;
    status: 'active' | 'completed' | 'paused';
    breakthrough_count: number;
}
export interface ResearchSession {
    session_id: string;
    user_id: string;
    projects: ResearchProject[];
    settings: {
        default_creativity_level: number;
        enable_temporal_advantage: boolean;
        enable_consciousness_verification: boolean;
        default_reasoning_depth: number;
    };
}
export declare class ReasonGraphResearchInterface {
    private reasoningEngine;
    private app;
    private sessions;
    private projects;
    constructor();
    private setupMiddleware;
    private setupRoutes;
    private logResearchQuery;
    start(port?: number): Promise<void>;
    stop(): Promise<void>;
}
export default ReasonGraphResearchInterface;
