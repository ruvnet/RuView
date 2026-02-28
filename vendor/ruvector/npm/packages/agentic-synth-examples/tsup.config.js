"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const tsup_1 = require("tsup");
exports.default = (0, tsup_1.defineConfig)({
    entry: {
        index: 'src/index.ts',
        'dspy/index': 'src/dspy/index.ts'
    },
    format: ['esm', 'cjs'],
    dts: true,
    clean: true,
    splitting: false,
    sourcemap: true,
    minify: false,
    target: 'es2022',
    outDir: 'dist',
    tsconfig: './tsconfig.json'
});
//# sourceMappingURL=tsup.config.js.map