import { postAdminBackend, readField, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
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

  return redirectTo(request, `/admin/memberships?email=${encodeURIComponent(email)}&action=${encodeURIComponent(action)}`);
}
