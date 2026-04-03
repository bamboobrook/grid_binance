import { postAdminBackend, proxyAdminBackendError, readField, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const eth = Number(readField(formData, "ethConfirmations") || "12");
  const bsc = Number(readField(formData, "bscConfirmations") || "12");
  const sol = Number(readField(formData, "solConfirmations") || "12");

  const response = await postAdminBackend(request, "/admin/system", {
    bsc_confirmations: bsc,
    eth_confirmations: eth,
    sol_confirmations: sol,
  });
  if (!response.ok) {
    return proxyAdminBackendError(response);
  }

  return redirectTo(request, `/admin/system?saved=1&eth=${eth}&bsc=${bsc}&sol=${sol}`);
}
