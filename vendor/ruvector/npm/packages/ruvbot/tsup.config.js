"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const tsup_1 = require("tsup");
exports.default = (0, tsup_1.defineConfig)([
    // Main bundle
    {
        entry: ['src/index.ts'],
        format: ['cjs', 'esm'],
        dts: true,
        clean: true,
        sourcemap: true,
        splitting: false,
        treeshake: true,
        minify: false,
        target: 'node18',
        outDir: 'dist',
    },
    // Subpath exports
    {
        entry: {
            'core/index': 'src/core/index.ts',
            'learning/index': 'src/learning/index.ts',
            'skills/index': 'src/skills/index.ts',
            'integrations/index': 'src/integrations/index.ts',
            'api/index': 'src/api/index.ts',
            'cli/index': 'src/cli/index.ts',
        },
        format: ['cjs', 'esm'],
        dts: true,
        sourcemap: true,
        splitting: false,
        treeshake: true,
        target: 'node18',
        outDir: 'dist',
    },
]);
//# sourceMappingURL=tsup.config.js.map