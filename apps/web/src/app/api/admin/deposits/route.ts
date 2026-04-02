import { postAdminBackend, readField, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const txHash = readField(formData, "txHash");
  const chain = readField(formData, "chain");
  const decision = readField(formData, "decision");
  const orderId = readField(formData, "orderId");

  const response = await postAdminBackend(request, "/admin/deposits/process", {
    chain,
    decision,
    order_id: orderId ? Number(orderId) : null,
    processed_at: new Date().toISOString(),
    tx_hash: txHash,
  });
  const payload = (await response.json()) as { deposit_status?: string };

  return redirectTo(
    request,
    `/admin/deposits?tx=${encodeURIComponent(txHash)}&result=${encodeURIComponent(payload.deposit_status ?? decision)}`,
  );
}
