import { defineConfig } from 'vitest/config';
import path from 'path';

// Minimal Vitest configuration for the local-web package. We deliberately
// avoid pulling in the full Vite app config (Sentry plugin, tanstack router
// codegen, executor schemas virtual module) because tests don't need them
// and they slow startup considerably.
export default defineConfig({
  resolve: {
    alias: [
      {
        find: '@web',
        replacement: path.resolve(__dirname, 'src'),
      },
      {
        find: /^@\//,
        replacement: `${path.resolve(__dirname, '../web-core/src')}/`,
      },
      {
        find: 'shared',
        replacement: path.resolve(__dirname, '../../shared'),
      },
    ],
  },
  test: {
    // Tests for the cutover cache-purge hook need only globalThis-level
    // browser APIs (indexedDB, localStorage) which we polyfill in-test via
    // fake-indexeddb and a hand-rolled localStorage stub. A full DOM is
    // unnecessary, so stay on the lighter-weight node environment.
    environment: 'node',
    globals: false,
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
  },
});
