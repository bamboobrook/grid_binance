import { NextResponse } from "next/server";
import { localizedPath, localizedPublicPath, publicUrl } from "@/lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const name = readField(formData, "name");
  const templateId = readField(formData, "templateId");
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, "/login?error=session+expired")), { status: 303 });
  }
  if (!templateId || !name) {
    return NextResponse.redirect(publicUrl(request, localizedPath(request, "/app/strategies/new?error=Template+and+strategy+name+are+required.")), { status: 303 });
  }

  const response = await fetch(`${authApiBaseUrl()}/strategies/templates/${templateId}/apply`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ name }),
    cache: "no-store",
  });
  if (!response.ok) {
    return NextResponse.redirect(publicUrl(request, localizedPath(request, `/app/strategies/new?error=${encodeURIComponent(await readError(response))}`)), { status: 303 });
  }

  const created = (await response.json()) as { id: string };
  return NextResponse.redirect(publicUrl(request, localizedPath(request, `/app/strategies/${created.id}?notice=template-applied`)), { status: 303 });
}

function readField(formData: FormData, key: string) {
  const value = formData.get(key);
  return typeof value === "string" ? value.trim() : "";
}

async function readError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "strategy request failed";
  } catch {
    return "strategy request failed";
  }
}

function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
