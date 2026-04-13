import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { DataTable } from "@/components/ui/table";
import { localizeNotificationTitle } from "@/lib/ui/domain-copy";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";
import { DISPLAY_TIME_ZONE, formatTaipeiDateTime } from "@/lib/ui/time";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string }>;
};

type NotificationInboxResponse = {
  email: string;
  items: Array<{
    created_at: string;
    delivered_at?: string | null;
    event: { kind: string; message: string; title: string };
    in_app_delivered: boolean;
    telegram_delivered: boolean;
  }>;
};

export default async function NotificationsPage({ params }: PageProps) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const inbox = await fetchNotifications();
  const items = inbox?.items ?? [];
  const inAppCount = items.filter((item) => item.in_app_delivered).length;
  const telegramCount = items.filter((item) => item.telegram_delivered).length;

  return (
    <>
      <AppShellSection
        actions={
          <Link className="inline-flex items-center justify-center rounded-sm px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary" href={`/${locale}/app/telegram`}>
            {pickText(lang, "打开 Telegram 送达", "Open Telegram delivery")}
          </Link>
        }
        description={pickText(lang, "这里展示站内通知本身；时间统一按 ", "This route is the in-app inbox itself. Time zone: ") + DISPLAY_TIME_ZONE + pickText(lang, " 显示，Telegram 页面只负责绑定和送达状态。", ". The Telegram page only handles binding and delivery status.")}
        eyebrow={pickText(lang, "站内通知", "In-app inbox")}
        title={pickText(lang, "系统通知", "Notifications")}
      >
        <div className="content-grid content-grid--metrics">
          {[
            [pickText(lang, "站内已送达", "In-app delivered"), String(inAppCount)],
            [pickText(lang, "Telegram 已送达", "Telegram delivered"), String(telegramCount)],
            [pickText(lang, "总事件", "Total events"), String(items.length)],
          ].map(([label, value]) => (
            <Card key={label}>
              <CardHeader>
                <CardTitle>{value}</CardTitle>
                <CardDescription>{label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>

      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "通知列表", "Notification feed")}</CardTitle>
          <CardDescription>{pickText(lang, "策略、会员、充值与凭证异常都会在这里留痕。", "Strategy, membership, deposit, and credential events remain visible here.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "event", label: pickText(lang, "事件", "Event") },
              { key: "summary", label: pickText(lang, "简要说明", "Summary") },
              { key: "time", label: pickText(lang, "时间", "Timestamp") },
              { key: "inbox", label: pickText(lang, "站内", "In-app") },
              { key: "telegram", label: pickText(lang, "Telegram", "Telegram"), align: "right" },
            ]}
            emptyMessage={pickText(lang, "当前还没有通知事件。", "No notification events yet.")}
            rows={items.map((item, index) => ({
              id: `${item.event.kind}-${index}`,
              event: localizeNotificationTitle(lang, item.event.kind, item.event.title),
              summary: item.event.message,
              time: formatTaipeiDateTime(item.created_at, lang),
              inbox: <Chip tone={item.in_app_delivered ? "success" : "info"}>{describeDelivery(lang, item.in_app_delivered, true)}</Chip>,
              telegram: <Chip tone={item.telegram_delivered ? "success" : "warning"}>{describeDelivery(lang, item.telegram_delivered, false)}</Chip>,
            }))}
          />
        </CardBody>
      </Card>
    </>
  );
}

async function fetchNotifications() {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
  const profile = await fetchProfile(sessionToken);
  if (!profile?.email) {
    return null;
  }
  const response = await fetch(`${authApiBaseUrl()}/notifications?email=${encodeURIComponent(profile.email)}`, {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return { email: profile.email, items: [] } as NotificationInboxResponse;
  }
  return (await response.json()) as NotificationInboxResponse;
}

async function fetchProfile(sessionToken: string): Promise<{ email: string } | null> {
  const response = await fetch(authApiBaseUrl() + "/profile", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  const payload = (await response.json()) as { email?: string };
  return typeof payload.email === "string" ? { email: payload.email } : null;
}

function describeDelivery(lang: UiLanguage, delivered: boolean, inApp: boolean) {
  if (delivered) {
    return pickText(lang, "已送达", "Delivered");
  }
  return inApp ? pickText(lang, "待送达", "Queued") : pickText(lang, "未送达", "Not delivered");
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
