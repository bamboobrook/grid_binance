import { NextResponse } from "next/server";

import { localizedAppPath, localizedPublicPath, publicUrl } from "../../../../../lib/auth";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const confirmed = readField(formData, "confirmAction") === "yes";
  const confirmDelete = readField(formData, "confirmDelete") === "yes";
  const view = readField(formData, "view");
  const ids = formData.getAll("ids").filter((value): value is string => typeof value === "string").map((value) => value.trim()).filter(Boolean);
  if (intent === "stop-all" && !confirmed) {
    return redirectApp(request, listPath({ confirmAction: "stop-all", view }));
  }
  if ((intent === "start" || intent === "pause" || intent === "delete") && ids.length > 0 && !confirmed) {
    return redirectApp(request, listPath({ confirmAction: intent, confirmIds: ids.join(","), count: String(ids.length), view }));
  }
  if (intent === "delete" && ids.length > 0 && confirmed && !confirmDelete) {
    return redirectApp(request, listPath({ confirmAction: "delete", confirmIds: ids.join(","), count: String(ids.length), view }));
  }
  if (process.env.NEXT_PUBLIC_UI_PREVIEW === "1") {
    if (intent === "stop-all") {
      return redirectApp(request, listPath({ notice: "preview-stop-all", view }));
    }
    if (ids.length === 0) {
      return redirectWithError(request, "请先勾选要操作的机器人。", view);
    }
    if (intent === "start" || intent === "pause" || intent === "delete") {
      return redirectApp(request, listPath({ notice: `preview-batch-${intent}`, view }));
    }
    return redirectWithError(request, "Unknown batch strategy action.", view);
  }
  const sessionToken = readSessionToken(request);
  if (!sessionToken) {
    return redirectPublic(request, "/login?error=session+expired");
  }

  if (intent === "stop-all") {
    const response = await fetch(`${authApiBaseUrl()}/strategies/stop-all`, {
      method: "POST",
      headers: { authorization: `Bearer ${sessionToken}` },
      cache: "no-store",
    });
    if (!response.ok) {
      return redirectWithError(request, await readError(response), view);
    }
    const payload = (await response.json()) as { stopped?: number };
    if ((payload.stopped ?? 0) === 0) {
      return redirectWithError(request, "No running strategies were stopped.", view);
    }
    return redirectApp(request, listPath({ notice: "stop-all-complete", view }));
  }

  if (ids.length === 0) {
    return redirectWithError(request, "Select at least one strategy.", view);
  }

  const path = intent === "start" ? "/strategies/batch/start" : intent === "pause" ? "/strategies/batch/pause" : intent === "delete" ? "/strategies/batch/delete" : null;
  if (!path) {
    return redirectWithError(request, "Unknown batch strategy action.", view);
  }

  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${sessionToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ ids }),
    cache: "no-store",
  });
  if (!response.ok) {
    return redirectWithError(request, await readError(response), view);
  }

  const payload = (await response.json()) as {
    started?: number;
    paused?: number;
    deleted?: number;
    failures?: Array<{ error?: string }>;
  };
  const changed = intent === "start" ? payload.started ?? 0 : intent === "pause" ? payload.paused ?? 0 : payload.deleted ?? 0;
  if (changed === 0) {
    const firstFailure = payload.failures?.[0]?.error;
    if (firstFailure) {
      return redirectWithError(request, firstFailure, view);
    }
    return redirectWithError(request, intent === "start" ? "No selected strategy could be started." : intent === "pause" ? "No running strategy was paused." : "Selected strategies could not be deleted.", view);
  }

  return redirectApp(request, listPath({ notice: `batch-${intent}-complete`, view }));
}

function redirectWithError(request: Request, error: string, view = "") {
  return redirectApp(request, listPath({ error, view }));
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

function redirectApp(request: Request, path: string) {
  return NextResponse.redirect(publicUrl(request, localizedAppPath(request, path)), { status: 303 });
}

function redirectPublic(request: Request, path: string) {
  return NextResponse.redirect(publicUrl(request, localizedPublicPath(request, path)), { status: 303 });
}

function listPath({
  confirmAction,
  confirmIds,
  count,
  error,
  notice,
  view,
}: {
  confirmAction?: string;
  confirmIds?: string;
  count?: string;
  error?: string;
  notice?: string;
  view?: string;
}) {
  const params = new URLSearchParams();
  if (view === "cards" || view === "table") {
    params.set("view", view);
  }
  if (confirmAction) {
    params.set("confirmAction", confirmAction);
  }
  if (confirmIds) {
    params.set("confirmIds", confirmIds);
  }
  if (count) {
    params.set("count", count);
  }
  if (error) {
    params.set("error", error);
  }
  if (notice) {
    params.set("notice", notice);
  }
  const query = params.toString();
  return query ? `/strategies?${query}` : "/strategies";
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
