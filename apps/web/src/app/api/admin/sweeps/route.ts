import { postAdminBackend, readField, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const chain = readField(formData, "chain") || "BSC";
  const asset = readField(formData, "asset") || "USDT";
  const treasuryAddress = readField(formData, "treasuryAddress");
  const fromAddress = readField(formData, "fromAddress");
  const amount = readField(formData, "amount");

  await postAdminBackend(request, "/admin/sweeps", {
    chain,
    asset,
    treasury_address: treasuryAddress,
    requested_at: new Date().toISOString(),
    transfers: [{ from_address: fromAddress, amount }],
  });

  return redirectTo(
    request,
    `/admin/sweeps?submitted=1&treasury=${encodeURIComponent(treasuryAddress)}&chain=${encodeURIComponent(chain)}&asset=${encodeURIComponent(asset)}`,
  );
}
