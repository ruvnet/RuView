"use strict";
/**
 * React Hook Example
 *
 * This example shows how to use Cognitum Gate in React applications
 * with a custom hook for action permission.
 *
 * Usage in your React app:
 *   import { useGate, GateProvider } from './react-hook';
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.GateProvider = GateProvider;
exports.useGate = useGate;
exports.usePermitAction = usePermitAction;
exports.ProtectedButton = ProtectedButton;
exports.ExampleApp = ExampleApp;
const react_1 = __importStar(require("react"));
const gate_1 = require("@cognitum/gate");
const GateContext = (0, react_1.createContext)(null);
function GateProvider({ children, config }) {
    const [gate, setGate] = (0, react_1.useState)(null);
    const [isReady, setIsReady] = (0, react_1.useState)(false);
    const [pendingActions] = (0, react_1.useState)(new Map());
    (0, react_1.useEffect)(() => {
        gate_1.CognitumGate.init({
            thresholds: {
                minCut: config?.minCut ?? 10.0,
                maxShift: config?.maxShift ?? 0.5,
                eDeny: 0.01,
                ePermit: 100.0,
            },
            storage: config?.storage ?? 'indexeddb',
        }).then((g) => {
            setGate(g);
            setIsReady(true);
        });
    }, [config]);
    const permitAction = (0, react_1.useCallback)(async (action) => {
        if (!gate)
            throw new Error('Gate not initialized');
        const result = await gate.permitAction(action);
        if (result.decision === gate_1.GateDecision.Defer) {
            pendingActions.set(result.receiptSequence, action);
        }
        return result;
    }, [gate, pendingActions]);
    return (<GateContext.Provider value={{ gate, isReady, permitAction, pendingActions }}>
      {children}
    </GateContext.Provider>);
}
// useGate Hook
function useGate() {
    const context = (0, react_1.useContext)(GateContext);
    if (!context) {
        throw new Error('useGate must be used within a GateProvider');
    }
    return context;
}
// usePermitAction Hook - simplified action permission
function usePermitAction() {
    const { permitAction, isReady } = useGate();
    const [isLoading, setIsLoading] = (0, react_1.useState)(false);
    const [error, setError] = (0, react_1.useState)(null);
    const [lastResult, setLastResult] = (0, react_1.useState)(null);
    const requestPermit = (0, react_1.useCallback)(async (action) => {
        if (!isReady) {
            setError(new Error('Gate not ready'));
            return null;
        }
        setIsLoading(true);
        setError(null);
        try {
            const result = await permitAction(action);
            setLastResult(result);
            return result;
        }
        catch (e) {
            setError(e);
            return null;
        }
        finally {
            setIsLoading(false);
        }
    }, [permitAction, isReady]);
    return { requestPermit, isLoading, error, lastResult, isReady };
}
function ProtectedButton({ actionId, actionType, target, onPermitted, onDeferred, onDenied, children, }) {
    const { requestPermit, isLoading, error } = usePermitAction();
    const handleClick = async () => {
        const result = await requestPermit({
            actionId,
            actionType,
            agentId: 'web-user',
            target,
            metadata: { timestamp: Date.now() },
        });
        if (!result)
            return;
        switch (result.decision) {
            case gate_1.GateDecision.Permit:
                onPermitted(result.token);
                break;
            case gate_1.GateDecision.Defer:
                onDeferred(result.receiptSequence);
                break;
            case gate_1.GateDecision.Deny:
                onDenied(result.reason || 'Action denied');
                break;
        }
    };
    return (<button onClick={handleClick} disabled={isLoading}>
      {isLoading ? 'Checking...' : children}
      {error && <span className="error">{error.message}</span>}
    </button>);
}
// Example App
function ExampleApp() {
    const [status, setStatus] = (0, react_1.useState)('');
    return (<GateProvider config={{ storage: 'indexeddb' }}>
      <div className="app">
        <h1>Cognitum Gate - React Example</h1>

        <ProtectedButton actionId="deploy-button" actionType="deployment" target="production" onPermitted={(token) => {
            setStatus(`✅ Permitted! Token: ${token.slice(0, 20)}...`);
        }} onDeferred={(seq) => {
            setStatus(`⏸️ Deferred - Human review needed (seq: ${seq})`);
        }} onDenied={(reason) => {
            setStatus(`❌ Denied: ${reason}`);
        }}>
          Deploy to Production
        </ProtectedButton>

        <p>{status}</p>

        <AuditLog />
      </div>
    </GateProvider>);
}
// Audit Log Component
function AuditLog() {
    const { gate, isReady } = useGate();
    const [receipts, setReceipts] = (0, react_1.useState)([]);
    (0, react_1.useEffect)(() => {
        if (isReady && gate) {
            gate.getReceipts(0, 10).then(setReceipts);
        }
    }, [gate, isReady]);
    return (<div className="audit-log">
      <h2>Recent Decisions</h2>
      <table>
        <thead>
          <tr>
            <th>Seq</th>
            <th>Action</th>
            <th>Decision</th>
            <th>Time</th>
          </tr>
        </thead>
        <tbody>
          {receipts.map((r) => (<tr key={r.sequence}>
              <td>{r.sequence}</td>
              <td>{r.token.actionId}</td>
              <td>{r.token.decision}</td>
              <td>{new Date(r.token.timestamp / 1000000).toLocaleString()}</td>
            </tr>))}
        </tbody>
      </table>
    </div>);
}
exports.default = ExampleApp;
//# sourceMappingURL=react-hook.js.map