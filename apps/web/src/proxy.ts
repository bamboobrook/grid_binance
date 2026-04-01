import type { NextRequest } from "next/server";
import { NextResponse } from "next/server";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const SESSION_TOKEN_COOKIE = "session_token";
const DEFAULT_SESSION_TOKEN_SECRET = "grid-binance-dev-session-secret";

type SessionClaims = {
  email: string;
  is_admin: boolean;
  sid: number;
};

type ProfileSnapshot = {
  admin_access_granted: boolean;
};

export async function proxy(request: NextRequest) {
  const sessionToken = request.cookies.get(SESSION_TOKEN_COOKIE)?.value;
  if (!sessionToken) {
    return redirectToLogin(request);
  }

  const claims = await verifySessionToken(sessionToken);
  if (!claims) {
    return redirectToLogin(request);
  }

  const profile = await fetchProfileSnapshot(sessionToken);
  if (!profile) {
    return redirectToLogin(request);
  }

  if (request.nextUrl.pathname.startsWith("/admin/") && !profile.admin_access_granted) {
    return redirectToLogin(request);
  }

  return NextResponse.next();
}

function redirectToLogin(request: NextRequest) {
  const loginUrl = new URL("/login", request.url);
  loginUrl.searchParams.set(
    "next",
    `${request.nextUrl.pathname}${request.nextUrl.search}`,
  );

  return NextResponse.redirect(loginUrl);
}

async function verifySessionToken(token: string): Promise<SessionClaims | null> {
  const [version, payload, signature, ...rest] = token.split(".");
  if (version !== "v1" || !payload || !signature || rest.length > 0) {
    return null;
  }

  const signedValue = `${version}.${payload}`;
  const expectedSignature = await signValue(signedValue);
  if (
    !timingSafeEqual(
      decodeBase64Url(signature),
      decodeBase64Url(expectedSignature),
    )
  ) {
    return null;
  }

  try {
    const claims = JSON.parse(
      new TextDecoder().decode(decodeBase64Url(payload)),
    ) as Partial<SessionClaims>;

    if (
      typeof claims.email !== "string" ||
      typeof claims.is_admin !== "boolean" ||
      typeof claims.sid !== "number"
    ) {
      return null;
    }

    return {
      email: claims.email,
      is_admin: claims.is_admin,
      sid: claims.sid,
    };
  } catch {
    return null;
  }
}

async function fetchProfileSnapshot(
  sessionToken: string,
): Promise<ProfileSnapshot | null> {
  try {
    const response = await fetch(`${authApiBaseUrl()}/profile`, {
      method: "GET",
      headers: {
        authorization: `Bearer ${sessionToken}`,
      },
      cache: "no-store",
    });
    if (!response.ok) {
      return null;
    }

    const payload = (await response.json()) as Partial<ProfileSnapshot>;
    if (typeof payload.admin_access_granted !== "boolean") {
      return null;
    }

    return {
      admin_access_granted: payload.admin_access_granted,
    };
  } catch {
    return null;
  }
}

async function signValue(value: string) {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(
      process.env.SESSION_TOKEN_SECRET ?? DEFAULT_SESSION_TOKEN_SECRET,
    ),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(value),
  );
  return encodeBase64Url(new Uint8Array(signature));
}

function decodeBase64Url(value: string) {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = normalized + "=".repeat((4 - (normalized.length % 4)) % 4);
  const decoded = atob(padded);
  return Uint8Array.from(decoded, (char) => char.charCodeAt(0));
}

function encodeBase64Url(bytes: Uint8Array) {
  const encoded = btoa(String.fromCharCode(...bytes));
  return encoded.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function timingSafeEqual(left: Uint8Array, right: Uint8Array) {
  if (left.length !== right.length) {
    return false;
  }

  let mismatch = 0;
  for (let index = 0; index < left.length; index += 1) {
    mismatch |= left[index] ^ right[index];
  }

  return mismatch === 0;
}

function authApiBaseUrl() {
  return (
    process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") ||
    DEFAULT_AUTH_API_BASE_URL
  );
}

export const config = {
  matcher: ["/app/:path*", "/admin/:path*"],
};
