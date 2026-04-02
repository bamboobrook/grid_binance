import { postAdminBackend, readField, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");

  if (intent === "save-plan") {
    const code = readField(formData, "code");
    const name = readField(formData, "name");
    const durationDays = Number(readField(formData, "durationDays") || "0");
    const isActive = readField(formData, "isActive") !== "false";
    await postAdminBackend(request, "/admin/memberships/plans", {
      code,
      name,
      duration_days: durationDays,
      is_active: isActive,
      prices: [
        { chain: "BSC", asset: "USDT", amount: readField(formData, "bscUsdtPrice") || "20.00" },
        { chain: "ETH", asset: "USDT", amount: readField(formData, "ethUsdtPrice") || "20.00" },
        { chain: "SOL", asset: "USDC", amount: readField(formData, "solUsdcPrice") || "20.00" },
      ],
    });
    return redirectTo(request, `/admin/memberships?planSaved=${encodeURIComponent(code)}`);
  }

  const email = readField(formData, "email");
  const action = readField(formData, "action");
  const durationDays = Number(readField(formData, "durationDays") || "0");
  const at = new Date().toISOString();

  if (action === "freeze" || action === "revoke") {
    await postAdminBackend(request, "/admin/memberships/override", {
      email,
      status: action === "freeze" ? "Frozen" : "Revoked",
      at,
    });
  } else {
    await postAdminBackend(request, "/admin/memberships/manage", {
      action,
      at,
      duration_days: action === "open" || action === "extend" ? durationDays : null,
      email,
    });
  }

  return redirectTo(request, `/admin/memberships?target=${encodeURIComponent(email)}&action=${encodeURIComponent(action)}`);
}
