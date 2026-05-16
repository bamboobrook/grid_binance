export async function requestBacktestApi(input: string, init?: RequestInit) {
  try {
    const response = await fetch(input, init);
    const text = await response.text();
    const contentType = response.headers.get("content-type") ?? "";
    let data: unknown = null;

    if (looksLikeJson(text, contentType)) {
      try {
        data = JSON.parse(text);
      } catch {
        data = null;
      }
    }

    const message = extractMessage(data, text, response.ok, response.status);

    return {
      ok: response.ok,
      status: response.status,
      text,
      data,
      message,
    };
  } catch (error) {
    return {
      ok: false,
      status: 0,
      text: "",
      data: null,
      message: readNetworkErrorMessage(error),
    };
  }
}

export async function publishPortfolio(payload: unknown) {
  return requestBacktestApi("/api/user/backtest/portfolios/publish", {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify(payload),
  });
}

function looksLikeJson(text: string, contentType: string) {
  if (contentType.includes("application/json")) {
    return true;
  }
  const trimmed = text.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

function extractMessage(data: unknown, text: string, ok: boolean, status: number) {
  if (data && typeof data === "object" && "error" in data && typeof data.error === "string") {
    return data.error;
  }
  if (data && typeof data === "object" && "message" in data && typeof data.message === "string") {
    return data.message;
  }
  if (text.trim()) {
    return text;
  }
  return ok ? "OK" : `Request failed (${status})`;
}

function readNetworkErrorMessage(error: unknown) {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return "Network request failed";
}
