import { defineConfig } from 'vite';

export default defineConfig({
  // Relative stier, slik at appen fungerer under /creature-forge/ på GitHub Pages.
  base: './',
  worker: { format: 'es' },
  build: { target: 'es2022', chunkSizeWarningLimit: 1000 }, // Three.js alene er ca. 500 kB.
});
