"use strict";
/**
 * Express Middleware Example
 *
 * This example shows how to use Cognitum Gate as Express middleware
 * to protect API endpoints with coherence-based access control.
 *
 * Run with: npx ts-node examples/express-middleware.ts
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const express_1 = __importDefault(require("express"));
const gate_1 = require("@cognitum/gate");
// Initialize the gate (singleton)
let gate;
async function initGate() {
    gate = await gate_1.CognitumGate.init({
        thresholds: {
            minCut: 10.0,
            maxShift: 0.5,
            eDeny: 0.01,
            ePermit: 100.0,
        },
        storage: 'memory',
    });
}
/**
 * Gate middleware factory
 * Creates middleware that checks coherence before allowing actions
 */
function gateMiddleware(actionType) {
    return async (req, res, next) => {
        const action = {
            actionId: `${req.method}-${req.path}-${Date.now()}`,
            actionType,
            agentId: req.headers['x-agent-id'] || 'anonymous',
            target: req.path,
            metadata: {
                method: req.method,
                ip: req.ip,
                userAgent: req.headers['user-agent'],
            },
        };
        try {
            const result = await gate.permitAction(action);
            switch (result.decision) {
                case gate_1.GateDecision.Permit:
                    // Attach token and continue
                    req.gateToken = result.token;
                    req.gateReceipt = result.receiptSequence;
                    next();
                    break;
                case gate_1.GateDecision.Defer:
                    // Return 202 Accepted with escalation info
                    res.status(202).json({
                        status: 'deferred',
                        message: 'Human approval required',
                        escalation: {
                            url: result.escalation?.contextUrl,
                            timeout: result.escalation?.timeoutNs,
                        },
                        receiptSequence: result.receiptSequence,
                    });
                    break;
                case gate_1.GateDecision.Deny:
                    // Return 403 Forbidden with witness
                    res.status(403).json({
                        status: 'denied',
                        reason: result.reason,
                        witness: {
                            structural: result.witness?.structural,
                            evidential: result.witness?.evidential,
                        },
                        receiptSequence: result.receiptSequence,
                    });
                    break;
            }
        }
        catch (error) {
            // Gate error - fail closed
            res.status(500).json({
                status: 'error',
                message: 'Gate evaluation failed',
            });
        }
    };
}
// Create Express app
const app = (0, express_1.default)();
app.use(express_1.default.json());
// Public endpoints (no gate)
app.get('/health', (req, res) => {
    res.json({ status: 'healthy' });
});
// Protected read endpoint
app.get('/api/config/:id', gateMiddleware('config_read'), (req, res) => {
    res.json({
        id: req.params.id,
        value: 'some-config-value',
        _gateReceipt: req.gateReceipt,
    });
});
// Protected write endpoint (higher scrutiny)
app.post('/api/config/:id', gateMiddleware('config_write'), (req, res) => {
    res.json({
        id: req.params.id,
        updated: true,
        _gateReceipt: req.gateReceipt,
    });
});
// Critical endpoint (deployment)
app.post('/api/deploy', gateMiddleware('deployment'), (req, res) => {
    res.json({
        deployed: true,
        version: req.body.version,
        _gateReceipt: req.gateReceipt,
    });
});
// Audit endpoint
app.get('/api/audit/receipts', async (req, res) => {
    const from = parseInt(req.query.from) || 0;
    const limit = parseInt(req.query.limit) || 100;
    const receipts = await gate.getReceipts(from, limit);
    res.json({
        receipts,
        chainValid: await gate.verifyChain(),
    });
});
// Start server
async function main() {
    await initGate();
    const PORT = process.env.PORT || 3000;
    app.listen(PORT, () => {
        console.log(`Cognitum Gate Express example listening on port ${PORT}`);
        console.log(`
Endpoints:
  GET  /health              - Health check (no gate)
  GET  /api/config/:id      - Read config (gated)
  POST /api/config/:id      - Write config (gated)
  POST /api/deploy          - Deploy (gated, high scrutiny)
  GET  /api/audit/receipts  - Audit trail

Test with:
  curl http://localhost:${PORT}/api/config/123 -H "X-Agent-Id: test-agent"
    `);
    });
}
main().catch(console.error);
//# sourceMappingURL=express-middleware.js.map