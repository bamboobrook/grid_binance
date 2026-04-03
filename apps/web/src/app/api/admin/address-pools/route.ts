import { postAdminBackend, proxyAdminBackendError, readField, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const chain = readField(formData, "chain");
  const address = readField(formData, "address");
  const isEnabled = readField(formData, "isEnabled") === "true";

  const response = await postAdminBackend(request, "/admin/address-pools", {
    address,
    chain,
    is_enabled: isEnabled,
  });
  if (!response.ok) {
    return proxyAdminBackendError(response);
  }

  return redirectTo(request, `/admin/address-pools?updated=${encodeURIComponent(address)}`);
}
