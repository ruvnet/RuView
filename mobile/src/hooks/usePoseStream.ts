import { useEffect } from 'react';
import { wsService } from '@/services/ws.service';
import { usePoseStore } from '@/stores/poseStore';

export interface UsePoseStreamResult {
  connectionStatus: ReturnType<typeof usePoseStore.getState>['connectionStatus'];
  lastFrame: ReturnType<typeof usePoseStore.getState>['lastFrame'];
  isSimulated: boolean;
}

export function usePoseStream(): UsePoseStreamResult {
  const connectionStatus = usePoseStore((state) => state.connectionStatus);
  const lastFrame = usePoseStore((state) => state.lastFrame);
  const isSimulated = usePoseStore((state) => state.isSimulated);

  useEffect(() => {
    const unsubscribe = wsService.subscribe((frame) => {
      usePoseStore.getState().handleFrame(frame);
    });

    return () => {
      unsubscribe();
    };
  }, []);

  return { connectionStatus, lastFrame, isSimulated };
}
