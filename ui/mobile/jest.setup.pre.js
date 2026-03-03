// Pre-define globals that expo/src/winter/runtime.native.ts would lazily
// install via require()-with-ESM-import, which jest 30 rejects.
// By defining them upfront as non-configurable, the `install()` function
// in installGlobal.ts will skip them with a console.error (which is harmless).
const globalsToProtect = [
  'TextDecoder',
  'TextDecoderStream',
  'TextEncoderStream',
  'URL',
  'URLSearchParams',
  '__ExpoImportMetaRegistry',
  'structuredClone',
];

for (const name of globalsToProtect) {
  if (globalThis[name] !== undefined) {
    // Already defined (e.g. Node provides URL, TextDecoder, structuredClone).
    // Make it non-configurable so expo's install() skips it.
    try {
      Object.defineProperty(globalThis, name, {
        value: globalThis[name],
        configurable: false,
        enumerable: true,
        writable: true,
      });
    } catch {
      // Already non-configurable, fine.
    }
  } else {
    // Not yet defined, set a stub value and make non-configurable.
    Object.defineProperty(globalThis, name, {
      value: name === '__ExpoImportMetaRegistry' ? { url: 'http://localhost:8081' } : undefined,
      configurable: false,
      enumerable: false,
      writable: true,
    });
  }
}
