/// <reference types="vitest/config" />
import { defineConfig } from 'vite';

export default defineConfig({
  // Relative stier, slik at appen fungerer under /creature-forge/ på GitHub Pages.
  base: './',
  worker: { format: 'es' },
  // Enhetstester; nettlesertestene i e2e/ kjøres med Playwright.
  test: { include: ['test/**/*.test.ts'] },
  build: { target: 'es2022', chunkSizeWarningLimit: 1000 }, // Three.js alene er ca. 500 kB.
});
