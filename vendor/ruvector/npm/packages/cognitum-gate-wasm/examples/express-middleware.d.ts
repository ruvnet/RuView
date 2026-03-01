/**
 * Express Middleware Example
 *
 * This example shows how to use Cognitum Gate as Express middleware
 * to protect API endpoints with coherence-based access control.
 *
 * Run with: npx ts-node examples/express-middleware.ts
 */
declare module 'express' {
    interface Request {
        gateToken?: string;
        gateReceipt?: number;
    }
}
export {};
//# sourceMappingURL=express-middleware.d.ts.map