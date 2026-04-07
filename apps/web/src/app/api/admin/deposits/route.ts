import { postAdminBackend, readField, redirectTo } from "../_shared";

function readOptionalOrderId(...values: string[]) {
  for (const value of values) {
    if (!value) {
      continue;
    }
    const parsed = Number(value);
    if (Number.isInteger(parsed) && parsed > 0) {
      return parsed;
    }
  }
  return null;
}

export async function POST(request: Request) {
  const formData = await request.formData();
  const txHash = readField(formData, "txHash");
  const chain = readField(formData, "chain");
  const decision = readField(formData, "decision");
  const confirmation = readField(formData, "confirmation");
  const justification = readField(formData, "justification");
  const orderId = readOptionalOrderId(readField(formData, "suggestedOrderId"), readField(formData, "orderId"));

  const response = await postAdminBackend(request, "/admin/deposits/process", {
    chain,
    confirmation: confirmation || null,
    decision,
    justification: justification || null,
    order_id: orderId,
    processed_at: new Date().toISOString(),
    tx_hash: txHash,
  });
  if (!response.ok) {
    const message = await readBackendError(response);
    return redirectTo(
      request,
      `/admin/deposits?tx=${encodeURIComponent(txHash)}&error=${encodeURIComponent(message)}`,
    );
  }

  const payload = (await response.json()) as { deposit_status?: string };

  return redirectTo(
    request,
    `/admin/deposits?tx=${encodeURIComponent(txHash)}&result=${encodeURIComponent(payload.deposit_status ?? decision)}`,
  );
}


async function readBackendError(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string };
    return payload.error ?? "admin deposit request failed";
  } catch {
    return "admin deposit request failed";
  }
}
