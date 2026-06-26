import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { Button, FormStack } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { localizeNotificationTitle } from "@/lib/ui/domain-copy";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE, type UiLanguage } from "@/lib/ui/preferences";
import { formatTaipeiDateTime } from "@/lib/ui/time";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    code?: string | string[];
    error?: string | string[];
    expires?: string | string[];
    notice?: string | string[];
  }>;
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

type TelegramBindingStatus = {
  bound: boolean;
  bound_at: string | null;
  chat_id: string | null;
  telegram_user_id: string | null;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function NotificationsPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const query = (await searchParams) ?? {};
  const notice = firstValue(query.notice);
  const error = firstValue(query.error);
  const bindCode = firstValue(query.code) ?? "";
  const expiresAt = firstValue(query.expires) ?? "";
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const context = await fetchNotificationCenterContext();
  const binding = context?.binding ?? null;
  const items = context?.notifications.items ?? [];
  const inAppCount = items.filter((item) => item.in_app_delivered).length;
  const telegramCount = items.filter((item) => item.telegram_delivered).length;
  const telegramBotLink = await resolveTelegramBotLink();

  return (
    <>
      {error ? <StatusBanner description={error} title={pickText(lang, "Telegram 操作失败", "Telegram action failed")} tone="info" lang={lang} /> : null}
      {notice === "bind-code-issued" ? (
        <StatusBanner
          action={telegramBotLink ? {
            href: telegramBotLink,
            label: pickText(lang, "打开 Telegram 机器人", "Open Telegram Bot"),
          } : undefined}
          description={pickText(lang, "先打开机器人领取欢迎语，再把下面的 /bind 命令发给机器人，收到回复后刷新页面。", "Open the Telegram bot first, then send the /bind command shown below and refresh after the bot replies.")}
          lang={lang}
          title={pickText(lang, "绑定码已生成", "Bind code issued")}
          tone="info"
        />
      ) : null}
      <AppShellSection
        eyebrow={pickText(lang, "通知中心", "Notification center")}
        title={pickText(lang, "提醒", "Alerts")}
      >
        <div className="content-grid content-grid--metrics">
          {[
            [pickText(lang, "站内已送达", "In-app delivered"), String(inAppCount)],
            [pickText(lang, "Telegram 已送达", "Telegram delivered"), String(telegramCount)],
            [pickText(lang, "Telegram 状态", "Telegram status"), binding?.bound ? pickText(lang, "已绑定", "Bound") : pickText(lang, "未绑定", "Not bound")],
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

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "通知方式", "Alert channels")}</CardTitle>
            <CardDescription>{pickText(lang, "站内提醒默认开启；绑定后，重要提醒也会同步发送到 Telegram。", "In-app alerts are always on; bind Telegram to receive important alerts there too.")}</CardDescription>
          </CardHeader>
          <CardBody>
            {bindCode !== "" ? (
              <div className="space-y-3">
                <p className="text-sm text-muted-foreground">{pickText(lang, "新的绑定码已生成。", "A fresh bind code is ready.")}</p>
                <div className="rounded-md border-2 border-primary/70 bg-primary/15 px-4 py-3">
                  <p className="text-sm font-bold text-foreground">/bind {bindCode}</p>
                </div>
                {telegramBotLink ? (
                  <a className="inline-flex items-center justify-center rounded-sm border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary" href={telegramBotLink} rel="noreferrer" target="_blank">
                    {pickText(lang, "打开 Telegram 机器人", "Open Telegram Bot")}
                  </a>
                ) : (
                  <p className="text-sm text-muted-foreground">
                    {pickText(lang, "当前还没有配置机器人链接，请先让管理员填写机器人链接。", "The bot link is not configured yet. Ask the operator to set it first.")}
                  </p>
                )}
                <p className="text-sm text-muted-foreground">{pickText(lang, "过期时间", "Expires at")}: {formatTaipeiDateTime(expiresAt, lang, { fallback: pickText(lang, "即将过期", "shortly") })}</p>
                <FormStack action="/api/user/telegram" method="post">
                  <Button name="intent" type="submit" value="generate">
                    {pickText(lang, "重新生成绑定码", "Generate new bind code")}
                  </Button>
                </FormStack>
              </div>
            ) : binding?.bound ? (
              <div className="space-y-4">
                <ul className="text-list">
                  <li>{pickText(lang, "绑定时间", "Telegram bound at")}: {formatTaipeiDateTime(binding.bound_at, lang)}</li>
                  <li>{pickText(lang, "聊天 ID", "Telegram chat id")}: {binding.chat_id ?? "-"}</li>
                  <li>{pickText(lang, "用户 ID", "Telegram user id")}: {binding.telegram_user_id ?? "-"}</li>
                </ul>
                <FormStack action="/api/user/telegram" method="post">
                  <Button name="intent" type="submit" value="generate">
                    {pickText(lang, "更换 Telegram", "Rebind Telegram")}
                  </Button>
                </FormStack>
              </div>
            ) : (
              <FormStack action="/api/user/telegram" method="post">
                <Button name="intent" type="submit" value="generate">
                  {pickText(lang, "绑定 Telegram", "Bind Telegram")}
                </Button>
              </FormStack>
            )}
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "通知内容", "Notification content")}</CardTitle>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>{pickText(lang, "策略启动与暂停", "Strategy started and paused")}</li>
              <li>{pickText(lang, "API 凭证异常", "API credential issue")}</li>
              <li>{pickText(lang, "会员到期提醒", "Membership reminder")}</li>
              <li>{pickText(lang, "充值成功与逐笔盈亏", "Deposit success and per-fill PnL")}</li>
            </ul>
          </CardBody>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "通知列表", "Notification feed")}</CardTitle>
          <CardDescription>{pickText(lang, "策略、会员、充值与凭证异常都会在这里展示。", "Strategy, membership, deposit, and credential events are shown here.")}</CardDescription>
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

async function fetchNotificationCenterContext() {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
  const profile = await fetchProfile(sessionToken);
  if (!profile?.email) {
    return null;
  }
  const [binding, notifications] = await Promise.all([
    fetchBindingStatus(sessionToken, profile.email),
    fetchNotifications(sessionToken, profile.email),
  ]);
  return { binding, notifications };
}

async function fetchNotifications(sessionToken: string, email: string) {
  const response = await fetch(`${authApiBaseUrl()}/notifications?email=${encodeURIComponent(email)}`, {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return { email, items: [] } as NotificationInboxResponse;
  }
  return (await response.json()) as NotificationInboxResponse;
}

async function fetchBindingStatus(sessionToken: string, email: string) {
  const response = await fetch(authApiBaseUrl() + "/telegram/binding?email=" + encodeURIComponent(email), {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as TelegramBindingStatus;
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

async function resolveTelegramBotLink() {
  return await resolveTelegramBotBaseUrl();
}

async function resolveTelegramBotBaseUrl() {
  const explicitUrl = process.env.TELEGRAM_BOT_LINK?.trim() || process.env.NEXT_PUBLIC_TELEGRAM_BOT_LINK?.trim();
  if (explicitUrl) {
    return explicitUrl;
  }

  const username = process.env.TELEGRAM_BOT_USERNAME?.trim().replace(/^@/, "");
  if (username) {
    return `https://t.me/${username}`;
  }

  const token = process.env.TELEGRAM_BOT_TOKEN?.trim();
  if (!token || token === "dummy") {
    return null;
  }

  try {
    const response = await fetch(`${telegramApiBaseUrl()}/bot${token}/getMe`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return null;
    }
    const payload = (await response.json()) as { ok?: boolean; result?: { username?: string } };
    if (!payload.ok || typeof payload.result?.username !== "string" || payload.result.username.trim() === "") {
      return null;
    }
    return `https://t.me/${payload.result.username.trim().replace(/^@/, "")}`;
  } catch {
    return null;
  }
}

function telegramApiBaseUrl() {
  return process.env.TELEGRAM_API_BASE_URL?.trim().replace(/\/+$/, "") || "https://api.telegram.org";
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
