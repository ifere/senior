import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
    test: {
        globals: true,
        environment: 'node',
        include: ['src/__tests__/integration/**/*.test.ts'],
        testTimeout: 15_000, // daemon startup can take a moment
        alias: {
            vscode: path.resolve(__dirname, '__mocks__/vscode.ts'),
        },
    },
});
