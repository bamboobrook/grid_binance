import { readFileSync, existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const composeArgs = [
  "compose",
  "-p",
  "grid-binance-e2e",
  "--env-file",
  ".env.example",
  "-f",
  "deploy/docker/docker-compose.yml",
  "-f",
  "deploy/docker/docker-compose.e2e.yml",
];

loadEnvFile(".env.example");
if (existsSync(".env")) {
  loadEnvFile(".env");
}

function loadEnvFile(path) {
  const content = readFileSync(path, "utf8");

  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }

    const separator = trimmed.indexOf("=");
    if (separator === -1) {
      continue;
    }

    const key = trimmed.slice(0, separator).trim();
    const value = trimmed.slice(separator + 1).trim();

    if (!process.env[key]) {
      process.env[key] = value;
    }
  }
}

function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    env: process.env,
  });

  if (typeof result.status === "number") {
    return result.status;
  }

  return 1;
}

function cleanup() {
  run("docker", [...composeArgs, "down", "-v"]);
}

cleanup();
process.on("exit", cleanup);
process.on("SIGINT", () => {
  cleanup();
  process.exit(130);
});
process.on("SIGTERM", () => {
  cleanup();
  process.exit(143);
});

const composeStatus = run("docker", [...composeArgs, "up", "-d", "--wait", "postgres", "redis"]);
if (composeStatus !== 0) {
  process.exit(composeStatus);
}

const playwrightStatus = run(
  "pnpm",
  [
    "exec",
    "playwright",
    "test",
    "--config",
    "apps/web/playwright.config.ts",
    ...process.argv.slice(2),
  ],
);

process.exit(playwrightStatus);
