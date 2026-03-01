/**
 * React Hook Example
 *
 * This example shows how to use Cognitum Gate in React applications
 * with a custom hook for action permission.
 *
 * Usage in your React app:
 *   import { useGate, GateProvider } from './react-hook';
 */
import { ReactNode } from 'react';
interface GateProviderProps {
    children: ReactNode;
    config?: {
        minCut?: number;
        maxShift?: number;
        storage?: 'memory' | 'indexeddb';
    };
}
export declare function GateProvider({ children, config }: GateProviderProps): any;
export declare function useGate(): any;
export declare function usePermitAction(): {
    requestPermit: any;
    isLoading: any;
    error: any;
    lastResult: any;
    isReady: any;
};
interface ProtectedButtonProps {
    actionId: string;
    actionType: string;
    target: string;
    onPermitted: (token: string) => void;
    onDeferred: (sequence: number) => void;
    onDenied: (reason: string) => void;
    children: ReactNode;
}
export declare function ProtectedButton({ actionId, actionType, target, onPermitted, onDeferred, onDenied, children, }: ProtectedButtonProps): any;
export declare function ExampleApp(): any;
export default ExampleApp;
//# sourceMappingURL=react-hook.d.ts.map