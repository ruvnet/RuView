// HudOverlay.tsx is an empty file (0 bytes). This test verifies that importing
// it does not throw and that the module exists.

describe('HudOverlay', () => {
  it('module can be imported without error', () => {
    expect(() => {
      require('@/components/HudOverlay');
    }).not.toThrow();
  });

  it('module exports are defined (may be empty)', () => {
    const mod = require('@/components/HudOverlay');
    // The module is empty, so it should be an object (possibly with no exports)
    expect(typeof mod).toBe('object');
  });
});
