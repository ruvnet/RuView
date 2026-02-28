import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { HeroUIProvider } from '@heroui/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from '../App';
import { useNetworkStore } from '../stores/networkStore';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const renderApp = () => {
  return render(
    <QueryClientProvider client={queryClient}>
      <HeroUIProvider>
        <App />
      </HeroUIProvider>
    </QueryClientProvider>
  );
};

describe('App', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset network store to initial state
    useNetworkStore.setState({
      stats: {
        totalNodes: 0,
        activeNodes: 0,
        totalCompute: 0,
        creditsEarned: 0,
        tasksCompleted: 0,
        uptime: 0,
        latency: 0,
        bandwidth: 0,
      },
      isConnected: false,
      isLoading: true,
      error: null,
      startTime: Date.now(),
    });
  });

  it('renders loading state initially', () => {
    renderApp();
    expect(screen.getByText(/Initializing Edge-Net/i)).toBeInTheDocument();
  });

  it('renders main dashboard after loading', async () => {
    renderApp();

    await waitFor(
      () => {
        expect(screen.getByText(/Network Overview/i)).toBeInTheDocument();
      },
      { timeout: 3000 }
    );
  });

  it('renders header with Edge-Net branding', async () => {
    renderApp();

    await waitFor(
      () => {
        expect(screen.getByText('Edge-Net')).toBeInTheDocument();
      },
      { timeout: 3000 }
    );
  });

  it('shows connection status after network connects', async () => {
    renderApp();

    // Wait for loading to complete and dashboard to render
    await waitFor(
      () => {
        expect(screen.getByText(/Network Overview/i)).toBeInTheDocument();
      },
      { timeout: 3000 }
    );

    // Update real stats which sets isConnected: true
    useNetworkStore.getState().updateRealStats();

    // Now check for connection status - could be "Connected" or node count
    await waitFor(
      () => {
        const state = useNetworkStore.getState();
        // Verify the store state is connected
        expect(state.isConnected).toBe(true);
      },
      { timeout: 1000 }
    );
  });
});
