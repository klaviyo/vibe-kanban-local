import { defineConfig } from 'vitest/config';
import path from 'path';

// Vitest config for the @vibe/web-core package. Mirrors the alias map used
// by the local-web Vite app so colocated `*.test.ts` files can import via
// the same `@/...` and `shared/...` specifiers used at runtime.
export default defineConfig({
  resolve: {
    alias: [
      {
        find: /^@\//,
        replacement: `${path.resolve(__dirname, 'src')}/`,
      },
      {
        find: /^shared\//,
        replacement: `${path.resolve(__dirname, '../../shared')}/`,
      },
    ],
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
  },
});
