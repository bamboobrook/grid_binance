import { createHmac } from "node:crypto";

const SESSION_TOKEN_SECRET =
  process.env.SESSION_TOKEN_SECRET ?? "grid-binance-dev-session-secret";

type SessionClaims = {
  email: string;
  is_admin: boolean;
  sid?: number;
};

export function createSessionToken({
  sid = 1,
  ...claims
}: SessionClaims): string {
  const payload = Buffer.from(
    JSON.stringify({
      ...claims,
      sid,
    }),
  ).toString("base64url");
  const signed = `v1.${payload}`;
  const signature = createHmac("sha256", SESSION_TOKEN_SECRET)
    .update(signed)
    .digest("base64url");

  return `${signed}.${signature}`;
}
