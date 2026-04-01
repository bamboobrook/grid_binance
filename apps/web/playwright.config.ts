import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "../../tests/e2e",
  use: {
    baseURL: "http://localhost:3000",
  },
  webServer: {
    command: "pnpm --filter web build && pnpm --filter web exec next start --hostname localhost --port 3000",
    url: "http://localhost:3000",
    reuseExistingServer: false,
  },
});
