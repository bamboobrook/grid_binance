import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Chip } from "@/components/ui/chip";
import { Button, FormStack } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable } from "@/components/ui/table";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute, type UiLanguage } from "@/lib/ui/preferences";
import { DISPLAY_TIME_ZONE, formatTaipeiDateTime } from "@/lib/ui/time";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type TelegramPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    code?: string | string[];
    error?: string | string[];
    expires?: string | string[];
    notice?: string | string[];
  }>;
};

type TelegramBindingStatus = {
  email: string;
  bound: boolean;
  bound_at: string | null;
  chat_id: string | null;
  telegram_user_id: string | null;
};

type NotificationInboxResponse = {
  email: string;
  items: Array<{
    created_at: string;
    delivered_at?: string | null;
    event: { kind: string; message: string; title: string };
    telegram_delivered: boolean;
    in_app_delivered: boolean;
  }>;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function TelegramPage({ params, searchParams }: TelegramPageProps) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const query = (await searchParams) ?? {};
  const notice = firstValue(query.notice);
  const error = firstValue(query.error);
  const bindCode = firstValue(query.code) ?? "";
  const expiresAt = firstValue(query.expires) ?? "";
  const context = await fetchTelegramContext();
  const binding = context?.binding ?? null;
  const inbox = context?.notifications.items ?? [];
  const telegramBotLink = await resolveTelegramBotLink(bindCode || undefined);

  return (
    <>
      <StatusBanner
        action={telegramBotLink ? {
          href: telegramBotLink,
          label: pickText(lang, "打开 Telegram 机器人", "Open Telegram Bot"),
        } : undefined}
        description={pickText(lang, "Telegram 是站内通知的补充通道；完整通知时间线请看站内通知页。", "Telegram is a companion channel for the in-app inbox. Use the notifications page for the full timeline.")}
        title={pickText(lang, "Telegram 绑定状态条", "Telegram bind status strip")}
       
      />
      {error ? <StatusBanner description={error} title={pickText(lang, "Telegram 操作失败", "Telegram action failed")} /> : null}
      {notice === "bind-code-issued" ? (
        <StatusBanner
          action={telegramBotLink ? {
            href: telegramBotLink,
            label: pickText(lang, "打开 Telegram 机器人", "Open Telegram Bot"),
          } : undefined}
          description={pickText(lang, "把这一串绑定码发给机器人，收到回复后再刷新页面。", "Send the issued code to the Telegram bot, then refresh after the bot replies.")}
          extra={telegramBotLink ? pickText(lang, "如果本机已安装 Telegram，点击右侧按钮会直接带着绑定码打开机器人会话。", "If Telegram is installed, the button opens the bot chat with the bind code attached.") : undefined}
          title={pickText(lang, "绑定码已签发", "Bind code issued")}
         
        />
      ) : null}
      <AppShellSection
        actions={
          <Link className="inline-flex items-center justify-center rounded-sm px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary" href={`/${locale}/app/notifications`}>
            {pickText(lang, "查看站内通知", "View notifications")}
          </Link>
        }
        description={pickText(lang, "这里专注 Telegram 绑定和送达状态；页面时间统一按 ", "This page focuses on Telegram binding and delivery state. Time zone: ") + DISPLAY_TIME_ZONE + pickText(lang, " 显示。通知正文与完整时间线在站内通知页。", ". Message bodies and the full timeline live in the in-app inbox.")}
        eyebrow={pickText(lang, "Telegram 绑定", "Telegram bind")}
        title={pickText(lang, "Telegram 通知", "Telegram Notifications")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "绑定 Telegram 账户", "Bind your Telegram account")}</CardTitle>
              <CardDescription>{pickText(lang, "一个用户只能绑定一个 Telegram 身份。", "One user binds one Telegram identity only.")}</CardDescription>
            </CardHeader>
            <CardBody>
              {binding?.bound ? (
                <ul className="text-list">
                  <li>{pickText(lang, "绑定时间", "Telegram bound at")}: {formatTaipeiDateTime(binding.bound_at, lang)}</li>
                  <li>{pickText(lang, "聊天 ID", "Telegram chat id")}: {binding.chat_id ?? "-"}</li>
                  <li>{pickText(lang, "用户 ID", "Telegram user id")}: {binding.telegram_user_id ?? "-"}</li>
                  <li>{pickText(lang, "站内信与 Telegram 都已启用。", "Web inbox and Telegram are both active.")}</li>
                </ul>
              ) : bindCode !== "" ? (
                <>
                  <p>{pickText(lang, "新的绑定码已生成。", "A fresh bind code is ready.")}</p>
                  <p>
                    <strong>{bindCode}</strong>
                  </p>
                  <p>{pickText(lang, "请向机器人发送", "Send this to the Telegram bot")}: <strong>/start {bindCode}</strong></p>
                  {telegramBotLink ? (
                    <p>
                      <a
                        className="inline-flex items-center justify-center rounded-sm border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary"
                        href={telegramBotLink}
                        rel="noreferrer"
                        target="_blank"
                      >
                        {pickText(lang, "打开 Telegram 机器人", "Open Telegram Bot")}
                      </a>
                    </p>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      {pickText(lang, "当前还没有配置机器人链接，请先让管理员填写 TELEGRAM_BOT_LINK 或 TELEGRAM_BOT_USERNAME。", "The bot link is not configured yet. Ask the operator to set TELEGRAM_BOT_LINK or TELEGRAM_BOT_USERNAME.")}
                    </p>
                  )}
                  <p>{pickText(lang, "过期时间", "Expires at")}: {formatTaipeiDateTime(expiresAt, lang, { fallback: pickText(lang, "即将过期", "shortly") })}</p>
                  <FormStack action="/api/user/telegram" method="post">
                    <Button name="intent" type="submit" value="generate">
                      {pickText(lang, "重新生成绑定码", "Generate new bind code")}
                    </Button>
                  </FormStack>
                </>
              ) : (
                <FormStack action="/api/user/telegram" method="post">
                  <Button name="intent" type="submit" value="generate">
                    {pickText(lang, "生成绑定码", "Generate bind code")}
                  </Button>
                </FormStack>
              )}
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "通知覆盖范围", "Notification coverage")}</CardTitle>
              <CardDescription>{pickText(lang, "关键提醒会在 Web 与 Telegram 双端镜像。", "Critical alerts are mirrored in web and Telegram.")}</CardDescription>
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
      </AppShellSection>
      <Card>
        <CardHeader>
          <CardTitle>{pickText(lang, "最近送达事件", "Recent delivery events")}</CardTitle>
          <CardDescription>{pickText(lang, "即使绑定完成，送达历史也会继续保留在这里。", "Delivery history remains visible here even after binding is complete.")}</CardDescription>
        </CardHeader>
        <CardBody>
          <DataTable
            columns={[
              { key: "time", label: pickText(lang, "时间", "Timestamp") },
              { key: "event", label: pickText(lang, "事件", "Event") },
              { key: "summary", label: pickText(lang, "简要说明", "Summary") },
              { key: "channel", label: pickText(lang, "通道", "Channel") },
              { key: "state", label: pickText(lang, "状态", "State"), align: "right" },
            ]}
            rows={inbox.map((row, index) => {
              const delivery = describeDelivery(lang, row, Boolean(binding?.bound));
              return {
                id: row.event.kind + "-" + index,
                event: row.event.title,
                summary: row.event.message,
                time: formatTaipeiDateTime(row.created_at, lang),
                channel: delivery.channel,
                state: <Chip tone={delivery.tone}>{delivery.state}</Chip>,
              };
            })}
          />
        </CardBody>
      </Card>
    </>
  );
}

async function fetchTelegramContext() {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (sessionToken === "") {
    return null;
  }

  const profile = await fetchProfile(sessionToken);
  if (profile === null) {
    return null;
  }

  const results = await Promise.all([
    fetchBindingStatus(sessionToken, profile.email),
    fetchNotifications(sessionToken, profile.email),
  ]);

  return { binding: results[0], notifications: results[1] };
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

async function fetchNotifications(sessionToken: string, email: string) {
  const response = await fetch(authApiBaseUrl() + "/notifications?email=" + encodeURIComponent(email), {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return { email, items: [] } as NotificationInboxResponse;
  }
  return (await response.json()) as NotificationInboxResponse;
}

function describeDelivery(
  lang: UiLanguage,
  row: NotificationInboxResponse["items"][number],
  bound: boolean,
) {
  if (row.telegram_delivered) {
    return { channel: pickText(lang, "Telegram + 站内", "Telegram + web"), state: pickText(lang, "已送达", "Delivered"), tone: "success" as const };
  }
  if (bound) {
    return { channel: pickText(lang, "仅站内", "Web only"), state: pickText(lang, "失败", "Failed"), tone: "warning" as const };
  }
  return { channel: pickText(lang, "仅站内", "Web only"), state: pickText(lang, "未绑定", "Not bound"), tone: "info" as const };
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}

async function resolveTelegramBotLink(bindCode?: string) {
  const baseUrl = await resolveTelegramBotBaseUrl();
  if (!baseUrl) {
    return null;
  }
  if (!bindCode) {
    return baseUrl;
  }

  const url = new URL(baseUrl);
  url.searchParams.set("start", bindCode);
  return url.toString();
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
