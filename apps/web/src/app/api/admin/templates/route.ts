import { appendAuditRecord, updateAdminProductState } from "../../../../lib/api/admin-product-state";

import { readField, readSessionToken, redirectTo } from "../_shared";

export async function POST(request: Request) {
  const formData = await request.formData();
  const intent = readField(formData, "intent");
  const sessionToken = readSessionToken(request);

  updateAdminProductState(sessionToken, (state) => {
    if (intent === "create") {
      const name = readField(formData, "name");
      const market = readField(formData, "market") || "Spot";
      const mode = readField(formData, "mode") || "classic";
      if (!name) {
        state.flash.templates = {
          description: "Template name is required.",
          title: "Template save failed",
          tone: "danger",
        };
        return;
      }

      state.counters.template += 1;
      state.templates.unshift({
        copies: 0,
        id: `tpl-${state.counters.template}`,
        market: market === "spot" ? "Spot" : market,
        mode,
        name,
        status: "draft",
        updatedAt: "2026-04-02",
      });
      state.flash.templates = {
        description: `${name} is saved as a draft template and ready for publish review.`,
        title: "Template saved",
        tone: "success",
      };
      appendAuditRecord(state, {
        action: "template.create",
        actor: "Operator Nova",
        domain: "template",
        summary: `Created ${name} in draft state.`,
        target: name,
      });
      return;
    }

    const templateId = readField(formData, "templateId");
    const template = state.templates.find((item) => item.id === templateId);
    if (!template) {
      state.flash.templates = {
        description: "The selected template no longer exists.",
        title: "Template publish failed",
        tone: "danger",
      };
      return;
    }

    template.status = "published";
    template.updatedAt = "2026-04-02";
    state.flash.templates = {
      description: `${template.name} is now available for operators to apply without mutating older user copies.`,
      title: "Template published",
      tone: "success",
    };
    appendAuditRecord(state, {
      action: "template.publish",
      actor: "Operator Nova",
      domain: "template",
      summary: `Published ${template.name} to the shared template catalog.`,
      target: template.name,
    });
  });

  return redirectTo(request, "/admin/templates");
}
