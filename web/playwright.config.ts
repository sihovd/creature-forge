import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: 'e2e',
  use: { baseURL: 'http://localhost:4173' },
  webServer: {
    command: 'npm run preview',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
  },
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'] } },
    // Telefoner støttes ikke; berøring testes på iPad Pro (Chromium, for CDP-touch).
    { name: 'ipad', use: { ...devices['iPad Pro 11'], browserName: 'chromium' } },
    { name: 'telefon', use: { ...devices['Pixel 7'] }, testMatch: /small-screen\.spec\.ts/ },
  ],
});
