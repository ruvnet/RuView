/**
 * Core type definitions for the sublinear-time solver
 */
// Error types
export class SolverError extends Error {
    code;
    details;
    constructor(message, code, details) {
        super(message);
        this.code = code;
        this.details = details;
        this.name = 'SolverError';
    }
}
export const ErrorCodes = {
    NOT_DIAGONALLY_DOMINANT: 'E001',
    CONVERGENCE_FAILED: 'E002',
    INVALID_MATRIX: 'E003',
    TIMEOUT: 'E004',
    INVALID_DIMENSIONS: 'E005',
    NUMERICAL_INSTABILITY: 'E006',
    MEMORY_LIMIT_EXCEEDED: 'E007',
    INVALID_PARAMETERS: 'E008'
};
