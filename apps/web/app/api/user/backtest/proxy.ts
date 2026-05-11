const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function proxyBacktestRequest(
  request: Request,
  options: {
    backendPath: string;
    method?: string;
    requireSession?: boolean;
  },
) {
  const sessionToken = readSessionToken(request);
  const inboundAuthorization = request.headers.get("authorization");
  const requireSession = options.requireSession ?? true;

  if (requireSession && !sessionToken && !inboundAuthorization) {
    return Response.json({ error: "Not authenticated" }, { status: 401 });
  }

  const response = await fetch(buildBackendUrl(request, options.backendPath), {
    method: options.method ?? request.method,
    headers: buildProxyHeaders(request, sessionToken),
    body: await readProxyBody(request, options.method ?? request.method),
    cache: "no-store",
  });

  return relayProxyResponse(response);
}

export function readSessionToken(request: Request) {
  const cookie = request.headers.get("cookie") ?? "";
  const match = cookie.match(/(?:^|; )session_token=([^;]+)/);
  return match?.[1] ?? "";
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

function buildBackendUrl(request: Request, backendPath: string) {
  const incomingUrl = new URL(request.url);
  return `${authApiBaseUrl()}${backendPath}${incomingUrl.search}`;
}

function buildProxyHeaders(request: Request, sessionToken: string) {
  const headers = new Headers();
  const contentType = request.headers.get("content-type");
  const cookie = request.headers.get("cookie");
  const authorization = request.headers.get("authorization");
  const accept = request.headers.get("accept");

  if (contentType) {
    headers.set("content-type", contentType);
  }
  if (cookie) {
    headers.set("cookie", cookie);
  }
  if (accept) {
    headers.set("accept", accept);
  }
  if (authorization) {
    headers.set("authorization", authorization);
  } else if (sessionToken) {
    headers.set("authorization", `Bearer ${sessionToken}`);
  }

  return headers;
}

async function readProxyBody(request: Request, method: string) {
  if (method === "GET" || method === "HEAD") {
    return undefined;
  }

  const text = await request.text();
  return text.length > 0 ? text : undefined;
}

async function relayProxyResponse(response: Response) {
  const text = await response.text();
  const contentType = response.headers.get("content-type") ?? "";
  const headers = new Headers();

  if (contentType) {
    headers.set("content-type", contentType);
  } else if (text) {
    headers.set("content-type", looksLikeJson(text) ? "application/json; charset=utf-8" : "text/plain; charset=utf-8");
  }

  if (!text) {
    return new Response(null, {
      status: response.status,
      headers,
    });
  }

  if (looksLikeJson(text)) {
    try {
      JSON.parse(text);
      return new Response(text, {
        status: response.status,
        headers,
      });
    } catch {
      // Fall through and return text verbatim if upstream mislabeled the payload.
    }
  }

  return new Response(text, {
    status: response.status,
    headers,
  });
}

function looksLikeJson(text: string) {
  const trimmed = text.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}
