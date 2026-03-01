"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const config_1 = require("vitest/config");
exports.default = (0, config_1.defineConfig)({
    test: {
        globals: true,
        environment: 'node',
        coverage: {
            provider: 'v8',
            reporter: ['text', 'json', 'html', 'lcov'],
            exclude: [
                'node_modules/**',
                'dist/**',
                'coverage/**',
                'tests/**',
                '**/*.test.ts',
                '**/*.test.js',
                '**/*.config.ts',
                '**/*.config.js',
                'benchmarks/**',
                'examples/**',
                'docs/**'
            ],
            include: ['src/**/*.ts', 'training/**/*.ts'],
            all: true,
            lines: 80,
            functions: 80,
            branches: 80,
            statements: 80
        },
        testTimeout: 10000,
        hookTimeout: 10000
    }
});
//# sourceMappingURL=vitest.config.js.map