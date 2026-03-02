import { useEffect, useState } from 'react';
import { apiService } from '@/services/api.service';

interface ServerReachability {
  reachable: boolean;
  latencyMs: number | null;
}

const POLL_MS = 10000;

export function useServerReachability(): ServerReachability {
  const [state, setState] = useState<ServerReachability>({
    reachable: false,
    latencyMs: null,
  });

  useEffect(() => {
    let active = true;

    const check = async () => {
      const started = Date.now();
      try {
        await apiService.getStatus();
        if (!active) {
          return;
        }
        setState({
          reachable: true,
          latencyMs: Date.now() - started,
        });
      } catch {
        if (!active) {
          return;
        }
        setState({
          reachable: false,
          latencyMs: null,
        });
      }
    };

    void check();
    const timer = setInterval(check, POLL_MS);

    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);

  return state;
}
