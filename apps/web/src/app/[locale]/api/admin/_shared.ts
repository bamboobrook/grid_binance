import { NextResponse } from "next/server";

import { authApiBaseUrl } from "../../../../lib/api/admin-product-state";


export function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

export function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

export function redirectTo(request: Request, path: string) {
  return NextResponse.redirect(new URL(path, request.url), { status: 303 });
}

export async function postAdminBackend(request: Request, path: string, body: Record<string, unknown>) {
  const sessionToken = readSessionToken(request) ?? "";
  return fetch(`${authApiBaseUrl()}${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
    cache: "no-store",
  });
}

export async function proxyAdminBackendError(response: Response) {
  const contentType = response.headers.get("content-type");
  const headers = new Headers();
  if (contentType) {
    headers.set("content-type", contentType);
  }

  return new Response(await response.text(), {
    status: response.status,
    headers,
  });
}
