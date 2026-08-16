/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import dts from 'vite-plugin-dts';

export default defineConfig({
  plugins: [
    vue(),
    dts({ include: ['src'] }),
  ],
  test: {
    // Default environment. Per-file DOM needs are declared with a
    // `@vitest-environment` docblock at the top of the file — Vitest 4 removed
    // `environmentMatchGlobs`, so config-side matching is silently ignored.
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
  build: {
    lib: {
      entry: 'src/index.ts',
      name: 'ArcVue',
      fileName: 'arc-vue',
      formats: ['es', 'umd'],
    },
    rolldownOptions: {
      external: ['vue', '@koyori-app/arc'],
      output: {
        globals: {
          vue: 'Vue',
          '@koyori-app/arc': 'KoyoriArc',
        },
      },
    },
  },
});
