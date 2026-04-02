import { appendAuditRecord, updateAdminProductState } from "../../../../lib/api/admin-product-state";

import { readField, readSessionToken, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const sessionToken = readSessionToken(request);
  const bscConfirmations = readField(formData, "bscConfirmations") || "12";

  updateAdminProductState(sessionToken, (state) => {
    state.system.billing.bscConfirmations = bscConfirmations;
    state.flash.system = {
      description: `Billing confirmation thresholds were updated. BSC now requires ${bscConfirmations} confirmations.`,
      title: "System configuration saved",
      tone: "success",
    };
    appendAuditRecord(state, {
      action: "system.update",
      actor: "Operator Nova",
      domain: "system",
      summary: `Updated BSC confirmations to ${bscConfirmations}.`,
      target: "billing-config",
    });
  });

  return redirectTo(request, "/admin/system");
}
