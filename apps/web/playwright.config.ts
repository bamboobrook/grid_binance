import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "../../tests/e2e",
  use: {
    baseURL: "http://localhost:3000",
  },
  webServer: [
    {
      command:
        '. "$HOME/.cargo/env" && mkdir -p .tmp && rm -f .tmp/e2e-api.sqlite3 && APP_DB_PATH=.tmp/e2e-api.sqlite3 SESSION_TOKEN_SECRET=grid-binance-dev-session-secret ADMIN_EMAILS=admin@example.com PORT=8080 cargo run -p api-server',
      url: "http://127.0.0.1:8080/healthz",
      reuseExistingServer: false,
      timeout: 120 * 1000,
    },
    {
      command:
        "pnpm --filter web build && AUTH_API_BASE_URL=http://127.0.0.1:8080 SESSION_TOKEN_SECRET=grid-binance-dev-session-secret pnpm --filter web exec next start --hostname localhost --port 3000",
      url: "http://localhost:3000",
      reuseExistingServer: false,
      timeout: 120 * 1000,
    },
  ],
});
