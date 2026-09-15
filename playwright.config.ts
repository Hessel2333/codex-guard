import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/ui',
  use: { channel: 'msedge', baseURL: 'http://localhost:1420', viewport: { width: 1100, height: 748 } },
  webServer: { command: 'node node_modules/vite/bin/vite.js --configLoader runner', url: 'http://localhost:1420', reuseExistingServer: !process.env.CI },
  reporter: 'list',
});
