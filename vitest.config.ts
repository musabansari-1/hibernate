import { defineConfig } from 'vitest/config';

export default defineConfig({
  // Integration fixtures include complete editor installations with their own tests.
  test: { include: ['tests/**/*.test.ts'] },
});
