import { cookies } from "next/headers";
import { redirect } from "next/navigation";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DataTable } from "@/components/ui/table";
import { getAdminAuditData, getCurrentAdminProfile } from "@/lib/api/admin-product-state";
import { pickText, resolveUiLanguage, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";

function payloadString(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (value === null) {
    return "null";
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return "-";
}

function sessionSummary(lang: UiLanguage, payload: Record<string, unknown>) {
  const role = typeof payload.session_role === "string" && payload.session_role.length > 0 ? payload.session_role : "-";
  const sid = typeof payload.session_sid === "number" ? String(payload.session_sid) : "-";
  return pickText(lang, "会话角色 " + role + "，会话 SID " + sid, "Session role " + role + ", session SID " + sid);
}

function beforeAfterSummary(lang: UiLanguage, payload: Record<string, unknown>) {
  const before = typeof payload.before_summary === "string" && payload.before_summary.length > 0 ? payload.before_summary : "-";
  const after = typeof payload.after_summary === "string" && payload.after_summary.length > 0 ? payload.after_summary : "-";
  return pickText(lang, "变更前 " + before + "；变更后 " + after, "Before " + before + "; after " + after);
}

function payloadSummary(payload: Record<string, unknown>) {
  const entries = Object.entries(payload);
  if (entries.length === 0) {
    return "-";
  }
  return entries.map(([key, value]) => key.replace(/_/g, " ") + " " + payloadString(value)).join(" | ");
}

export default async function AdminAuditPage() {
  const [cookieStore, profile] = await Promise.all([cookies(), getCurrentAdminProfile()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  if (profile.admin_role !== "super_admin") {
    redirect("/admin/dashboard");
  }
  const data = await getAdminAuditData();

  return (
    <>
      <AppShellSection
        description={pickText(lang, "超级管理员值班席位通过这张表回看审计留痕、会话摘要与变更前后说明。", "The super-admin desk reviews audit trails, session summaries, and before-after notes here.")}
        eyebrow={pickText(lang, "审计留痕", "Audit Trail")}
        title={pickText(lang, "审计复核", "Audit Review")}
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "审计日志", "Audit Log")}</CardTitle>
              <CardDescription>{pickText(lang, "服务端写入的审计记录会保留操作者、目标与会话摘要。", "Server-side audit records keep actor, target, and session summary visible.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "time", label: pickText(lang, "时间", "Time") },
                  { key: "actor", label: pickText(lang, "操作者", "Actor") },
                  { key: "action", label: pickText(lang, "动作", "Action") },
                  { key: "session", label: pickText(lang, "会话摘要", "Session Summary") },
                  { key: "target", label: pickText(lang, "目标", "Target") },
                ]}
                rows={data.items.map((item, index) => ({
                  action: item.action,
                  actor: item.actor_email,
                  id: item.action + String(index),
                  session: sessionSummary(lang, item.payload),
                  target: item.target_type + ":" + item.target_id,
                  time: item.created_at.replace("T", " ").slice(0, 16),
                }))}
              />
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "变更前后", "Before / After")}</CardTitle>
              <CardDescription>{pickText(lang, "关键动作会带上变更前后摘要和原始 payload。", "Critical actions include before-after summaries and raw payload detail.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <DataTable
                columns={[
                  { key: "action", label: pickText(lang, "动作", "Action") },
                  { key: "summary", label: pickText(lang, "摘要", "Summary") },
                  { key: "payload", label: pickText(lang, "原始载荷", "Payload") },
                ]}
                rows={data.items.map((item, index) => ({
                  action: item.action,
                  id: "summary-" + String(index),
                  payload: payloadSummary(item.payload),
                  summary: beforeAfterSummary(lang, item.payload),
                }))}
              />
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
