import { appendAuditRecord, updateAdminProductState } from "../../../../lib/api/admin-product-state";

import { readField, readSessionToken, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const depositId = readField(formData, "depositId");
  const sessionToken = readSessionToken(request);

  updateAdminProductState(sessionToken, (state) => {
    const deposit = state.deposits.find((item) => item.id === depositId);
    if (!deposit) {
      state.flash.deposits = {
        description: "The abnormal deposit case no longer exists.",
        title: "Deposit update failed",
        tone: "danger",
      };
      return;
    }

    deposit.state = "refunded";
    deposit.note = "Treasury refund recorded after support verification.";
    state.flash.deposits = {
      description: "Refunded after user contact. Treasury refund recorded after support verification.",
      title: "Deposit case updated",
      tone: "success",
    };
    appendAuditRecord(state, {
      action: "deposit.refund",
      actor: "Operator Mira",
      domain: "deposit",
      summary: `Resolved ${deposit.order} as refunded after support review.`,
      target: deposit.order,
    });
  });

  return redirectTo(request, "/admin/deposits");
}
