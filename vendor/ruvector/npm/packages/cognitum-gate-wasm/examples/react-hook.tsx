/**
 * React Hook Example
 *
 * This example shows how to use Cognitum Gate in React applications
 * with a custom hook for action permission.
 *
 * Usage in your React app:
 *   import { useGate, GateProvider } from './react-hook';
 */

import React, { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { CognitumGate, GateDecision, ActionContext, PermitResult } from '@cognitum/gate';

// Gate Context
interface GateContextValue {
  gate: CognitumGate | null;
  isReady: boolean;
  permitAction: (action: ActionContext) => Promise<PermitResult>;
  pendingActions: Map<number, ActionContext>;
}

const GateContext = createContext<GateContextValue | null>(null);

// Gate Provider
interface GateProviderProps {
  children: ReactNode;
  config?: {
    minCut?: number;
    maxShift?: number;
    storage?: 'memory' | 'indexeddb';
  };
}

export function GateProvider({ children, config }: GateProviderProps) {
  const [gate, setGate] = useState<CognitumGate | null>(null);
  const [isReady, setIsReady] = useState(false);
  const [pendingActions] = useState(new Map<number, ActionContext>());

  useEffect(() => {
    CognitumGate.init({
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

  const permitAction = useCallback(async (action: ActionContext) => {
    if (!gate) throw new Error('Gate not initialized');
    const result = await gate.permitAction(action);

    if (result.decision === GateDecision.Defer) {
      pendingActions.set(result.receiptSequence, action);
    }

    return result;
  }, [gate, pendingActions]);

  return (
    <GateContext.Provider value={{ gate, isReady, permitAction, pendingActions }}>
      {children}
    </GateContext.Provider>
  );
}

// useGate Hook
export function useGate() {
  const context = useContext(GateContext);
  if (!context) {
    throw new Error('useGate must be used within a GateProvider');
  }
  return context;
}

// usePermitAction Hook - simplified action permission
export function usePermitAction() {
  const { permitAction, isReady } = useGate();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [lastResult, setLastResult] = useState<PermitResult | null>(null);

  const requestPermit = useCallback(async (action: ActionContext) => {
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
    } catch (e) {
      setError(e as Error);
      return null;
    } finally {
      setIsLoading(false);
    }
  }, [permitAction, isReady]);

  return { requestPermit, isLoading, error, lastResult, isReady };
}

// Example Component: Protected Button
interface ProtectedButtonProps {
  actionId: string;
  actionType: string;
  target: string;
  onPermitted: (token: string) => void;
  onDeferred: (sequence: number) => void;
  onDenied: (reason: string) => void;
  children: ReactNode;
}

export function ProtectedButton({
  actionId,
  actionType,
  target,
  onPermitted,
  onDeferred,
  onDenied,
  children,
}: ProtectedButtonProps) {
  const { requestPermit, isLoading, error } = usePermitAction();

  const handleClick = async () => {
    const result = await requestPermit({
      actionId,
      actionType,
      agentId: 'web-user',
      target,
      metadata: { timestamp: Date.now() },
    });

    if (!result) return;

    switch (result.decision) {
      case GateDecision.Permit:
        onPermitted(result.token);
        break;
      case GateDecision.Defer:
        onDeferred(result.receiptSequence);
        break;
      case GateDecision.Deny:
        onDenied(result.reason || 'Action denied');
        break;
    }
  };

  return (
    <button onClick={handleClick} disabled={isLoading}>
      {isLoading ? 'Checking...' : children}
      {error && <span className="error">{error.message}</span>}
    </button>
  );
}

// Example App
export function ExampleApp() {
  const [status, setStatus] = useState<string>('');

  return (
    <GateProvider config={{ storage: 'indexeddb' }}>
      <div className="app">
        <h1>Cognitum Gate - React Example</h1>

        <ProtectedButton
          actionId="deploy-button"
          actionType="deployment"
          target="production"
          onPermitted={(token) => {
            setStatus(`✅ Permitted! Token: ${token.slice(0, 20)}...`);
          }}
          onDeferred={(seq) => {
            setStatus(`⏸️ Deferred - Human review needed (seq: ${seq})`);
          }}
          onDenied={(reason) => {
            setStatus(`❌ Denied: ${reason}`);
          }}
        >
          Deploy to Production
        </ProtectedButton>

        <p>{status}</p>

        <AuditLog />
      </div>
    </GateProvider>
  );
}

// Audit Log Component
function AuditLog() {
  const { gate, isReady } = useGate();
  const [receipts, setReceipts] = useState<any[]>([]);

  useEffect(() => {
    if (isReady && gate) {
      gate.getReceipts(0, 10).then(setReceipts);
    }
  }, [gate, isReady]);

  return (
    <div className="audit-log">
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
          {receipts.map((r) => (
            <tr key={r.sequence}>
              <td>{r.sequence}</td>
              <td>{r.token.actionId}</td>
              <td>{r.token.decision}</td>
              <td>{new Date(r.token.timestamp / 1_000_000).toLocaleString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default ExampleApp;
