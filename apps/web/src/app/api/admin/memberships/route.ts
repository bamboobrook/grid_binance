import { appendAuditRecord, updateAdminProductState } from "../../../../lib/api/admin-product-state";

import { readField, readSessionToken, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const membershipId = readField(formData, "membershipId");
  const sessionToken = readSessionToken(request);

  updateAdminProductState(sessionToken, (state) => {
    const membership = state.memberships.find((item) => item.id === membershipId);
    if (!membership) {
      state.flash.memberships = {
        description: "The requested membership record no longer exists.",
        title: "Membership update failed",
        tone: "danger",
      };
      return;
    }

    membership.status = "Active";
    membership.expiresAt = "2026-05-17";
    membership.graceEndsAt = null;
    membership.note = "Extended by operator after grace-period review.";
    state.flash.memberships = {
      description: `Extended to ${membership.expiresAt} and restored to Active.`,
      title: "Membership updated",
      tone: "success",
    };
    appendAuditRecord(state, {
      action: "membership.extend",
      actor: "Operator Nova",
      domain: "membership",
      summary: `Extended ${membership.email} by 30 days from the memberships console.`,
      target: membership.email,
    });
  });

  return redirectTo(request, "/admin/memberships");
}
