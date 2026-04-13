import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "../../tests/e2e",
  use: {
    baseURL: "http://localhost:13000",
  },
  webServer: [
    {
      command:
        '. "$HOME/.cargo/env" && APP_ENV=test DATABASE_URL=postgres://postgres:postgres@127.0.0.1:15432/grid_binance REDIS_URL=redis://127.0.0.1:16379/0 SESSION_TOKEN_SECRET=grid-binance-dev-session-secret ADMIN_EMAILS=admin@example.com SUPER_ADMIN_EMAILS=admin-app-super@example.com,admin-commercial-super@example.com PORT=18080 cargo run -p api-server',
      url: "http://127.0.0.1:18080/healthz",
      reuseExistingServer: false,
      timeout: 120 * 1000,
    },
    {
      command:
        "pnpm --filter web build && AUTH_API_BASE_URL=http://127.0.0.1:18080 SESSION_TOKEN_SECRET=grid-binance-dev-session-secret pnpm --filter web exec next start --hostname localhost --port 13000",
      url: "http://localhost:13000",
      reuseExistingServer: false,
      timeout: 120 * 1000,
    },
  ],
});
