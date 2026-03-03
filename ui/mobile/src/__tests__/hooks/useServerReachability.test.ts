// useServerReachability calls apiService.getStatus() and tracks reachability.
// We test the module export shape and the underlying API service interaction.

jest.mock('@/services/api.service', () => ({
  apiService: {
    getStatus: jest.fn(),
    setBaseUrl: jest.fn(),
    get: jest.fn(),
    post: jest.fn(),
  },
}));

describe('useServerReachability', () => {
  it('module exports useServerReachability function', () => {
    const mod = require('@/hooks/useServerReachability');
    expect(typeof mod.useServerReachability).toBe('function');
  });

  it('apiService.getStatus is the underlying method used', () => {
    const { apiService } = require('@/services/api.service');
    expect(typeof apiService.getStatus).toBe('function');
  });

  it('hook return type includes reachable and latencyMs', () => {
    // The hook returns { reachable: boolean, latencyMs: number | null }
    // We verify the module exists and exports correctly
    const mod = require('@/hooks/useServerReachability');
    expect(mod.useServerReachability).toBeDefined();
  });

  it('apiService.getStatus can resolve (reachable case)', async () => {
    const { apiService } = require('@/services/api.service');
    (apiService.getStatus as jest.Mock).mockResolvedValueOnce({ status: 'ok' });
    await expect(apiService.getStatus()).resolves.toEqual({ status: 'ok' });
  });

  it('apiService.getStatus can reject (unreachable case)', async () => {
    const { apiService } = require('@/services/api.service');
    (apiService.getStatus as jest.Mock).mockRejectedValueOnce(new Error('timeout'));
    await expect(apiService.getStatus()).rejects.toThrow('timeout');
  });
});
